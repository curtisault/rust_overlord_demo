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
}

#[derive(Debug)]
pub struct TaskActor {
    pub metadata: TaskMetadata,
}

impl TaskActor {
    pub fn new(name: String, message: String) -> Self {
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

impl Handler<StartTask> for TaskActor {
    type Result = ();

    fn handle(&mut self, msg: StartTask, ctx: &mut Self::Context) -> Self::Result {
        println!("Starting task {} with duration {:?}", self.metadata.name, msg.duration);

        let addr = ctx.address();

        actix::spawn(async move {
            tokio::time::sleep(msg.duration).await;
            let _ = addr.send(CompleteTask {
                result: "Task completed successfully".to_string(),
            }).await;
        });
    }
}

impl Handler<CompleteTask> for TaskActor {
    type Result = ();

    fn handle(&mut self, msg: CompleteTask, ctx: &mut Self::Context) -> Self::Result {
        self.metadata.status = TaskStatus::Completed;
        self.metadata.finished_at = Some(Utc::now());
        self.metadata.result = Some(msg.result);

        println!("Task {} completed: {:?}", self.metadata.name, self.metadata.result);
        ctx.stop();
    }
}

impl Handler<ErrorTask> for TaskActor {
    type Result = ();

    fn handle(&mut self, msg: ErrorTask, ctx: &mut Self::Context) -> Self::Result {
        self.metadata.status = TaskStatus::Error;
        self.metadata.finished_at = Some(Utc::now());
        self.metadata.error = Some(msg.error);

        println!("Task {} failed: {:?}", self.metadata.name, self.metadata.error);
        ctx.stop();
    }
}

impl Handler<CancelTask> for TaskActor {
    type Result = ();

    fn handle(&mut self, _msg: CancelTask, ctx: &mut Self::Context) -> Self::Result {
        self.metadata.status = TaskStatus::Error;
        self.metadata.finished_at = Some(Utc::now());
        self.metadata.error = Some("Task was cancelled".to_string());

        println!("Task {} cancelled", self.metadata.name);
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
    Quick,
    Long,
    Error,
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
        let task = TaskActor::new(msg.name, msg.message);
        let task_id = task.metadata.id;
        let initial_metadata = task.metadata.clone();
        let task_addr = task.start();

        // Store initial metadata
        self.task_metadata.insert(task_id, initial_metadata);
        self.tasks.insert(task_id, task_addr.clone());

        // Start the task based on type
        let duration = match msg.task_type {
            TaskType::Quick => Duration::from_secs(2),
            TaskType::Long => Duration::from_secs(10),
            TaskType::Error => {
                // Send error message immediately
                let error_addr = task_addr.clone();
                actix::spawn(async move {
                    let _ = error_addr.send(ErrorTask {
                        error: "Simulated task error".to_string(),
                    }).await;
                });
                return MessageResult(task_id);
            }
        };

        // Send start message
        let start_addr = task_addr.clone();
        actix::spawn(async move {
            let _ = start_addr.send(StartTask { duration }).await;
        });

        // Set up task completion notification
        let manager_addr = ctx.address();
        actix::spawn(async move {
            // Wait for task to finish (a bit longer than the task duration)
            tokio::time::sleep(duration + Duration::from_millis(100)).await;

            // Try to get final status and notify manager
            if let Ok(final_metadata) = task_addr.send(GetTaskStatus).await {
                let _ = manager_addr.send(TaskFinished {
                    id: task_id,
                    metadata: final_metadata,
                }).await;
            } else {
                // Task already stopped, create finished metadata
                let finished_metadata = TaskMetadata {
                    id: task_id,
                    name: "Unknown".to_string(),
                    message: "Task completed".to_string(),
                    status: TaskStatus::Completed,
                    started_at: Utc::now(),
                    finished_at: Some(Utc::now()),
                    result: Some("Task completed successfully".to_string()),
                    error: None,
                };

                let _ = manager_addr.send(TaskFinished {
                    id: task_id,
                    metadata: finished_metadata,
                }).await;
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
                metadata.status = TaskStatus::Error;
                metadata.finished_at = Some(Utc::now());
                metadata.error = Some("Task was cancelled".to_string());
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

