use actix::{Actor, ActorContext, Addr, AsyncContext, Context, Handler, Message, MessageResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    InProgress,
    Completed,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMetadata {
    pub id: Uuid,
    pub name: String,
    pub message: String,
    pub status: TaskStatus,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub result: Option<String>,
    pub error: Option<String>,
    pub timeout_ms: u64,
    pub actual_duration_ms: Option<u64>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub timeout_at: Option<DateTime<Utc>>,
}

impl TaskMetadata {
    pub fn calculate_duration(&mut self) {
        if let Some(finished_at) = self.finished_at {
            let duration = finished_at.signed_duration_since(self.started_at);
            self.actual_duration_ms = Some(duration.num_milliseconds().max(0) as u64);
        }
    }

    pub fn mark_completed(&mut self, result: String) {
        self.status = TaskStatus::Completed;
        self.finished_at = Some(Utc::now());
        self.result = Some(result);
        self.calculate_duration();
    }

    pub fn mark_error(&mut self, error: String, is_timeout: bool) {
        self.status = TaskStatus::Error;
        let now = Utc::now();
        self.finished_at = Some(now);
        self.error = Some(error);
        if is_timeout {
            self.timeout_at = Some(now);
        }
        self.calculate_duration();
    }

    pub fn mark_cancelled(&mut self) {
        self.status = TaskStatus::Error;
        let now = Utc::now();
        self.finished_at = Some(now);
        self.cancelled_at = Some(now);
        self.error = Some("Task was cancelled".to_string());
        self.calculate_duration();
    }

    pub fn was_cancelled(&self) -> bool {
        self.cancelled_at.is_some()
    }

    pub fn was_timeout(&self) -> bool {
        self.timeout_at.is_some()
    }
}

#[derive(Debug)]
pub struct TaskActor {
    pub metadata: TaskMetadata,
}

impl TaskActor {
    pub fn new(name: String, message: String, timeout_ms: u64) -> Self {
        Self {
            metadata: TaskMetadata {
                id: Uuid::new_v4(),
                name,
                message,
                status: TaskStatus::InProgress,
                started_at: Utc::now(),
                finished_at: None,
                result: None,
                error: None,
                timeout_ms,
                actual_duration_ms: None,
                cancelled_at: None,
                timeout_at: None,
            },
        }
    }
}

impl Actor for TaskActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        println!("TaskActor {} started", self.metadata.id);
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        println!("TaskActor {} stopped", self.metadata.id);
    }
}

#[derive(Message, Debug, Clone, Serialize, Deserialize)]
#[rtype(result = "()")]
pub struct StartTask {
    #[serde(with = "duration_serde")]
    pub duration: Duration,
}

#[derive(Message, Debug, Clone, Serialize, Deserialize)]
#[rtype(result = "()")]
pub struct CompleteTask {
    pub result: String,
}

#[derive(Message, Debug, Clone, Serialize, Deserialize)]
#[rtype(result = "()")]
pub struct ErrorTask {
    pub error: String,
}

#[derive(Message, Debug, Clone, Serialize, Deserialize)]
#[rtype(result = "()")]
pub struct CancelTask;

#[derive(Message, Debug, Clone, Serialize, Deserialize)]
#[rtype(result = "TaskMetadata")]
pub struct GetTaskStatus;

// Duration serialization helper
mod duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration.as_secs().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(Duration::from_secs(secs))
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct TimeoutTask;

impl Handler<StartTask> for TaskActor {
    type Result = ();

    fn handle(&mut self, msg: StartTask, ctx: &mut Self::Context) -> Self::Result {
        println!(
            "Starting task {} with duration {:?} (timeout: {}ms)",
            self.metadata.name, msg.duration, self.metadata.timeout_ms
        );

        let addr = ctx.address();
        let timeout = Duration::from_millis(self.metadata.timeout_ms);

        // Set up timeout handler
        ctx.run_later(timeout, move |_act, ctx| {
            ctx.address().do_send(TimeoutTask);
        });

        // Start the actual work
        let work_addr = addr.clone();
        actix::spawn(async move {
            tokio::time::sleep(msg.duration).await;
            let _ = work_addr
                .send(CompleteTask {
                    result: "Task completed successfully".to_string(),
                })
                .await;
        });
    }
}

impl Handler<TimeoutTask> for TaskActor {
    type Result = ();

    fn handle(&mut self, _msg: TimeoutTask, ctx: &mut Self::Context) -> Self::Result {
        if self.metadata.status == TaskStatus::InProgress {
            self.metadata.mark_error(
                format!("Task timed out after {}ms", self.metadata.timeout_ms),
                true,
            );
            println!(
                "Task {} timed out after {}ms",
                self.metadata.name, self.metadata.timeout_ms
            );
            ctx.stop();
        }
    }
}

impl Handler<CompleteTask> for TaskActor {
    type Result = ();

    fn handle(&mut self, msg: CompleteTask, ctx: &mut Self::Context) -> Self::Result {
        self.metadata.mark_completed(msg.result);
        println!(
            "Task {} completed in {}ms: {:?}",
            self.metadata.name,
            self.metadata.actual_duration_ms.unwrap_or(0),
            self.metadata.result
        );
        ctx.stop();
    }
}

impl Handler<ErrorTask> for TaskActor {
    type Result = ();

    fn handle(&mut self, msg: ErrorTask, ctx: &mut Self::Context) -> Self::Result {
        self.metadata.mark_error(msg.error, false);
        println!(
            "Task {} failed after {}ms: {:?}",
            self.metadata.name,
            self.metadata.actual_duration_ms.unwrap_or(0),
            self.metadata.error
        );
        ctx.stop();
    }
}

impl Handler<CancelTask> for TaskActor {
    type Result = ();

    fn handle(&mut self, _msg: CancelTask, ctx: &mut Self::Context) -> Self::Result {
        self.metadata.mark_cancelled();
        println!(
            "Task {} cancelled after {}ms",
            self.metadata.name,
            self.metadata.actual_duration_ms.unwrap_or(0)
        );
        ctx.stop();
    }
}

impl Handler<GetTaskStatus> for TaskActor {
    type Result = MessageResult<GetTaskStatus>;

    fn handle(&mut self, _msg: GetTaskStatus, _ctx: &mut Self::Context) -> Self::Result {
        MessageResult(self.metadata.clone())
    }
}

#[derive(Debug)]
pub struct TaskManagerActor {
    tasks: HashMap<Uuid, Addr<TaskActor>>,
    task_metadata: HashMap<Uuid, TaskMetadata>,
}

impl TaskManagerActor {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            task_metadata: HashMap::new(),
        }
    }

    fn cleanup_finished_task(&mut self, task_id: Uuid) {
        if let Some(_addr) = self.tasks.remove(&task_id) {
            println!("Cleaning up finished task: {}", task_id);
        }
    }
}

impl Default for TaskManagerActor {
    fn default() -> Self {
        Self::new()
    }
}

impl Actor for TaskManagerActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        println!("TaskManagerActor started");
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        println!("TaskManagerActor stopped");
    }
}

#[derive(Message, Debug, Clone, Serialize, Deserialize)]
#[rtype(result = "Uuid")]
pub struct CreateTask {
    pub name: String,
    pub message: String,
    pub task_type: TaskType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskType {
    Quick {
        timeout_ms: Option<u64>,
    },
    Long {
        timeout_ms: Option<u64>,
    },
    Error {
        timeout_ms: Option<u64>,
        error_type: ErrorType,
    },
    Custom {
        name: String,
        timeout_ms: u64,
        failure_rate: Option<f32>, // 0.0-1.0, probability of failure
    },
}

impl TaskType {
    pub fn get_timeout(&self) -> Duration {
        let timeout_ms = match self {
            TaskType::Quick { timeout_ms } => timeout_ms.unwrap_or(2000), // 2s default
            TaskType::Long { timeout_ms } => timeout_ms.unwrap_or(10000), // 10s default
            TaskType::Error { timeout_ms, .. } => timeout_ms.unwrap_or(5000), // 5s default
            TaskType::Custom { timeout_ms, .. } => *timeout_ms,
        };

        Duration::from_millis(timeout_ms.max(100)) // Minimum 100ms
    }

    pub fn get_name(&self) -> String {
        match self {
            TaskType::Quick { .. } => "Quick Task".to_string(),
            TaskType::Long { .. } => "Long Task".to_string(),
            TaskType::Error { error_type, .. } => format!("Error Task ({:?})", error_type),
            TaskType::Custom { name, .. } => name.clone(),
        }
    }

    pub fn should_fail(&self) -> Option<ErrorType> {
        match self {
            TaskType::Error { error_type, .. } => Some(error_type.clone()),
            TaskType::Custom { failure_rate, .. } => {
                if let Some(rate) = failure_rate {
                    if rand::random::<f32>() < *rate {
                        Some(ErrorType::Random)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorType {
    Immediate,       // Fails immediately
    Timeout,         // Fails after timeout
    Random,          // Random failure during execution
    NetworkError,    // Simulates network failure
    ValidationError, // Simulates validation error
}

#[derive(Message, Debug, Clone, Serialize, Deserialize)]
#[rtype(result = "Vec<TaskMetadata>")]
pub struct GetAllTasks;

#[derive(Message, Debug, Clone, Serialize, Deserialize)]
#[rtype(result = "Option<TaskMetadata>")]
pub struct GetTask {
    pub id: Uuid,
}

#[derive(Message, Debug, Clone, Serialize, Deserialize)]
#[rtype(result = "bool")]
pub struct CancelTaskById {
    pub id: Uuid,
}

#[derive(Message, Debug, Clone, Serialize, Deserialize)]
#[rtype(result = "()")]
pub struct TaskFinished {
    pub id: Uuid,
    pub metadata: TaskMetadata,
}

// API Response Types for Web Interface
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCreateResponse {
    pub id: Uuid,
    pub name: String,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskListResponse {
    pub tasks: Vec<TaskMetadata>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStatusUpdate {
    pub id: Uuid,
    pub status: TaskStatus,
    pub updated_at: DateTime<Utc>,
    pub result: Option<String>,
    pub error: Option<String>,
}

impl Handler<CreateTask> for TaskManagerActor {
    type Result = MessageResult<CreateTask>;

    fn handle(&mut self, msg: CreateTask, ctx: &mut Self::Context) -> Self::Result {
        let timeout = msg.task_type.get_timeout();
        let task_name = if msg.name.is_empty() {
            msg.task_type.get_name()
        } else {
            msg.name.clone()
        };

        let task = TaskActor::new(task_name, msg.message.clone(), timeout.as_millis() as u64);

        let task_id = task.metadata.id;
        let initial_metadata = task.metadata.clone();
        let task_addr = task.start();

        // Store initial metadata
        self.task_metadata.insert(task_id, initial_metadata);
        self.tasks.insert(task_id, task_addr.clone());

        // Check if task should fail immediately
        if let Some(error_type) = msg.task_type.should_fail() {
            match error_type {
                ErrorType::Immediate => {
                    let error_addr = task_addr.clone();
                    actix::spawn(async move {
                        let _ = error_addr
                            .send(ErrorTask {
                                error: "Immediate failure simulation".to_string(),
                            })
                            .await;
                    });
                    return MessageResult(task_id);
                }
                _ => {
                    // Other error types will be handled during execution
                }
            }
        }

        // Calculate work duration (should be less than timeout for successful completion)
        let work_duration = match &msg.task_type {
            TaskType::Quick { .. } => Duration::from_millis(timeout.as_millis() as u64 * 3 / 4), // 75% of timeout
            TaskType::Long { .. } => Duration::from_millis(timeout.as_millis() as u64 * 8 / 10), // 80% of timeout
            TaskType::Error { error_type, .. } => {
                match error_type {
                    ErrorType::Timeout => timeout + Duration::from_millis(1000), // Intentionally exceed timeout
                    ErrorType::Random => Duration::from_millis(timeout.as_millis() as u64 / 2), // 50% of timeout
                    _ => Duration::from_millis(timeout.as_millis() as u64 / 3), // 33% of timeout
                }
            }
            TaskType::Custom { .. } => Duration::from_millis(timeout.as_millis() as u64 * 3 / 4), // 75% of timeout
        };

        // Send start message
        let start_addr = task_addr.clone();
        actix::spawn(async move {
            let _ = start_addr
                .send(StartTask {
                    duration: work_duration,
                })
                .await;
        });

        // Set up task completion notification
        let manager_addr = ctx.address();
        let notification_timeout = timeout + Duration::from_millis(500);

        actix::spawn(async move {
            tokio::time::sleep(notification_timeout).await;

            // Try to get final status and notify manager
            if let Ok(final_metadata) = task_addr.send(GetTaskStatus).await {
                let _ = manager_addr
                    .send(TaskFinished {
                        id: task_id,
                        metadata: final_metadata,
                    })
                    .await;
            } else {
                // Task already stopped, we'll get notified through other means
            }
        });

        MessageResult(task_id)
    }
}

impl Handler<GetAllTasks> for TaskManagerActor {
    type Result = Vec<TaskMetadata>;

    fn handle(&mut self, _msg: GetAllTasks, _ctx: &mut Self::Context) -> Self::Result {
        self.task_metadata.values().cloned().collect()
    }
}

impl Handler<GetTask> for TaskManagerActor {
    type Result = Option<TaskMetadata>;

    fn handle(&mut self, msg: GetTask, _ctx: &mut Self::Context) -> Self::Result {
        self.task_metadata.get(&msg.id).cloned()
    }
}

impl Handler<CancelTaskById> for TaskManagerActor {
    type Result = bool;

    fn handle(&mut self, msg: CancelTaskById, _ctx: &mut Self::Context) -> Self::Result {
        if let Some(task_addr) = self.tasks.get(&msg.id) {
            let cancel_addr = task_addr.clone();
            actix::spawn(async move {
                let _ = cancel_addr.send(CancelTask).await;
            });

            // Update metadata to show cancelled status
            if let Some(metadata) = self.task_metadata.get_mut(&msg.id) {
                metadata.mark_cancelled();
            }

            true
        } else {
            false
        }
    }
}

impl Handler<TaskFinished> for TaskManagerActor {
    type Result = ();

    fn handle(&mut self, msg: TaskFinished, _ctx: &mut Self::Context) -> Self::Result {
        // Update stored metadata with final results
        self.task_metadata.insert(msg.id, msg.metadata);

        // Clean up the task actor reference
        self.cleanup_finished_task(msg.id);
    }
}
