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
let connectionAttempts = 0;
let maxReconnectAttempts = 10;
let reconnectDelay = 1000; // Start with 1 second
let isOfflineMode = false;

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
    updateConnectionStatus('connecting', 'Connecting...');

    window.ws = new WebSocket(wsUrl);

    window.ws.onopen = function() {
        isConnecting = false; // Reset connecting flag
        connectionAttempts = 0; // Reset on successful connection

        logTelemetryEvent('WEBSOCKET_CONNECTED', {
            url: wsUrl,
            readyState: window.ws.readyState
        });

        updateConnectionStatus('connected', 'Connected');

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

                // Extract body content from the full HTML and update only the body
                // This preserves the JavaScript context and WebSocket connection
                const parser = new DOMParser();
                const doc = parser.parseFromString(data.html, 'text/html');
                const newBody = doc.body;

                if (newBody) {
                    // Replace body content while preserving head scripts
                    document.body.innerHTML = newBody.innerHTML;

                    // Update the status to connected since we successfully processed the full page
                    updateConnectionStatus('connected', 'Connected');
                } else {
                    console.error('Failed to parse full page HTML');
                }

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
        connectionAttempts++;

        logTelemetryEvent('WEBSOCKET_CLOSED', {
            code: event.code,
            reason: event.reason,
            wasClean: event.wasClean,
            attempt: connectionAttempts
        });

        if (connectSpan) {
            connectSpan.setAttributes({
                'websocket.connection_status': 'closed',
                'websocket.close_code': event.code,
                'websocket.attempt': connectionAttempts
            });
            connectSpan.end();
        }

        // Only reconnect if it's not due to a full page replacement
        if (!window.fullPageReplacement) {
            updateConnectionStatus('disconnected', 'Disconnected');
            handleReconnection();
        } else {
            logTelemetryEvent('WEBSOCKET_NOT_RECONNECTING', { reason: 'full_page_replacement' });
        }
    };

    window.ws.onerror = function(error) {
        isConnecting = false; // Reset connecting flag
        connectionAttempts++;

        logTelemetryEvent('WEBSOCKET_ERROR', {
            error: error.type || 'unknown',
            attempt: connectionAttempts
        });

        if (connectSpan) {
            connectSpan.setAttributes({
                'websocket.connection_status': 'error',
                'websocket.attempt': connectionAttempts
            });
            connectSpan.recordException(new Error('WebSocket connection error'));
            connectSpan.end();
        }

        // Only reconnect on error if it's not due to a full page replacement
        if (!window.fullPageReplacement) {
            handleReconnection();
        }
    };
}

/**
 * Handle WebSocket reconnection with exponential backoff and fallback
 */
function handleReconnection() {
    if (connectionAttempts >= maxReconnectAttempts) {
        logTelemetryEvent('WEBSOCKET_MAX_RETRIES_EXCEEDED', {
            attempts: connectionAttempts,
            switching_to: 'offline_mode'
        });
        switchToOfflineMode();
        return;
    }

    // Exponential backoff with jitter
    const delay = Math.min(reconnectDelay * Math.pow(2, connectionAttempts), 30000);
    const jitteredDelay = delay + (Math.random() * 1000);

    logTelemetryEvent('WEBSOCKET_RECONNECTING', {
        reason: 'connection_lost',
        attempt: connectionAttempts,
        delay_ms: jitteredDelay
    });

    updateConnectionStatus('connecting', `Reconnecting (${connectionAttempts}/${maxReconnectAttempts})...`);

    setTimeout(() => {
        connect();
    }, jitteredDelay);
}

/**
 * Switch to offline mode with REST API fallback
 */
function switchToOfflineMode() {
    isOfflineMode = true;
    updateConnectionStatus('offline', 'Offline Mode');
    showConnectionStatus('⚠️ WebSocket unavailable - Using polling fallback', 'warning');

    logTelemetryEvent('OFFLINE_MODE_ACTIVATED', {
        reason: 'websocket_failed',
        fallback: 'rest_api_polling'
    });

    // Start polling every 5 seconds as fallback
    startRestApiPolling();
}

/**
 * Start REST API polling as WebSocket fallback
 */
function startRestApiPolling() {
    const pollInterval = 5000; // 5 seconds

    const poll = async () => {
        try {
            const response = await fetch('/api/tasks');
            if (!response.ok) {
                throw new Error(`HTTP ${response.status}: ${response.statusText}`);
            }

            const data = await response.json();
            if (data.success && data.data) {
                updateTaskGridFromRestData(data.data.tasks);
                updateConnectionStatus('offline', `Polling (${data.data.tasks.length} tasks)`);
            }
        } catch (error) {
            logTelemetryEvent('REST_API_POLL_ERROR', {
                error: error.message,
                mode: 'offline_polling'
            });
            updateConnectionStatus('disconnected', 'Server Unavailable');
            showConnectionStatus('❌ Server unavailable - Retrying...', 'error');
        }

        // Continue polling only if still in offline mode
        if (isOfflineMode) {
            setTimeout(poll, pollInterval);
        }
    };

    // Start first poll immediately
    poll();
}

/**
 * Update the header connection status
 */
function updateConnectionStatus(status, message) {
    const statusIndicator = document.getElementById('status-indicator');
    const statusText = document.getElementById('status-text');

    if (statusIndicator && statusText) {
        // Remove existing status classes
        statusIndicator.className = statusIndicator.className.replace(/status-\w+/g, '');
        statusIndicator.className += ` status-indicator status-${status}`;
        statusText.textContent = message;
    }
}

/**
 * Show temporary connection status message to user (fallback for important messages)
 */
function showConnectionStatus(message, type = 'info') {
    // For important messages, still show floating notifications
    if (type === 'error' || type === 'warning') {
        let statusEl = document.getElementById('temp-connection-status');
        if (!statusEl) {
            statusEl = document.createElement('div');
            statusEl.id = 'temp-connection-status';
            statusEl.style.cssText = `
                position: fixed;
                top: 70px;
                right: 10px;
                padding: 10px 15px;
                border-radius: 5px;
                font-size: 12px;
                font-weight: 600;
                z-index: 1000;
                max-width: 300px;
            `;
            document.body.appendChild(statusEl);
        }

        const colors = {
            info: 'background: #3182ce; color: white;',
            warning: 'background: #d69e2e; color: white;',
            error: 'background: #e53e3e; color: white;'
        };

        statusEl.textContent = message;
        statusEl.style.cssText += colors[type] || colors.info;
        statusEl.style.display = 'block';

        // Auto-hide after 5 seconds
        setTimeout(() => {
            if (statusEl && statusEl.style.display !== 'none') {
                statusEl.style.display = 'none';
            }
        }, 5000);
    }
}

/**
 * Update task grid from REST API data (fallback mode)
 */
function updateTaskGridFromRestData(tasks) {
    const taskGrid = document.getElementById('task-grid');
    if (!taskGrid || !tasks) return;

    // Group tasks by status
    const tasksByStatus = {
        'InProgress': tasks.filter(t => t.status === 'InProgress'),
        'Completed': tasks.filter(t => t.status === 'Completed'),
        'Error': tasks.filter(t => t.status === 'Error')
    };

    // Update each column
    const columns = taskGrid.querySelectorAll('.task-column');
    const statusNames = ['InProgress', 'Completed', 'Error'];

    columns.forEach((column, index) => {
        const status = statusNames[index];
        const statusTasks = tasksByStatus[status] || [];

        // Update count
        const countEl = column.querySelector('.task-count');
        if (countEl) countEl.textContent = statusTasks.length;

        // Update task list (simplified for REST mode)
        const taskList = column.querySelector('.task-list');
        if (taskList) {
            if (statusTasks.length === 0) {
                taskList.innerHTML = '<div class="empty-state">No tasks yet...</div>';
            } else {
                taskList.innerHTML = statusTasks.map(task => `
                    <div class="task-card ${status.toLowerCase()}">
                        <div class="task-name">${task.name}</div>
                        <div class="task-message">${task.message}</div>
                        <div class="task-meta">
                            <div>Started: ${new Date(task.started_at).toLocaleTimeString()}</div>
                            ${task.finished_at ? `<div>Finished: ${new Date(task.finished_at).toLocaleTimeString()}</div>` : ''}
                            ${task.actual_duration_ms ? `<div>Duration: ${task.actual_duration_ms}ms</div>` : ''}
                            ${task.result ? `<div>Result: ${task.result}</div>` : ''}
                            ${task.error ? `<div style="color: #e53e3e;">Error: ${task.error}</div>` : ''}
                        </div>
                        ${status === 'InProgress' ? `
                            <div class="task-actions">
                                <button class="btn-cancel" onclick="cancelTaskRestMode('${task.id}')">Cancel</button>
                            </div>
                        ` : ''}
                    </div>
                `).join('');
            }
        }
    });
}

/**
 * Cancel task using REST API (fallback mode)
 */
async function cancelTaskRestMode(taskId) {
    try {
        const response = await fetch(`/api/tasks/${taskId}`, { method: 'DELETE' });
        const data = await response.json();

        if (!data.success) {
            showConnectionStatus(`❌ Cancel failed: ${data.error?.message || 'Unknown error'}`, 'error');
        } else {
            showConnectionStatus('✅ Task cancelled successfully', 'info');
        }
    } catch (error) {
        logTelemetryEvent('REST_CANCEL_ERROR', {
            taskId: taskId,
            error: error.message
        });
        showConnectionStatus(`❌ Cancel failed: ${error.message}`, 'error');
    }
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
 * Create a new task via WebSocket or REST API fallback
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

    // Try WebSocket first
    if (window.ws && window.ws.readyState === WebSocket.OPEN && !isOfflineMode) {
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
        // Fallback to REST API
        createTaskRestMode(taskType, span);
    }
}

/**
 * Create task using REST API (fallback mode)
 */
async function createTaskRestMode(taskType, span) {
    try {
        const response = await fetch('/api/tasks', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                name: '',
                message: 'Task created via REST API fallback',
                task_type: {
                    type: taskType,
                    timeout_ms: taskType === 'quick' ? 2000 : taskType === 'long' ? 10000 : 5000
                }
            })
        });

        const data = await response.json();

        if (!data.success) {
            const errorMsg = data.error?.message || 'Unknown error occurred';
            showConnectionStatus(`❌ Task creation failed: ${errorMsg}`, 'error');

            logTelemetryEvent('REST_CREATE_TASK_ERROR', {
                taskType: taskType,
                error: errorMsg,
                errorCode: data.error?.error_code
            });
        } else {
            showConnectionStatus(`✅ ${taskType} task created successfully`, 'info');

            logTelemetryEvent('REST_CREATE_TASK_SUCCESS', {
                taskType: taskType,
                taskId: data.data?.id
            });
        }

        if (span) {
            span.setAttributes({
                'rest.api_used': true,
                'rest.success': data.success
            });
            span.end();
        }
    } catch (error) {
        const errorMessage = `Failed to create ${taskType} task: ${error.message}`;
        showConnectionStatus(`❌ ${errorMessage}`, 'error');

        logTelemetryEvent('REST_CREATE_TASK_NETWORK_ERROR', {
            taskType: taskType,
            error: error.message
        });

        if (span) {
            span.recordException(error);
            span.setStatus({
                code: 2, // ERROR
                message: errorMessage
            });
            span.end();
        }
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
 * Create a custom task with form inputs
 */
function createCustomTask(event) {
    event.preventDefault();

    const taskName = document.getElementById('task-name').value.trim();
    const taskMessage = document.getElementById('task-message').value.trim();
    const modal = document.getElementById('task-modal');
    const taskType = modal.dataset.taskType || 'quick';

    if (!taskMessage) {
        alert('Please enter a task message');
        return;
    }

    // Build the message object based on task type
    let message = {
        type: 'create_custom_task',
        name: taskName,
        message: taskMessage,
        task_type: taskType
    };

    // Add custom options if custom task type is selected
    if (taskType === 'custom') {
        const timeout = document.getElementById('custom-timeout')?.value || 5000;
        const failureRate = document.getElementById('custom-failure-rate')?.value || 0;

        message.custom_timeout = parseInt(timeout);
        message.custom_failure_rate = parseFloat(failureRate);
    }

    const span = createSpan('user.create_custom_task', {
        'task.type': taskType,
        'task.name': taskName,
        'user.action': 'form_submit'
    });

    logTelemetryEvent('USER_CREATE_CUSTOM_TASK_CLICKED', {
        taskType: taskType,
        taskName: taskName,
        timestamp: Date.now()
    });

    // Try WebSocket first
    if (window.ws && window.ws.readyState === WebSocket.OPEN && !isOfflineMode) {
        window.ws.send(JSON.stringify(message));

        if (span) {
            span.setAttributes({
                'websocket.message_sent': true,
                'websocket.ready_state': window.ws.readyState
            });
            span.end();
        }

        // Clear form and close modal on successful send
        closeModal();
    } else {
        // Fallback to REST API
        createCustomTaskRestMode(message, span);
    }
}

/**
 * Create custom task using REST API (fallback mode)
 */
async function createCustomTaskRestMode(formData, span) {
    try {
        const requestBody = {
            name: formData.name,
            message: formData.message,
            task_type: buildTaskTypeRequest(formData)
        };

        const response = await fetch('/api/tasks', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify(requestBody)
        });

        const data = await response.json();

        if (!data.success) {
            const errorMsg = data.error?.message || 'Unknown error occurred';
            showConnectionStatus(`❌ Task creation failed: ${errorMsg}`, 'error');
        } else {
            showConnectionStatus(`✅ ${formData.task_type} task created successfully`, 'info');
            closeModal();
        }

        if (span) {
            span.setAttributes({
                'rest.api_used': true,
                'rest.success': data.success
            });
            span.end();
        }
    } catch (error) {
        const errorMessage = `Failed to create custom task: ${error.message}`;
        showConnectionStatus(`❌ ${errorMessage}`, 'error');

        if (span) {
            span.recordException(error);
            span.setStatus({
                code: 2, // ERROR
                message: errorMessage
            });
            span.end();
        }
    }
}

/**
 * Build task type request object for REST API
 */
function buildTaskTypeRequest(formData) {
    switch (formData.task_type) {
        case 'quick':
            return { type: 'quick' };
        case 'long':
            return { type: 'long' };
        case 'error':
            return { type: 'error' };
        case 'custom':
            return {
                type: 'custom',
                custom_name: formData.name || 'Custom Task',
                timeout_ms: formData.custom_timeout || 5000,
                failure_rate: formData.custom_failure_rate || 0
            };
        default:
            return { type: 'quick' };
    }
}

/**
 * Update task options based on selected task type
 */
function updateTaskOptions() {
    const taskType = document.getElementById('task-type').value;
    const optionsDiv = document.getElementById('task-options');

    if (taskType === 'custom') {
        optionsDiv.innerHTML = `
            <div class="form-group">
                <label for="custom-timeout">Timeout (ms):</label>
                <input type="number" id="custom-timeout" value="5000" min="100" max="300000">
            </div>
            <div class="form-group">
                <label for="custom-failure-rate">Failure Rate (0.0-1.0):</label>
                <input type="number" id="custom-failure-rate" value="0" min="0" max="1" step="0.1">
            </div>
        `;
    } else {
        optionsDiv.innerHTML = '';
    }
}

/**
 * Clear the form inputs
 */
function clearForm() {
    document.getElementById('task-name').value = '';
    document.getElementById('task-message').value = '';
    document.getElementById('task-options').innerHTML = '';
}

/**
 * Open task creation modal with pre-selected task type
 */
function openTaskModal(taskType) {
    const modal = document.getElementById('task-modal');
    const badge = document.getElementById('modal-task-type-badge');
    const taskOptions = document.getElementById('task-options');

    // Set up the modal based on task type
    const taskConfig = {
        quick: {
            name: 'Quick Task',
            badge: 'badge-quick',
            placeholder: 'Quick task message (completes in ~2s)'
        },
        long: {
            name: 'Long Task',
            badge: 'badge-long',
            placeholder: 'Long task message (completes in ~10s)'
        },
        error: {
            name: 'Error Task',
            badge: 'badge-error',
            placeholder: 'Error task message (will fail for testing)'
        },
        custom: {
            name: 'Custom Task',
            badge: 'badge-custom',
            placeholder: 'Custom task message'
        }
    };

    const config = taskConfig[taskType] || taskConfig.quick;

    // Update modal UI
    badge.textContent = config.name;
    badge.className = `task-type-badge ${config.badge}`;

    // Update message placeholder
    document.getElementById('task-message').placeholder = config.placeholder;

    // Store the selected task type
    modal.dataset.taskType = taskType;

    // Show custom options for custom tasks
    if (taskType === 'custom') {
        taskOptions.innerHTML = `
            <div class="form-group">
                <label for="custom-timeout">Timeout (ms):</label>
                <input type="number" id="custom-timeout" value="5000" min="100" max="300000">
            </div>
            <div class="form-group">
                <label for="custom-failure-rate">Failure Rate (0.0-1.0):</label>
                <input type="number" id="custom-failure-rate" value="0" min="0" max="1" step="0.1">
            </div>
        `;
    } else {
        taskOptions.innerHTML = '';
    }

    // Clear form and show modal
    clearForm();
    modal.classList.add('show');

    // Focus on message input
    setTimeout(() => {
        document.getElementById('task-message').focus();
    }, 100);

    logTelemetryEvent('MODAL_OPENED', {
        taskType: taskType,
        timestamp: Date.now()
    });
}

/**
 * Close the task creation modal
 */
function closeModal() {
    const modal = document.getElementById('task-modal');
    modal.classList.remove('show');
    clearForm();

    logTelemetryEvent('MODAL_CLOSED', {
        timestamp: Date.now()
    });
}

/**
 * Close modal when clicking on backdrop
 */
function closeModalOnBackdrop(event) {
    if (event.target.id === 'task-modal') {
        closeModal();
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

    // Add keyboard support for modal
    document.addEventListener('keydown', function(event) {
        if (event.key === 'Escape') {
            closeModal();
        }
    });

    // Export functions to global scope for HTML onclick handlers
    window.createTask = createTask;
    window.cancelTask = cancelTask;
    window.refreshTasks = refreshTasks;
    window.createCustomTask = createCustomTask;
    window.updateTaskOptions = updateTaskOptions;
    window.clearForm = clearForm;
    window.openTaskModal = openTaskModal;
    window.closeModal = closeModal;
    window.closeModalOnBackdrop = closeModalOnBackdrop;
}