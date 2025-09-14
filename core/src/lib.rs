use actix::{Actor, ActorContext, AsyncContext, Context, Handler, Message, MessageResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
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

#[derive(Message)]
#[rtype(result = "()")]
pub struct StartTask {
    pub duration: Duration,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct CompleteTask {
    pub result: String,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct ErrorTask {
    pub error: String,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct CancelTask;

#[derive(Message)]
#[rtype(result = "TaskMetadata")]
pub struct GetTaskStatus;

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