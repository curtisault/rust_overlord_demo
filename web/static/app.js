/**
 * WebSocket LiveView Connection Manager with OpenTelemetry
 *
 * This module handles the WebSocket connection for the LiveView system, providing
 * real-time DOM updates without full page refreshes, similar to Phoenix LiveView.
 *
 * Features:
 * - Persistent WebSocket connection with automatic reconnection
 * - DOM diffing and selective updates to maintain state
 * - Task creation and management through WebSocket messages
 * - Automatic refresh polling every 2 seconds
 * - Full page load on initial connection, partial updates thereafter
 * - OpenTelemetry instrumentation for comprehensive telemetry tracking
 *
 * Message Types:
 * - full_page_load: Complete HTML document replacement (initial load only)
 * - task_grid_update: Partial update of task grid content (DOM diffing)
 * - create_task: Create a new task (outbound)
 * - cancel_task: Cancel an existing task (outbound)
 * - refresh: Request current task state (outbound)
 *
 * DOM Diffing Strategy:
 * - Compares new HTML with existing DOM elements
 * - Updates only changed content (task lists, counts)
 * - Preserves WebSocket connection and JavaScript state
 * - Updates task columns independently for efficiency
 *
 * Telemetry:
 * - Tracks all WebSocket operations with OpenTelemetry
 * - Records user interactions, connection events, and DOM updates
 * - Exports traces to console (can be configured for real exporters)
 *
 * Usage:
 *   Include OpenTelemetry CDN scripts before this file
 *   The connection will be established automatically and handle all updates.
 *
 * Author: Claude Code Assistant
 * Compatible with: Modern browsers with WebSocket and OpenTelemetry support
 */

// Global WebSocket connection and telemetry
window.ws = null;
let tracer = null;
let isConnecting = false;

// Initialize OpenTelemetry if available
function initTelemetry() {
    if (typeof opentelemetry !== 'undefined') {
        const { NodeSDK } = opentelemetry;

        // Create a tracer
        tracer = opentelemetry.trace.getTracer('liveview-websocket', '1.0.0');
        console.log('📊 [TELEMETRY] OpenTelemetry initialized');

        return true;
    } else {
        console.warn('📊 [TELEMETRY] OpenTelemetry not available, using console logging');
        return false;
    }
}

// Create a span for telemetry tracking
function createSpan(name, attributes = {}) {
    if (tracer) {
        const span = tracer.startSpan(name, {
            attributes: {
                'service.name': 'liveview-client',
                'service.version': '1.0.0',
                ...attributes
            }
        });
        return span;
    }
    return null;
}

// Log telemetry event (fallback when OpenTelemetry not available)
function logTelemetryEvent(eventType, data) {
    const timestamp = new Date().toISOString();
    console.log(`📊 [TELEMETRY] ${timestamp} - ${eventType}:`, data);
}

/**
 * Initialize WebSocket connection to the LiveView server
 * Sets up message handlers and connection management
 */
function initWebSocketConnection() {
    // Initialize telemetry
    initTelemetry();

    // Create span for connection initialization
    const span = createSpan('websocket.init', {
        'websocket.url': location.host + '/ws/'
    });

    logTelemetryEvent('CONNECTION_INIT', {
        url: location.host + '/ws/',
        timestamp: Date.now()
    });

    connect();

    if (span) span.end();
}

/**
 * Establish WebSocket connection with automatic reconnection
 * Handles both initial page loads and subsequent partial updates
 */
function connect() {
    // Prevent multiple connection attempts
    if (isConnecting || (window.ws && window.ws.readyState === WebSocket.CONNECTING)) {
        logTelemetryEvent('CONNECTION_SKIPPED', { reason: 'already_connecting' });
        return;
    }

    // Don't reconnect if already connected
    if (window.ws && window.ws.readyState === WebSocket.OPEN) {
        logTelemetryEvent('CONNECTION_SKIPPED', { reason: 'already_connected' });
        return;
    }

    isConnecting = true;

    const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = protocol + '//' + location.host + '/ws/';

    const connectSpan = createSpan('websocket.connect', {
        'websocket.url': wsUrl,
        'websocket.protocol': protocol
    });

    logTelemetryEvent('WEBSOCKET_CONNECTING', { url: wsUrl });

    window.ws = new WebSocket(wsUrl);

    window.ws.onopen = function() {
        isConnecting = false; // Reset connecting flag

        logTelemetryEvent('WEBSOCKET_CONNECTED', {
            url: wsUrl,
            readyState: window.ws.readyState
        });

        if (connectSpan) {
            connectSpan.setAttributes({
                'websocket.connection_status': 'connected'
            });
            connectSpan.end();
        }
    };

    window.ws.onmessage = function(event) {
        const messageSpan = createSpan('websocket.message_received');

        try {
            const data = JSON.parse(event.data);

            logTelemetryEvent('WEBSOCKET_MESSAGE_RECEIVED', {
                type: data.type,
                size: event.data.length,
                timestamp: Date.now()
            });

            if (messageSpan) {
                messageSpan.setAttributes({
                    'message.type': data.type,
                    'message.size': event.data.length
                });
            }

            if (data.type === 'full_page_load') {
                logTelemetryEvent('FULL_PAGE_LOAD', {
                    htmlSize: data.html?.length || 0
                });

                // Initial connection - replace entire document
                // Mark that we're doing a full page replacement to avoid reconnection loop
                window.fullPageReplacement = true;
                document.documentElement.innerHTML = data.html;
                // Don't reconnect - the new page will have its own script that will connect

            } else if (data.type === 'task_grid_update') {
                logTelemetryEvent('TASK_GRID_UPDATE', {
                    htmlSize: data.html?.length || 0
                });

                // Partial update - use DOM diffing to update only changed content
                updateTaskGrid(data.html);
            }

        } catch (error) {
            logTelemetryEvent('MESSAGE_PARSE_ERROR', {
                error: error.message,
                rawMessage: event.data
            });

            if (messageSpan) {
                messageSpan.recordException(error);
                messageSpan.setStatus({
                    code: 2, // ERROR
                    message: error.message
                });
            }
        } finally {
            if (messageSpan) messageSpan.end();
        }
    };

    window.ws.onclose = function(event) {
        isConnecting = false; // Reset connecting flag

        logTelemetryEvent('WEBSOCKET_CLOSED', {
            code: event.code,
            reason: event.reason,
            wasClean: event.wasClean
        });

        if (connectSpan) {
            connectSpan.setAttributes({
                'websocket.connection_status': 'closed',
                'websocket.close_code': event.code
            });
            connectSpan.end();
        }

        // Only reconnect if it's not due to a full page replacement
        if (!window.fullPageReplacement) {
            logTelemetryEvent('WEBSOCKET_RECONNECTING', { reason: 'connection_lost' });
            setTimeout(connect, 1000);
        } else {
            logTelemetryEvent('WEBSOCKET_NOT_RECONNECTING', { reason: 'full_page_replacement' });
        }
    };

    window.ws.onerror = function(error) {
        isConnecting = false; // Reset connecting flag

        logTelemetryEvent('WEBSOCKET_ERROR', {
            error: error.type || 'unknown'
        });

        if (connectSpan) {
            connectSpan.setAttributes({
                'websocket.connection_status': 'error'
            });
            connectSpan.recordException(new Error('WebSocket connection error'));
            connectSpan.end();
        }

        // Only reconnect on error if it's not due to a full page replacement
        if (!window.fullPageReplacement) {
            logTelemetryEvent('WEBSOCKET_RECONNECTING', { reason: 'error' });
            setTimeout(connect, 1000);
        }
    };
}

/**
 * Update the task grid using DOM diffing
 * Compares new HTML with existing content and updates only what changed
 *
 * @param {string} newHtml - New HTML content for the task grid
 */
function updateTaskGrid(newHtml) {
    const taskGrid = document.getElementById('task-grid');
    if (!taskGrid || !newHtml) return;

    // Parse the new HTML content
    const tempDiv = document.createElement('div');
    tempDiv.innerHTML = newHtml;

    // Update each task column independently
    const newColumns = tempDiv.querySelectorAll('.task-column');
    newColumns.forEach((newColumn, index) => {
        const existingColumn = taskGrid.children[index];
        if (existingColumn && newColumn) {

            // Update task count badge if changed
            const newCount = newColumn.querySelector('.task-count');
            const existingCount = existingColumn.querySelector('.task-count');
            if (newCount && existingCount && newCount.textContent !== existingCount.textContent) {
                existingCount.textContent = newCount.textContent;
            }

            // Update task list content if changed
            const newTaskList = newColumn.querySelector('.task-list');
            const existingTaskList = existingColumn.querySelector('.task-list');
            if (newTaskList && existingTaskList && newTaskList.innerHTML !== existingTaskList.innerHTML) {
                existingTaskList.innerHTML = newTaskList.innerHTML;
            }
        }
    });
}

/**
 * Create a new task via WebSocket
 * Sends a task creation message to the server
 *
 * @param {string} taskType - Type of task to create ('quick', 'long', 'error')
 */
function createTask(taskType) {
    const span = createSpan('user.create_task', {
        'task.type': taskType,
        'user.action': 'button_click'
    });

    logTelemetryEvent('USER_CREATE_TASK_CLICKED', {
        taskType: taskType,
        timestamp: Date.now(),
        buttonId: `create-${taskType}-task`
    });

    if (window.ws && window.ws.readyState === WebSocket.OPEN) {
        const message = {
            type: 'create_task',
            task_type: taskType
        };

        logTelemetryEvent('WEBSOCKET_MESSAGE_SENT', {
            type: 'create_task',
            taskType: taskType,
            messageSize: JSON.stringify(message).length
        });

        window.ws.send(JSON.stringify(message));

        if (span) {
            span.setAttributes({
                'websocket.message_sent': true,
                'websocket.ready_state': window.ws.readyState
            });
            span.end();
        }
    } else {
        const errorMessage = `WebSocket not connected (state: ${window.ws ? window.ws.readyState : 'null'})`;

        logTelemetryEvent('WEBSOCKET_SEND_ERROR', {
            action: 'create_task',
            taskType: taskType,
            error: errorMessage,
            wsState: window.ws ? window.ws.readyState : null
        });

        if (span) {
            span.recordException(new Error(errorMessage));
            span.setStatus({
                code: 2, // ERROR
                message: errorMessage
            });
            span.end();
        }

        console.error(`❌ [TELEMETRY] ${errorMessage} - cannot create ${taskType} task`);
    }
}

/**
 * Cancel an existing task via WebSocket
 * Sends a task cancellation message to the server
 *
 * @param {string} taskId - UUID of the task to cancel
 */
function cancelTask(taskId) {
    const span = createSpan('user.cancel_task', {
        'task.id': taskId,
        'user.action': 'button_click'
    });

    logTelemetryEvent('USER_CANCEL_TASK_CLICKED', {
        taskId: taskId,
        timestamp: Date.now()
    });

    if (window.ws && window.ws.readyState === WebSocket.OPEN) {
        const message = {
            type: 'cancel_task',
            task_id: taskId
        };

        logTelemetryEvent('WEBSOCKET_MESSAGE_SENT', {
            type: 'cancel_task',
            taskId: taskId,
            messageSize: JSON.stringify(message).length
        });

        window.ws.send(JSON.stringify(message));

        if (span) {
            span.setAttributes({
                'websocket.message_sent': true,
                'websocket.ready_state': window.ws.readyState
            });
            span.end();
        }
    } else {
        const errorMessage = `WebSocket not connected (state: ${window.ws ? window.ws.readyState : 'null'})`;

        logTelemetryEvent('WEBSOCKET_SEND_ERROR', {
            action: 'cancel_task',
            taskId: taskId,
            error: errorMessage,
            wsState: window.ws ? window.ws.readyState : null
        });

        if (span) {
            span.recordException(new Error(errorMessage));
            span.setStatus({
                code: 2, // ERROR
                message: errorMessage
            });
            span.end();
        }

        console.error(`❌ [TELEMETRY] ${errorMessage} - cannot cancel task ${taskId}`);
    }
}

/**
 * Request a refresh of current task state
 * Triggers a server-side task list update
 */
function refreshTasks() {
    if (window.ws && window.ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: 'refresh' }));
    }
}

/**
 * Start automatic refresh polling
 * Requests task updates every 2 seconds to keep UI in sync
 */
function startAutoRefresh() {
    setInterval(() => {
        refreshTasks();
    }, 2000);
}

// Auto-initialize when script loads
if (typeof window !== 'undefined') {
    // Start WebSocket connection
    initWebSocketConnection();

    // Start automatic refresh polling
    startAutoRefresh();

    // Export functions to global scope for HTML onclick handlers
    window.createTask = createTask;
    window.cancelTask = cancelTask;
    window.refreshTasks = refreshTasks;
}