use actix::Actor;
use actix_web::{
    delete, get, http::header, middleware::Logger, post, web, App, HttpResponse, HttpServer,
    Responder, Result,
};
use actix_files as fs;
use serde::Deserialize;
use task_core::*;
use uuid::Uuid;

mod liveview;

// Application state
pub struct AppState {
    pub task_manager: actix::Addr<TaskManagerActor>,
}

// Request/Response types
#[derive(Deserialize)]
struct CreateTaskRequest {
    name: String,
    message: String,
    task_type: TaskTypeRequest,
}

#[derive(Clone, Deserialize)]
#[serde(tag = "type")]
enum TaskTypeRequest {
    #[serde(rename = "quick")]
    Quick { timeout_ms: Option<u64> },
    #[serde(rename = "long")]
    Long { timeout_ms: Option<u64> },
    #[serde(rename = "error")]
    Error {
        timeout_ms: Option<u64>,
        error_type: Option<String>,
    },
    #[serde(rename = "custom")]
    Custom {
        custom_name: String,
        timeout_ms: u64,
        failure_rate: Option<f32>,
    },
}

impl TaskTypeRequest {
    fn to_task_type(self) -> TaskType {
        match self {
            TaskTypeRequest::Quick { timeout_ms } => TaskType::Quick { timeout_ms },
            TaskTypeRequest::Long { timeout_ms } => TaskType::Long { timeout_ms },
            TaskTypeRequest::Error {
                timeout_ms,
                error_type,
            } => {
                let error_type = match error_type.as_deref() {
                    Some("immediate") => ErrorType::Immediate,
                    Some("timeout") => ErrorType::Timeout,
                    Some("random") => ErrorType::Random,
                    Some("network") => ErrorType::NetworkError,
                    Some("validation") => ErrorType::ValidationError,
                    _ => ErrorType::Immediate,
                };
                TaskType::Error {
                    timeout_ms,
                    error_type,
                }
            }
            TaskTypeRequest::Custom {
                custom_name,
                timeout_ms,
                failure_rate,
            } => TaskType::Custom {
                name: custom_name,
                timeout_ms,
                failure_rate,
            },
        }
    }
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
    let task_type = req.task_type.clone().to_task_type();
    let task_name = if req.name.is_empty() {
        task_type.get_name()
    } else {
        req.name.clone()
    };

    let task_id = data
        .task_manager
        .send(CreateTask {
            name: task_name.clone(),
            message: req.message.clone(),
            task_type,
        })
        .await
        .map_err(|_| actix_web::error::ErrorInternalServerError("Failed to create task"))?;

    let response = TaskCreateResponse {
        id: task_id,
        name: task_name,
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
async fn get_task(data: web::Data<AppState>, path: web::Path<Uuid>) -> Result<impl Responder> {
    let task_id = path.into_inner();

    let task = data
        .task_manager
        .send(GetTask { id: task_id })
        .await
        .map_err(|_| actix_web::error::ErrorInternalServerError("Failed to get task"))?;

    match task {
        Some(task_metadata) => Ok(HttpResponse::Ok().json(ApiResponse::success(task_metadata))),
        None => {
            Ok(HttpResponse::NotFound()
                .json(ApiResponse::<()>::error("Task not found".to_string())))
        }
    }
}

#[delete("/tasks/{id}")]
async fn cancel_task(data: web::Data<AppState>, path: web::Path<Uuid>) -> Result<impl Responder> {
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
    // For now, return a simple response to test basic functionality
    let tasks = data
        .task_manager
        .send(GetAllTasks)
        .await
        .map_err(|_| actix_web::error::ErrorInternalServerError("Failed to get tasks"))?;

    let response = TaskListResponse {
        total: tasks.len(),
        tasks,
    };

    let json = serde_json::json!({
        "type": "task_list_update",
        "data": response,
        "timestamp": chrono::Utc::now()
    });

    Ok(HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, "text/event-stream"))
        .insert_header((header::CACHE_CONTROL, "no-cache"))
        .insert_header((header::CONNECTION, "keep-alive"))
        .insert_header((header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"))
        .body(format!("data: {}\n\n", json)))
}

async fn liveview_page() -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/html")
        .body(r#"
<!DOCTYPE html>
<html>
<head>
    <title>LiveView Redirect</title>
    <script>
        // Simple redirect to establish WebSocket connection
        const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
        const ws = new WebSocket(protocol + '//' + location.host + '/ws/');

        ws.onmessage = function(event) {
            const data = JSON.parse(event.data);
            if (data.type === 'full_page_load') {
                document.open();
                document.write(data.html);
                document.close();
            }
        };

        ws.onerror = function() {
            document.body.innerHTML = '<h1>Connecting to LiveView...</h1>';
        };
    </script>
</head>
<body>
    <h1>Initializing LiveView...</h1>
</body>
</html>
        "#)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    println!("🚀 Starting Task Overlord Dashboard Server");

    // Start the task manager actor
    let task_manager = TaskManagerActor::new().start();

    let app_state = web::Data::new(AppState { task_manager });

    println!("📡 Server starting on http://127.0.0.1:3333");

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .wrap(Logger::default())
            .service(
                web::scope("/api")
                    .service(index)
                    .service(create_task)
                    .service(get_all_tasks)
                    .service(get_task)
                    .service(cancel_task)
                    .service(health_check)
                    // .service(task_stream) // Temporarily disabled
            )
            .route("/ws/", web::get().to(liveview::websocket_handler))
            .route("/liveview", web::get().to(liveview_page))
            .service(fs::Files::new("/", "./web/static")
                .index_file("index.html"))
    })
    .bind("127.0.0.1:3333")?
    .run()
    .await
}
