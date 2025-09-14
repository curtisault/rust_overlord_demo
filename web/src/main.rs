use actix::Actor;
use actix_web::{
    delete, get, post, web, App, HttpResponse, HttpServer, Responder, Result, middleware::Logger,
    http::header,
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use task_core::*;
use uuid::Uuid;

// Application state
struct AppState {
    task_manager: actix::Addr<TaskManagerActor>,
}

// Request/Response types
#[derive(Deserialize)]
struct CreateTaskRequest {
    name: String,
    message: String,
    task_type: TaskType,
}

// API Endpoints
#[get("/")]
async fn index() -> impl Responder {
    HttpResponse::Ok().body("Task Overlord Dashboard API v1.0")
}

#[post("/tasks")]
async fn create_task(
    data: web::Data<AppState>,
    req: web::Json<CreateTaskRequest>,
) -> Result<impl Responder> {
    let task_id = data
        .task_manager
        .send(CreateTask {
            name: req.name.clone(),
            message: req.message.clone(),
            task_type: req.task_type.clone(),
        })
        .await
        .map_err(|_| actix_web::error::ErrorInternalServerError("Failed to create task"))?;

    let response = TaskCreateResponse {
        id: task_id,
        name: req.name.clone(),
        status: TaskStatus::InProgress,
        created_at: chrono::Utc::now(),
    };

    Ok(HttpResponse::Created().json(ApiResponse::success(response)))
}

#[get("/tasks")]
async fn get_all_tasks(data: web::Data<AppState>) -> Result<impl Responder> {
    let tasks = data
        .task_manager
        .send(GetAllTasks)
        .await
        .map_err(|_| actix_web::error::ErrorInternalServerError("Failed to get tasks"))?;

    let response = TaskListResponse {
        total: tasks.len(),
        tasks,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

#[get("/tasks/{id}")]
async fn get_task(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<impl Responder> {
    let task_id = path.into_inner();

    let task = data
        .task_manager
        .send(GetTask { id: task_id })
        .await
        .map_err(|_| actix_web::error::ErrorInternalServerError("Failed to get task"))?;

    match task {
        Some(task_metadata) => Ok(HttpResponse::Ok().json(ApiResponse::success(task_metadata))),
        None => Ok(HttpResponse::NotFound().json(ApiResponse::<()>::error(
            "Task not found".to_string(),
        ))),
    }
}

#[delete("/tasks/{id}")]
async fn cancel_task(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<impl Responder> {
    let task_id = path.into_inner();

    let cancelled = data
        .task_manager
        .send(CancelTaskById { id: task_id })
        .await
        .map_err(|_| actix_web::error::ErrorInternalServerError("Failed to cancel task"))?;

    if cancelled {
        Ok(HttpResponse::Ok().json(ApiResponse::success("Task cancelled successfully")))
    } else {
        Ok(HttpResponse::NotFound().json(ApiResponse::<()>::error(
            "Task not found or already completed".to_string(),
        )))
    }
}

#[get("/health")]
async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now()
    }))
}

#[get("/tasks/stream")]
async fn task_stream(data: web::Data<AppState>) -> Result<impl Responder> {
    let stream = async_stream::stream! {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(2));

        loop {
            interval.tick().await;

            // Get current tasks from manager
            if let Ok(tasks) = data.task_manager.send(GetAllTasks).await {
                let json = serde_json::to_string(&serde_json::json!({
                    "type": "task_list_update",
                    "data": TaskListResponse {
                        tasks: tasks.clone(),
                        total: tasks.len()
                    },
                    "timestamp": chrono::Utc::now()
                })).unwrap_or_else(|_| "{}".to_string());

                yield Ok::<_, actix_web::Error>(
                    web::Bytes::from(format!("data: {}\n\n", json))
                );
            }
        }
    };

    Ok(HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, "text/event-stream"))
        .insert_header((header::CACHE_CONTROL, "no-cache"))
        .insert_header((header::CONNECTION, "keep-alive"))
        .insert_header((header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"))
        .streaming(stream))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    println!("🚀 Starting Task Overlord Dashboard Server");

    // Start the task manager actor
    let task_manager = TaskManagerActor::new().start();

    let app_state = web::Data::new(AppState { task_manager });

    println!("📡 Server starting on http://127.0.0.1:3000");

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .wrap(Logger::default())
            .service(index)
            .service(create_task)
            .service(get_all_tasks)
            .service(get_task)
            .service(cancel_task)
            .service(health_check)
            .service(task_stream)
    })
    .bind("127.0.0.1:3000")?
    .run()
    .await
}
