use maud::{html, Markup, PreEscaped};
use task_core::TaskType;

#[derive(Debug, Clone)]
pub struct TaskTypeConfig {
    pub name: String,
    pub badge_class: String,
    pub placeholder: String,
    pub description: String,
    pub default_timeout: u64,
    pub has_custom_options: bool,
}

impl TaskTypeConfig {
    pub fn get_config(task_type: &str) -> Self {
        match task_type {
            "quick" => Self {
                name: "Quick Task".to_string(),
                badge_class: "badge-quick".to_string(),
                placeholder: "Quick task message (completes in ~2s)".to_string(),
                description: "A fast task that completes in approximately 2 seconds".to_string(),
                default_timeout: 2000,
                has_custom_options: false,
            },
            "long" => Self {
                name: "Long Task".to_string(),
                badge_class: "badge-long".to_string(),
                placeholder: "Long task message (completes in ~10s)".to_string(),
                description: "A longer task that takes approximately 10 seconds to complete"
                    .to_string(),
                default_timeout: 10000,
                has_custom_options: false,
            },
            "error" => Self {
                name: "Error Task".to_string(),
                badge_class: "badge-error".to_string(),
                placeholder: "Error task message (will fail for testing)".to_string(),
                description: "A task designed to fail for testing error handling".to_string(),
                default_timeout: 5000,
                has_custom_options: false,
            },
            "custom" => Self {
                name: "Custom Task".to_string(),
                badge_class: "badge-custom".to_string(),
                placeholder: "Custom task message".to_string(),
                description: "A fully customizable task with configurable timeout and failure rate"
                    .to_string(),
                default_timeout: 5000,
                has_custom_options: true,
            },
            _ => Self::get_config("quick"),
        }
    }
}

pub fn render_modal_styles() -> Markup {
    html! {
        style {
            (PreEscaped(r#"
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
                    max-height: 80vh;
                    overflow-y: auto;
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
                    margin-bottom: 15px;
                    font-size: 1.8rem;
                }
                .task-type-badge {
                    display: inline-block;
                    padding: 8px 16px;
                    border-radius: 20px;
                    font-size: 0.9rem;
                    font-weight: 600;
                    text-transform: uppercase;
                    margin-bottom: 10px;
                }
                .badge-quick { background: linear-gradient(45deg, #4facfe, #00f2fe); color: white; }
                .badge-long { background: linear-gradient(45deg, #fa709a, #fee140); color: white; }
                .badge-error { background: linear-gradient(45deg, #ff6b6b, #ffa500); color: white; }
                .badge-custom { background: linear-gradient(45deg, #667eea, #764ba2); color: white; }
                .task-description {
                    color: #718096;
                    font-size: 0.9rem;
                    text-align: center;
                    margin-bottom: 20px;
                    line-height: 1.4;
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
                    box-shadow: 0 0 0 3px rgba(102, 126, 234, 0.1);
                }
                .form-actions {
                    display: flex;
                    gap: 15px;
                    justify-content: center;
                    margin-top: 30px;
                }
                .btn-secondary {
                    background: #718096;
                    color: white;
                    padding: 12px 24px;
                    border: none;
                    border-radius: 8px;
                    cursor: pointer;
                    font-weight: 600;
                    transition: background 0.3s ease;
                }
                .btn-secondary:hover {
                    background: #4a5568;
                }
                .btn-primary {
                    background: #667eea;
                    color: white;
                    padding: 12px 24px;
                    border: none;
                    border-radius: 8px;
                    cursor: pointer;
                    font-weight: 600;
                    transition: background 0.3s ease;
                }
                .btn-primary:hover {
                    background: #5a6fd8;
                }
                .custom-options {
                    background: #f7fafc;
                    border-radius: 10px;
                    padding: 20px;
                    margin-top: 15px;
                    border-left: 4px solid #667eea;
                }
                .custom-options h4 {
                    color: #2d3748;
                    margin-bottom: 15px;
                    font-size: 1.1rem;
                }
            "#))
        }
    }
}

pub fn render_task_modal(task_type: &str) -> Markup {
    let config = TaskTypeConfig::get_config(task_type);

    html! {
        div class="modal" id="task-modal" onclick="closeModalOnBackdrop(event)" data-task-type=(task_type) {
            div class="modal-content" {
                div class="modal-header" {
                    h2 { "Create Task" }
                    div class={"task-type-badge " (config.badge_class)} id="modal-task-type-badge" { (config.name) }
                    div class="task-description" id="modal-task-description" { (config.description) }
                }
                form id="task-form" onsubmit="createCustomTask(event)" data-task-type=(task_type) {
                    div class="form-group" {
                        label for="task-name" { "Task Name (optional):" }
                        input type="text" id="task-name" placeholder="Leave empty for auto-generated name" maxlength="100";
                    }
                    div class="form-group" {
                        label for="task-message" { "Message:" }
                        input type="text" id="task-message" placeholder=(config.placeholder) required maxlength="500";
                    }

                    // Always include custom options but hide them by default
                    div class="custom-options" style="display: none;" {
                        h4 { "Advanced Options" }
                        div class="form-group" {
                            label for="custom-timeout" { "Timeout (milliseconds):" }
                            input type="number" id="custom-timeout" value="5000" min="100" max="300000";
                            small style="color: #718096;" { "Maximum: 5 minutes (300000ms)" }
                        }
                        div class="form-group" {
                            label for="custom-failure-rate" { "Failure Rate:" }
                            input type="number" id="custom-failure-rate" value="0" min="0" max="1" step="0.1";
                            small style="color: #718096;" { "0.0 = never fails, 1.0 = always fails" }
                        }
                    }

                    div class="form-actions" {
                        button type="button" onclick="closeModal()" class="btn btn-secondary" { "Cancel" }
                        button type="submit" class="btn btn-primary" { "Create " (config.name) }
                    }
                }
            }
        }
    }
}
