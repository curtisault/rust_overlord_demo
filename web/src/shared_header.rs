use maud::{html, Markup, PreEscaped};

pub fn render_header() -> Markup {
    html! {
        style {
            (PreEscaped(r#"
                .top-header {
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                    padding: 15px 30px;
                    background: rgba(255, 255, 255, 0.95);
                    border-radius: 15px;
                    margin-bottom: 20px;
                    box-shadow: 0 5px 15px rgba(0,0,0,0.1);
                }
                .header-left {
                    display: flex;
                    gap: 15px;
                    align-items: center;
                }
                .header-right {
                    display: flex;
                    gap: 15px;
                    align-items: center;
                }
                .nav-link {
                    color: #2d3748;
                    text-decoration: none;
                    font-weight: 600;
                    font-size: 1.1rem;
                    padding: 8px 16px;
                    border-radius: 8px;
                    transition: all 0.3s ease;
                }
                .nav-link:hover {
                    background: #f7fafc;
                    color: #3182ce;
                }
                .ws-status {
                    display: flex;
                    align-items: center;
                    gap: 8px;
                    font-size: 0.9rem;
                    font-weight: 600;
                }
                .status-indicator {
                    width: 12px;
                    height: 12px;
                    border-radius: 50%;
                    animation: pulse 2s infinite;
                }
                .status-connected { background: #38a169; }
                .status-connecting { background: #d69e2e; }
                .status-disconnected { background: #e53e3e; }
                .status-offline { background: #718096; }
                @keyframes pulse {
                    0% { opacity: 1; }
                    50% { opacity: 0.5; }
                    100% { opacity: 1; }
                }
            "#))
        }
        div class="top-header" {
            div class="header-left" {
                a class="nav-link" href="/" { "🏠 Home" }
                a class="nav-link" href="/monitor" { "📡 Monitor" }
            }
            div class="header-right" {
                div class="ws-status" id="ws-status" {
                    div class="status-indicator status-connecting" id="status-indicator" {}
                    span id="status-text" { "Connecting..." }
                }
            }
        }
    }
}
