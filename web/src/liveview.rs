use crate::{modal, shared_header};
use actix::{Actor, ActorContext, Addr, AsyncContext, Handler, Message, StreamHandler};
use actix_web::web;
use actix_web_actors::ws;
use maud::{html, Markup, PreEscaped, DOCTYPE};
use serde_json;
use std::time::{Duration, Instant};
use task_core::*;
use uuid::Uuid;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct LiveViewState {
    pub tasks: Vec<TaskMetadata>,
}

impl Default for LiveViewState {
    fn default() -> Self {
        Self { tasks: Vec::new() }
    }
}

pub struct LiveViewSession {
    id: Uuid,
    hb: Instant,
    task_manager: Addr<TaskManagerActor>,
    ws_monitor: Addr<WebSocketMonitorActor>,
    state: LiveViewState,
    last_html: String,
}

impl LiveViewSession {
    pub fn new(
        task_manager: Addr<TaskManagerActor>,
        ws_monitor: Addr<WebSocketMonitorActor>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            hb: Instant::now(),
            task_manager,
            ws_monitor,
            state: LiveViewState::default(),
            last_html: String::new(),
        }
    }

    fn hb(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                println!("LiveView client heartbeat failed, disconnecting!");
                ctx.stop();
                return;
            }

            ctx.ping(b"");
        });
    }

    fn render_page(&self) -> Markup {
        html! {
            (DOCTYPE)
            html lang="en" {
                head {
                    meta charset="UTF-8";
                    meta name="viewport" content="width=device-width, initial-scale=1.0";
                    title { "Task Overlord LiveView" }

                    // Simple telemetry implementation (no external dependencies)
                    script {
                        (PreEscaped(r#"
                            // Simple OpenTelemetry-compatible implementation
                            window.opentelemetry = {
                                trace: {
                                    getTracer: function(name, version) {
                                        return {
                                            startSpan: function(name, options) {
                                                const span = {
                                                    name: name,
                                                    startTime: Date.now(),
                                                    attributes: options?.attributes || {},

                                                    setAttributes: function(attrs) {
                                                        Object.assign(this.attributes, attrs);
                                                    },

                                                    recordException: function(error) {
                                                        this.attributes['exception.type'] = error.constructor.name;
                                                        this.attributes['exception.message'] = error.message;
                                                    },

                                                    setStatus: function(status) {
                                                        this.attributes['span.status.code'] = status.code;
                                                        this.attributes['span.status.message'] = status.message;
                                                    },

                                                    end: function() {
                                                        this.endTime = Date.now();
                                                        this.duration = this.endTime - this.startTime;
                                                        console.log('🔍 [OTEL SPAN]', this.name, '(' + this.duration + 'ms)', this.attributes);
                                                    }
                                                };
                                                return span;
                                            }
                                        };
                                    }
                                }
                            };
                        "#))
                    }
                    style {
                        (PreEscaped(r#"
                            * { margin: 0; padding: 0; box-sizing: border-box; }
                            body {
                                font-family: 'Segoe UI', system-ui, sans-serif;
                                background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
                                min-height: 100vh;
                                color: #333;
                            }
                            .container {
                                max-width: 1200px;
                                margin: 0 auto;
                                padding: 20px;
                            }
                            .header {
                                background: rgba(255, 255, 255, 0.95);
                                border-radius: 15px;
                                padding: 30px;
                                margin-bottom: 30px;
                                box-shadow: 0 10px 30px rgba(0,0,0,0.1);
                                text-align: center;
                            }
                            .header h1 {
                                font-size: 2.5rem;
                                margin-bottom: 20px;
                                color: #2d3748;
                            }
                            .controls {
                                display: flex;
                                gap: 15px;
                                justify-content: center;
                                flex-wrap: wrap;
                            }
                            .btn {
                                padding: 12px 24px;
                                border: none;
                                border-radius: 8px;
                                font-size: 16px;
                                cursor: pointer;
                                transition: all 0.3s ease;
                                font-weight: 600;
                                text-transform: uppercase;
                                letter-spacing: 0.5px;
                            }
                            .btn:hover { transform: translateY(-2px); }
                            .btn-quick {
                                background: linear-gradient(45deg, #4facfe, #00f2fe);
                                color: white;
                            }
                            .btn-long {
                                background: linear-gradient(45deg, #fa709a, #fee140);
                                color: white;
                            }
                            .btn-error {
                                background: linear-gradient(45deg, #ff6b6b, #ffa500);
                                color: white;
                            }
                            .task-grid {
                                display: grid;
                                grid-template-columns: repeat(auto-fit, minmax(350px, 1fr));
                                gap: 25px;
                            }
                            .task-column {
                                background: rgba(255, 255, 255, 0.95);
                                border-radius: 15px;
                                padding: 25px;
                                box-shadow: 0 10px 30px rgba(0,0,0,0.1);
                            }
                            .column-header {
                                display: flex;
                                align-items: center;
                                justify-content: space-between;
                                margin-bottom: 20px;
                                padding-bottom: 15px;
                                border-bottom: 2px solid #e2e8f0;
                            }
                            .column-title {
                                font-size: 1.3rem;
                                font-weight: 700;
                            }
                            .status-in-progress { color: #3182ce; }
                            .status-completed { color: #38a169; }
                            .status-error { color: #e53e3e; }
                            .task-count {
                                background: #4a5568;
                                color: white;
                                padding: 8px 12px;
                                border-radius: 20px;
                                font-weight: 700;
                                min-width: 40px;
                                text-align: center;
                            }
                            .task-card {
                                background: #f7fafc;
                                border-radius: 10px;
                                padding: 20px;
                                margin-bottom: 15px;
                                border-left: 4px solid #e2e8f0;
                                transition: all 0.3s ease;
                            }
                            .task-card:hover {
                                transform: translateY(-2px);
                                box-shadow: 0 5px 15px rgba(0,0,0,0.1);
                            }
                            .task-card.in-progress { border-left-color: #3182ce; }
                            .task-card.completed { border-left-color: #38a169; }
                            .task-card.error { border-left-color: #e53e3e; }
                            .task-name {
                                font-weight: 700;
                                margin-bottom: 8px;
                                font-size: 1.1rem;
                            }
                            .task-message {
                                color: #718096;
                                margin-bottom: 12px;
                                font-size: 0.9rem;
                            }
                            .task-meta {
                                font-size: 0.8rem;
                                color: #a0aec0;
                            }
                            .task-actions {
                                margin-top: 15px;
                                display: flex;
                                gap: 10px;
                            }
                            .btn-cancel {
                                background: #e53e3e;
                                color: white;
                                padding: 6px 12px;
                                border: none;
                                border-radius: 6px;
                                cursor: pointer;
                                font-size: 0.8rem;
                            }
                            .empty-state {
                                text-align: center;
                                color: #a0aec0;
                                padding: 40px;
                                font-style: italic;
                            }
                            .error-banner {
                                background: linear-gradient(45deg, #fed7d7, #feb2b2);
                                color: #9b2c2c;
                                padding: 15px;
                                margin: 20px 0;
                                border-radius: 10px;
                                border-left: 5px solid #e53e3e;
                                font-weight: 600;
                                text-align: center;
                            }
                            .task-form {
                                background: rgba(255, 255, 255, 0.95);
                                border-radius: 15px;
                                padding: 25px;
                                margin-bottom: 20px;
                                box-shadow: 0 10px 30px rgba(0,0,0,0.1);
                            }
                            .form-group {
                                margin-bottom: 20px;
                            }
                            .form-group label {
                                display: block;
                                margin-bottom: 8px;
                                font-weight: 600;
                                color: #2d3748;
                            }
                            .form-group input, .form-group select {
                                width: 100%;
                                padding: 12px;
                                border: 2px solid #e2e8f0;
                                border-radius: 8px;
                                font-size: 16px;
                                transition: border-color 0.3s ease;
                            }
                            .form-group input:focus, .form-group select:focus {
                                outline: none;
                                border-color: #667eea;
                            }
                            .form-actions {
                                display: flex;
                                gap: 15px;
                                justify-content: center;
                            }
                            .btn-secondary {
                                background: #718096;
                                color: white;
                                padding: 12px 24px;
                                border: none;
                                border-radius: 8px;
                                cursor: pointer;
                                font-weight: 600;
                            }
                            .btn-primary {
                                background: #667eea;
                                color: white;
                                padding: 12px 24px;
                                border: none;
                                border-radius: 8px;
                                cursor: pointer;
                                font-weight: 600;
                            }
                            .modal {
                                display: none;
                                position: fixed;
                                z-index: 1000;
                                left: 0;
                                top: 0;
                                width: 100%;
                                height: 100%;
                                background-color: rgba(0,0,0,0.6);
                                backdrop-filter: blur(5px);
                            }
                            .modal.show {
                                display: flex;
                                align-items: center;
                                justify-content: center;
                            }
                            .modal-content {
                                background: white;
                                border-radius: 15px;
                                padding: 30px;
                                max-width: 500px;
                                width: 90%;
                                box-shadow: 0 20px 60px rgba(0,0,0,0.3);
                                animation: modalSlideIn 0.3s ease-out;
                            }
                            @keyframes modalSlideIn {
                                from {
                                    opacity: 0;
                                    transform: translateY(-50px) scale(0.9);
                                }
                                to {
                                    opacity: 1;
                                    transform: translateY(0) scale(1);
                                }
                            }
                            .modal-header {
                                margin-bottom: 25px;
                                text-align: center;
                            }
                            .modal-header h2 {
                                color: #2d3748;
                                margin-bottom: 10px;
                                font-size: 1.8rem;
                            }
                            .modal-header .task-type-badge {
                                display: inline-block;
                                padding: 6px 12px;
                                border-radius: 20px;
                                font-size: 0.9rem;
                                font-weight: 600;
                                text-transform: uppercase;
                            }
                            .badge-quick { background: linear-gradient(45deg, #4facfe, #00f2fe); color: white; }
                            .badge-long { background: linear-gradient(45deg, #fa709a, #fee140); color: white; }
                            .badge-error { background: linear-gradient(45deg, #ff6b6b, #ffa500); color: white; }
                            .badge-custom { background: linear-gradient(45deg, #667eea, #764ba2); color: white; }
                        "#))
                    }
                    (modal::render_modal_styles())
                }
                body {
                    div class="container" {
                        (shared_header::render_header())
                        div class="header" {
                            h1 { "Task Overlord LiveView" }
                            div class="controls" {
                                button class="btn btn-quick" onclick="openTaskModal('quick')" { "⚡ Quick Task (2s)" }
                                button class="btn btn-long" onclick="openTaskModal('long')" { "⏰ Long Task (10s)" }
                                button class="btn btn-error" onclick="openTaskModal('error')" { "💥 Error Task" }
                            }
                        }
                        div class="task-grid" id="task-grid" {
                            (self.render_task_column("In Progress", &self.get_tasks_by_status(TaskStatus::InProgress), "in-progress"))
                            (self.render_task_column("Completed", &self.get_tasks_by_status(TaskStatus::Completed), "completed"))
                            (self.render_task_column("Error", &self.get_tasks_by_status(TaskStatus::Error), "error"))
                        }

                        // Task Creation Modal (Server-Generated - will be updated by JS)
                        (modal::render_task_modal("custom"))
                    }
                    script src="/static/app.js" {}
                }
            }
        }
    }

    fn render_task_column(
        &self,
        title: &str,
        tasks: &[TaskMetadata],
        status_class: &str,
    ) -> Markup {
        html! {
            div class="task-column" {
                div class="column-header" {
                    div class={"column-title status-" (status_class)} { (title) }
                    div class="task-count" { (tasks.len()) }
                }
                div class="task-list" {
                    @if tasks.is_empty() {
                        div class="empty-state" { "No tasks yet..." }
                    } @else {
                        @for task in tasks {
                            (self.render_task_card(task))
                        }
                    }
                }
            }
        }
    }

    fn render_task_card(&self, task: &TaskMetadata) -> Markup {
        let status_class = match task.status {
            TaskStatus::InProgress => "in-progress",
            TaskStatus::Completed => "completed",
            TaskStatus::Error => "error",
        };

        html! {
            div class={"task-card " (status_class)} {
                div class="task-name" { (task.name) }
                div class="task-message" { (task.message) }
                div class="task-meta" {
                    div { "Started: " (task.started_at.format("%H:%M:%S")) }
                    @if let Some(finished_at) = task.finished_at {
                        div { "Finished: " (finished_at.format("%H:%M:%S")) }
                    }
                    @if let Some(duration) = task.actual_duration_ms {
                        div { "Duration: " (duration) "ms" }
                    }
                    @if let Some(result) = &task.result {
                        div { "Result: " (result) }
                    }
                    @if let Some(error) = &task.error {
                        div style="color: #e53e3e;" { "Error: " (error) }
                    }
                }
                @if task.status == TaskStatus::InProgress {
                    div class="task-actions" {
                        button class="btn-cancel" onclick={"cancelTask('" (task.id) "')"} { "Cancel" }
                    }
                }
            }
        }
    }

    fn get_tasks_by_status(&self, status: TaskStatus) -> Vec<TaskMetadata> {
        self.state
            .tasks
            .iter()
            .filter(|task| task.status == status)
            .cloned()
            .collect()
    }

    async fn update_tasks(&mut self, ctx: &mut ws::WebsocketContext<Self>) {
        match self.task_manager.send(GetAllTasks).await {
            Ok(tasks) => {
                self.state.tasks = tasks;
                self.send_html_update(ctx);
            }
            Err(e) => {
                println!("Failed to get tasks: {}", e);
            }
        }
    }

    fn render_task_grid(&self) -> Markup {
        html! {
            (self.render_task_column("In Progress", &self.get_tasks_by_status(TaskStatus::InProgress), "in-progress"))
            (self.render_task_column("Completed", &self.get_tasks_by_status(TaskStatus::Completed), "completed"))
            (self.render_task_column("Error", &self.get_tasks_by_status(TaskStatus::Error), "error"))
        }
    }

    fn send_html_update(&mut self, ctx: &mut ws::WebsocketContext<Self>) {
        let new_task_grid_html = self.render_task_grid().into_string();
        println!(
            "🔄 Session {} checking for updates: new={} chars, old={} chars",
            self.id,
            new_task_grid_html.len(),
            self.last_html.len()
        );

        if new_task_grid_html != self.last_html {
            println!(
                "📤 Session {} sending task grid update ({} chars)",
                self.id,
                new_task_grid_html.len()
            );
            let message = serde_json::json!({
                "type": "task_grid_update",
                "html": new_task_grid_html
            });

            let message_str = message.to_string();

            // Log outgoing message
            let ws_monitor = self.ws_monitor.clone();
            let session_id = self.id;
            let content_for_log = message_str.clone();
            let size_bytes = message_str.len();
            actix::spawn(async move {
                let _ = ws_monitor
                    .send(LogWebSocketMessage {
                        session_id,
                        direction: WsMessageDirection::Outgoing,
                        message_type: "task_grid_update".to_string(),
                        content: content_for_log,
                        size_bytes,
                    })
                    .await;
            });

            ctx.text(message_str);
            self.last_html = new_task_grid_html;
        } else {
            println!(
                "⏸️  Session {} no changes detected, skipping update",
                self.id
            );
        }
    }
}

impl Actor for LiveViewSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        println!("🔌 LiveView session started: {}", self.id);
        self.hb(ctx);

        // Get initial tasks to render
        println!("📋 Getting initial tasks for session {}", self.id);
        let task_manager = self.task_manager.clone();
        let ctx_addr = ctx.address();
        let session_id = self.id;

        actix::spawn(async move {
            match task_manager.send(GetAllTasks).await {
                Ok(tasks) => {
                    println!(
                        "✅ Initial tasks retrieved for session {}: {} tasks",
                        session_id,
                        tasks.len()
                    );
                    let _ = ctx_addr.send(UpdateTasks { tasks }).await;
                }
                Err(e) => {
                    println!(
                        "❌ Failed to get initial tasks for session {}: {}",
                        session_id, e
                    );
                }
            }
        });

        // Send initial task grid HTML for tracking
        let initial_task_grid_html = self.render_task_grid().into_string();
        self.last_html = initial_task_grid_html.clone();
        println!(
            "📤 Sending initial task grid HTML ({} chars) for session {}",
            initial_task_grid_html.len(),
            self.id
        );

        // Send the full page for first load
        let initial_full_html = self.render_page().into_string();
        let message = serde_json::json!({
            "type": "full_page_load",
            "html": initial_full_html
        });
        println!(
            "📤 Sending full page load ({} chars) for session {}",
            initial_full_html.len(),
            self.id
        );

        let message_str = message.to_string();

        // Log outgoing full page load message
        let ws_monitor = self.ws_monitor.clone();
        let session_id = self.id;
        let content_for_log = message_str.clone();
        let size_bytes = message_str.len();
        actix::spawn(async move {
            let _ = ws_monitor
                .send(LogWebSocketMessage {
                    session_id,
                    direction: WsMessageDirection::Outgoing,
                    message_type: "full_page_load".to_string(),
                    content: content_for_log,
                    size_bytes,
                })
                .await;
        });

        ctx.text(message_str);
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        println!("LiveView session stopped: {}", self.id);
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for LiveViewSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.hb = Instant::now();
            }
            Ok(ws::Message::Text(text)) => {
                self.hb = Instant::now();
                println!(
                    "📨 Session {} received WebSocket message: {}",
                    self.id, text
                );

                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&text) {
                    let msg_type = data
                        .get("type")
                        .and_then(|t| t.as_str())
                        .unwrap_or("unknown");
                    println!(
                        "📋 Session {} processing message type: {}",
                        self.id, msg_type
                    );

                    // Log incoming message
                    let ws_monitor = self.ws_monitor.clone();
                    let session_id = self.id;
                    let text_string = text.to_string();
                    let size_bytes = text.len();
                    let msg_type_string = msg_type.to_string();
                    actix::spawn(async move {
                        let _ = ws_monitor
                            .send(LogWebSocketMessage {
                                session_id,
                                direction: WsMessageDirection::Incoming,
                                message_type: msg_type_string,
                                content: text_string,
                                size_bytes,
                            })
                            .await;
                    });

                    match msg_type {
                        "create_task" | "create_custom_task" => {
                            if let Some(task_type_str) =
                                data.get("task_type").and_then(|t| t.as_str())
                            {
                                // Extract custom parameters for custom tasks
                                let task_name = data
                                    .get("name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("")
                                    .to_string();

                                let task_message = data
                                    .get("message")
                                    .and_then(|m| m.as_str())
                                    .unwrap_or("LiveView task")
                                    .to_string();

                                let task_type = match task_type_str {
                                    "quick" => TaskType::Quick { timeout_ms: None },
                                    "long" => TaskType::Long { timeout_ms: None },
                                    "error" => TaskType::Error {
                                        timeout_ms: None,
                                        error_type: ErrorType::Random,
                                    },
                                    "custom" => {
                                        let timeout_ms = data
                                            .get("custom_timeout")
                                            .and_then(|t| t.as_u64())
                                            .unwrap_or(5000);
                                        let failure_rate = data
                                            .get("custom_failure_rate")
                                            .and_then(|f| f.as_f64())
                                            .map(|f| f as f32);

                                        TaskType::Custom {
                                            name: if task_name.is_empty() {
                                                "Custom Task".to_string()
                                            } else {
                                                task_name.clone()
                                            },
                                            timeout_ms,
                                            failure_rate,
                                        }
                                    }
                                    _ => TaskType::Quick { timeout_ms: None },
                                };

                                println!(
                                    "🚀 Session {} creating task: {} (name: {}, message: {})",
                                    self.id, task_type_str, task_name, task_message
                                );
                                let task_manager = self.task_manager.clone();
                                let ctx_addr = ctx.address();
                                let session_id = self.id;

                                actix::spawn(async move {
                                    match task_manager
                                        .send(CreateTask {
                                            name: task_name,
                                            message: task_message,
                                            task_type,
                                        })
                                        .await
                                    {
                                        Ok(task_id) => {
                                            println!(
                                                "✅ Session {} task created with ID: {}",
                                                session_id, task_id
                                            );
                                            // Trigger a refresh after task creation
                                            tokio::time::sleep(tokio::time::Duration::from_millis(
                                                100,
                                            ))
                                            .await;
                                            if let Ok(tasks) = task_manager.send(GetAllTasks).await
                                            {
                                                println!("📋 Session {} refreshing with {} tasks after creation", session_id, tasks.len());
                                                let _ = ctx_addr.send(UpdateTasks { tasks }).await;
                                            }
                                        }
                                        Err(e) => {
                                            println!(
                                                "❌ Session {} failed to create task: {}",
                                                session_id, e
                                            );
                                        }
                                    }
                                });
                            } else {
                                println!(
                                    "⚠️  Session {} create_task message missing task_type",
                                    self.id
                                );
                            }
                        }
                        "cancel_task" => {
                            if let Some(task_id_str) = data.get("task_id").and_then(|t| t.as_str())
                            {
                                if let Ok(task_id) = Uuid::parse_str(task_id_str) {
                                    println!("🗑️  Session {} canceling task: {}", self.id, task_id);
                                    let task_manager = self.task_manager.clone();
                                    let session_id = self.id;

                                    let ctx_addr = ctx.address();
                                    actix::spawn(async move {
                                        match task_manager
                                            .send(CancelTaskById { id: task_id })
                                            .await
                                        {
                                            Ok(canceled) => {
                                                println!(
                                                    "✅ Session {} task cancellation result: {}",
                                                    session_id, canceled
                                                );
                                                // Trigger a refresh after task cancellation
                                                tokio::time::sleep(
                                                    tokio::time::Duration::from_millis(100),
                                                )
                                                .await;
                                                if let Ok(tasks) =
                                                    task_manager.send(GetAllTasks).await
                                                {
                                                    println!("📋 Session {} refreshing with {} tasks after cancellation", session_id, tasks.len());
                                                    let _ =
                                                        ctx_addr.send(UpdateTasks { tasks }).await;
                                                }
                                            }
                                            Err(e) => {
                                                println!(
                                                    "❌ Session {} failed to cancel task: {}",
                                                    session_id, e
                                                );
                                            }
                                        }
                                    });
                                } else {
                                    println!("⚠️  Session {} cancel_task message has invalid task_id: {}", self.id, task_id_str);
                                }
                            } else {
                                println!(
                                    "⚠️  Session {} cancel_task message missing task_id",
                                    self.id
                                );
                            }
                        }
                        "refresh" => {
                            println!("🔄 Session {} processing refresh request", self.id);
                            let ctx_addr = ctx.address();
                            let task_manager = self.task_manager.clone();
                            let session_id = self.id;

                            actix::spawn(async move {
                                match task_manager.send(GetAllTasks).await {
                                    Ok(tasks) => {
                                        println!(
                                            "📋 Session {} refresh found {} tasks",
                                            session_id,
                                            tasks.len()
                                        );
                                        let _ = ctx_addr.send(UpdateTasks { tasks }).await;
                                    }
                                    Err(e) => {
                                        println!("❌ Session {} refresh failed: {}", session_id, e);
                                    }
                                }
                            });
                        }
                        _ => {
                            println!("⚠️  Session {} unknown message type: {}", self.id, msg_type);
                        }
                    }
                } else {
                    println!(
                        "❌ Session {} failed to parse JSON message: {}",
                        self.id, text
                    );
                }
            }
            Ok(ws::Message::Binary(_)) => println!("Unexpected binary message"),
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            _ => ctx.stop(),
        }
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct UpdateTasks {
    pub tasks: Vec<TaskMetadata>,
}

impl Handler<UpdateTasks> for LiveViewSession {
    type Result = ();

    fn handle(&mut self, msg: UpdateTasks, ctx: &mut Self::Context) {
        println!(
            "📝 Session {} updating tasks: received {} tasks",
            self.id,
            msg.tasks.len()
        );

        // Log task details for debugging
        for task in &msg.tasks {
            println!("   Task: {} - {} ({:?})", task.id, task.name, task.status);
        }

        self.state.tasks = msg.tasks;
        println!("📤 Session {} triggering HTML update", self.id);
        self.send_html_update(ctx);
    }
}

pub async fn websocket_handler(
    req: actix_web::HttpRequest,
    stream: web::Payload,
    data: web::Data<crate::AppState>,
) -> Result<actix_web::HttpResponse, actix_web::Error> {
    let session = LiveViewSession::new(data.task_manager.clone(), data.ws_monitor.clone());
    ws::start(session, &req, stream)
}
