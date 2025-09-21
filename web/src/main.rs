use actix::Actor;
use actix_files as fs;
use actix_web::{
    delete, get, http::header, middleware::Logger, post, web, App, HttpResponse, HttpServer,
    Responder, Result,
};
use serde::Deserialize;
use task_core::*;
use uuid::Uuid;

mod liveview;
mod shared_header;

// Application state
pub struct AppState {
    pub task_manager: actix::Addr<TaskManagerActor>,
    pub ws_monitor: actix::Addr<WebSocketMonitorActor>,
}

// Request/Response types
#[derive(Deserialize)]
struct CreateTaskRequest {
    name: String,
    message: String,
    task_type: TaskTypeRequest,
}

impl CreateTaskRequest {
    fn validate(&self) -> Result<(), ApiError> {
        // Validate name length
        if self.name.len() > 100 {
            return Err(ApiError::validation_error(
                "Task name cannot exceed 100 characters".to_string(),
                Some(serde_json::json!({
                    "field": "name",
                    "provided_length": self.name.len(),
                    "max_length": 100
                })),
            ));
        }

        // Validate message length
        if self.message.len() > 500 {
            return Err(ApiError::validation_error(
                "Task message cannot exceed 500 characters".to_string(),
                Some(serde_json::json!({
                    "field": "message",
                    "provided_length": self.message.len(),
                    "max_length": 500
                })),
            ));
        }

        // Validate task type specific constraints
        match &self.task_type {
            TaskTypeRequest::Custom {
                timeout_ms,
                failure_rate,
                ..
            } => {
                if *timeout_ms > 300000 {
                    // 5 minutes max
                    return Err(ApiError::validation_error(
                        "Custom task timeout cannot exceed 5 minutes (300000ms)".to_string(),
                        Some(serde_json::json!({
                            "field": "task_type.timeout_ms",
                            "provided_value": timeout_ms,
                            "max_value": 300000
                        })),
                    ));
                }

                if let Some(rate) = failure_rate {
                    if *rate < 0.0 || *rate > 1.0 {
                        return Err(ApiError::validation_error(
                            "Failure rate must be between 0.0 and 1.0".to_string(),
                            Some(serde_json::json!({
                                "field": "task_type.failure_rate",
                                "provided_value": rate,
                                "valid_range": "0.0-1.0"
                            })),
                        ));
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }
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
    // Validate request
    if let Err(validation_error) = req.validate() {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error(validation_error)));
    }

    let task_type = req.task_type.clone().to_task_type();
    let task_name = if req.name.is_empty() {
        task_type.get_name()
    } else {
        req.name.clone()
    };

    // Create task with proper error handling
    let task_id = match data
        .task_manager
        .send(CreateTask {
            name: task_name.clone(),
            message: req.message.clone(),
            task_type,
        })
        .await
    {
        Ok(id) => id,
        Err(_) => {
            let error = ApiError::internal_error(
                "Failed to create task - internal service error".to_string(),
            );
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error(error)));
        }
    };

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
    let tasks = match data.task_manager.send(GetAllTasks).await {
        Ok(tasks) => tasks,
        Err(_) => {
            let error = ApiError::internal_error(
                "Failed to retrieve tasks - internal service error".to_string(),
            );
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error(error)));
        }
    };

    let response = TaskListResponse {
        total: tasks.len(),
        tasks,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

#[get("/tasks/{id}")]
async fn get_task(data: web::Data<AppState>, path: web::Path<Uuid>) -> Result<impl Responder> {
    let task_id = path.into_inner();

    let task = match data.task_manager.send(GetTask { id: task_id }).await {
        Ok(task) => task,
        Err(_) => {
            let error = ApiError::internal_error(
                "Failed to retrieve task - internal service error".to_string(),
            );
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error(error)));
        }
    };

    match task {
        Some(task_metadata) => Ok(HttpResponse::Ok().json(ApiResponse::success(task_metadata))),
        None => {
            let error = ApiError::not_found("Task", &task_id.to_string());
            Ok(HttpResponse::NotFound().json(ApiResponse::<()>::error(error)))
        }
    }
}

#[delete("/tasks/{id}")]
async fn cancel_task(data: web::Data<AppState>, path: web::Path<Uuid>) -> Result<impl Responder> {
    let task_id = path.into_inner();

    // First check if task exists and get its current status
    let task_status = match data.task_manager.send(GetTask { id: task_id }).await {
        Ok(Some(task)) => task.status,
        Ok(None) => {
            let error = ApiError::not_found("Task", &task_id.to_string());
            return Ok(HttpResponse::NotFound().json(ApiResponse::<()>::error(error)));
        }
        Err(_) => {
            let error = ApiError::internal_error(
                "Failed to retrieve task status - internal service error".to_string(),
            );
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error(error)));
        }
    };

    // Check if task can be cancelled
    if task_status != TaskStatus::InProgress {
        let error = ApiError::task_already_completed(&task_id.to_string());
        return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error(error)));
    }

    // Attempt to cancel the task
    let cancelled = match data.task_manager.send(CancelTaskById { id: task_id }).await {
        Ok(result) => result,
        Err(_) => {
            let error = ApiError::internal_error(
                "Failed to cancel task - internal service error".to_string(),
            );
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error(error)));
        }
    };

    if cancelled {
        Ok(HttpResponse::Ok().json(ApiResponse::success("Task cancelled successfully")))
    } else {
        // This shouldn't happen given our checks, but handle it gracefully
        let error = ApiError::internal_error("Task cancellation failed unexpectedly".to_string());
        Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error(error)))
    }
}

#[get("/health")]
async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now()
    }))
}

#[get("/websocket/messages")]
async fn get_websocket_messages(
    data: web::Data<AppState>,
    query: web::Query<serde_json::Value>,
) -> Result<impl Responder> {
    let limit = query
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);

    let session_id = query
        .get("session_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok());

    let messages = match data
        .ws_monitor
        .send(GetWebSocketMessages { limit, session_id })
        .await
    {
        Ok(messages) => messages,
        Err(_) => {
            let error = ApiError::internal_error(
                "Failed to retrieve WebSocket messages - internal service error".to_string(),
            );
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error(error)));
        }
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(messages)))
}

#[delete("/websocket/messages")]
async fn clear_websocket_messages(data: web::Data<AppState>) -> Result<impl Responder> {
    let cleared_count = match data.ws_monitor.send(ClearWebSocketMessages).await {
        Ok(count) => count,
        Err(_) => {
            let error = ApiError::internal_error(
                "Failed to clear WebSocket messages - internal service error".to_string(),
            );
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error(error)));
        }
    };

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
            "cleared_count": cleared_count,
            "message": format!("Cleared {} WebSocket messages", cleared_count)
        }))),
    )
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
    let file_path = "web/static/redirect.html";
    match std::fs::read_to_string(file_path) {
        Ok(content) => HttpResponse::Ok().content_type("text/html").body(content),
        Err(e) => {
            eprintln!("Error reading file '{}': {}", file_path, e);
            eprintln!("Current working directory: {:?}", std::env::current_dir());
            HttpResponse::InternalServerError().body(format!(
                "Error loading LiveView redirect page: {} (cwd: {:?})",
                e,
                std::env::current_dir()
            ))
        }
    }
}

async fn websocket_monitor_page() -> impl Responder {
    use maud::{html, Markup, PreEscaped, DOCTYPE};

    let page = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "WebSocket Monitor - Task Overlord" }
                style {
                    (PreEscaped(r#"
                        * { margin: 0; padding: 0; box-sizing: border-box; }
                        body {
                            font-family: 'Segoe UI', system-ui, sans-serif;
                            background: linear-gradient(135deg, #ff6b35 0%, #f7931e 25%, #ff8c42 50%, #c73e1d 100%);
                            min-height: 100vh;
                            color: #333;
                        }
                        .container { max-width: 1400px; margin: 0 auto; padding: 20px; }

                        .monitor-header {
                            background: rgba(255, 255, 255, 0.95);
                            border-radius: 15px;
                            padding: 20px;
                            margin-bottom: 20px;
                            text-align: center;
                            box-shadow: 0 5px 15px rgba(0,0,0,0.1);
                        }
async fn websocket_monitor_page() -> impl Responder {
    use maud::{html, Markup, PreEscaped, DOCTYPE};

    let page = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "WebSocket Monitor - Task Overlord" }
                style {
                    (PreEscaped(r#"
                        * { margin: 0; padding: 0; box-sizing: border-box; }
                        body {
                            font-family: 'Segoe UI', system-ui, sans-serif;
                            background: linear-gradient(135deg, #ff6b35 0%, #f7931e 25%, #ff8c42 50%, #c73e1d 100%);
                            min-height: 100vh;
                            color: #333;
                        }
                        .container { max-width: 1400px; margin: 0 auto; padding: 20px; }

                        .monitor-header {
                            background: rgba(255, 255, 255, 0.95);
                            border-radius: 15px;
                            padding: 20px;
                            margin-bottom: 20px;
                            text-align: center;
                            box-shadow: 0 5px 15px rgba(0,0,0,0.1);
                        }
                        .monitor-header h1 { color: #2d3748; font-size: 2rem; }

                        .controls {
                            background: rgba(255, 255, 255, 0.95);
                            border-radius: 15px;
                            padding: 20px;
                            margin-bottom: 20px;
                            display: flex;
                            gap: 15px;
                            align-items: center;
                            box-shadow: 0 5px 15px rgba(0,0,0,0.1);
                        }
                        .btn {
                            padding: 10px 20px;
                            border: none;
                            border-radius: 8px;
                            cursor: pointer;
                            font-weight: 600;
                            text-transform: uppercase;
                        }
                        .btn-refresh { background: #4facfe; color: white; }
                        .btn-clear { background: #e53e3e; color: white; }
                        .auto-refresh { display: flex; align-items: center; gap: 10px; }

                        .messages-container {
                            background: rgba(255, 255, 255, 0.95);
                            border-radius: 15px;
                            padding: 20px;
                            box-shadow: 0 5px 15px rgba(0,0,0,0.1);
                        }
                        .message {
                            border-left: 4px solid #e2e8f0;
                            margin-bottom: 15px;
                            padding: 15px;
                            background: #f7fafc;
                            border-radius: 8px;
                            font-family: 'Monaco', 'Courier New', monospace;
                            font-size: 0.9rem;
                        }
                        .message.incoming { border-left-color: #3182ce; }
                        .message.outgoing { border-left-color: #38a169; }

                        .message-header {
                            display: flex;
                            justify-content: space-between;
                            margin-bottom: 10px;
                            font-weight: bold;
                        }
                        .direction.incoming { color: #3182ce; }
                        .direction.outgoing { color: #38a169; }
                        .timestamp { color: #718096; font-size: 0.8rem; }

                        .message-meta {
                            color: #718096;
                            font-size: 0.8rem;
                            margin-bottom: 10px;
                        }
                        .message-content {
                            background: #edf2f7;
                            padding: 10px;
                            border-radius: 6px;
                            white-space: pre-wrap;
                            max-height: 300px;
                            overflow-y: auto;
                        }
                        .empty-state {
                            text-align: center;
                            color: #a0aec0;
                            padding: 40px;
                            font-style: italic;
                        }
                    "#))
                }
            }
            body {
                div class="container" {
                    (shared_header::render_header())

                    div class="monitor-header" {
                        h1 { "📡 WebSocket Monitor" }
                    }

                    div class="controls" {
                        button class="btn btn-refresh" onclick="loadMessages()" { "🔄 Refresh" }
                        button class="btn btn-clear" onclick="clearMessages()" { "🗑️ Clear All" }
                        div class="auto-refresh" {
                            label {
                                input type="checkbox" id="auto-refresh" checked;
                                " Auto-refresh (5s)"
                            }
                        }
                        div {
                            "Limit: "
                            select id="limit" {
                                option value="50" selected { "50" }
                                option value="100" { "100" }
                                option value="200" { "200" }
                                option value="" { "All" }
                            }
                        }
                    }

                    div class="messages-container" {
                        div id="messages" {
                            div class="empty-state" { "Loading WebSocket messages..." }
                        }
                    }
                }

                script {
                    (PreEscaped(r#"
                        let autoRefreshInterval;

                        function loadMessages() {
                            const limit = document.getElementById('limit').value;
                            const url = `/api/websocket/messages${limit ? '?limit=' + limit : ''}`;

                            fetch(url)
                                .then(response => response.json())
                                .then(data => {
                                    if (data.success) {
                                        displayMessages(data.data);
                                    } else {
                                        document.getElementById('messages').innerHTML =
                                            '<div class="empty-state">Error loading messages: ' +
                                            (data.error?.message || 'Unknown error') + '</div>';
                                    }
                                })
                                .catch(error => {
                                    document.getElementById('messages').innerHTML =
                                        '<div class="empty-state">Network error: ' + error.message + '</div>';
                                });
                        }

                        function displayMessages(messages) {
                            const container = document.getElementById('messages');

                            if (!messages || messages.length === 0) {
                                container.innerHTML = '<div class="empty-state">No WebSocket messages yet...</div>';
                                return;
                            }

                            container.innerHTML = messages.map(msg => `
                                <div class="message ${msg.direction.toLowerCase()}">
                                    <div class="message-header">
                                        <span class="direction ${msg.direction.toLowerCase()}">
                                            ${msg.direction === 'Incoming' ? '⬇️ INCOMING' : '⬆️ OUTGOING'}
                                            [${msg.message_type}]
                                        </span>
                                        <span class="timestamp">${new Date(msg.timestamp).toLocaleString()}</span>
                                    </div>
                                    <div class="message-meta">
                                        Session: ${msg.session_id} | Size: ${msg.size_bytes} bytes
                                    </div>
                                    <div class="message-content">${JSON.stringify(JSON.parse(msg.content), null, 2)}</div>
                                </div>
                            `).join('');
                        }

                        function clearMessages() {
                            if (!confirm('Are you sure you want to clear all WebSocket messages?')) return;

                            fetch('/api/websocket/messages', { method: 'DELETE' })
                                .then(response => response.json())
                                .then(data => {
                                    if (data.success) {
                                        loadMessages();
                                        alert(data.data.message);
                                    } else {
                                        alert('Error clearing messages: ' + (data.error?.message || 'Unknown error'));
                                    }
                                })
                                .catch(error => {
                                    alert('Network error: ' + error.message);
                                });
                        }

                        function toggleAutoRefresh() {
                            const checkbox = document.getElementById('auto-refresh');

                            if (checkbox.checked) {
                                autoRefreshInterval = setInterval(loadMessages, 5000);
                            } else {
                                clearInterval(autoRefreshInterval);
                            }
                        }

                        // Initialize
                        document.getElementById('auto-refresh').addEventListener('change', toggleAutoRefresh);
                        loadMessages();
                        toggleAutoRefresh();
                    "#))
                }
            }
        }
    };

    HttpResponse::Ok()
        .content_type("text/html")
        .body(page.into_string())
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    println!("🚀 Starting Task Overlord Dashboard Server");

    // Start the actors
    let task_manager = TaskManagerActor::new().start();
    let ws_monitor = WebSocketMonitorActor::new().start();

    let app_state = web::Data::new(AppState {
        task_manager,
        ws_monitor,
    });

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
                    .service(get_websocket_messages)
                    .service(clear_websocket_messages), // .service(task_stream) // Temporarily disabled
            )
            .route("/", web::get().to(liveview_page))
            .route("/ws/", web::get().to(liveview::websocket_handler))
            .route("/liveview", web::get().to(liveview_page))
            .route("/monitor", web::get().to(websocket_monitor_page))
            .service(fs::Files::new("/static/", "web/static"))
    })
    .bind("127.0.0.1:3333")?
    .run()
    .await
}
