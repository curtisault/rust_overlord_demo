/**
 * WebSocket LiveView Connection Manager
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
 * Usage:
 *   Include this script in your HTML and call initWebSocketConnection()
 *   The connection will be established automatically and handle all updates.
 *
 * Author: Claude Code Assistant
 * Compatible with: Modern browsers with WebSocket support
 */

let ws = null;

/**
 * Initialize WebSocket connection to the LiveView server
 * Sets up message handlers and connection management
 */
function initWebSocketConnection() {
    connect();
}

/**
 * Establish WebSocket connection with automatic reconnection
 * Handles both initial page loads and subsequent partial updates
 */
function connect() {
    const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
    ws = new WebSocket(protocol + '//' + location.host + '/ws/');

    ws.onmessage = function(event) {
        const data = JSON.parse(event.data);

        if (data.type === 'full_page_load') {
            // Initial connection - replace entire document
            // This only happens on first load to establish the UI
            document.documentElement.innerHTML = data.html;
            // Reconnect after page replacement to re-establish WebSocket
            setTimeout(connect, 100);

        } else if (data.type === 'task_grid_update') {
            // Partial update - use DOM diffing to update only changed content
            updateTaskGrid(data.html);
        }
    };

    ws.onclose = function() {
        // Automatic reconnection after 1 second
        setTimeout(connect, 1000);
    };

    ws.onerror = function() {
        // Automatic reconnection on error after 1 second
        setTimeout(connect, 1000);
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
    if (ws && ws.readyState === WebSocket.OPEN) {
        const message = {
            type: 'create_task',
            task_type: taskType
        };
        ws.send(JSON.stringify(message));
    } else {
        console.warn('WebSocket not connected - cannot create task');
    }
}

/**
 * Cancel an existing task via WebSocket
 * Sends a task cancellation message to the server
 *
 * @param {string} taskId - UUID of the task to cancel
 */
function cancelTask(taskId) {
    if (ws && ws.readyState === WebSocket.OPEN) {
        const message = {
            type: 'cancel_task',
            task_id: taskId
        };
        ws.send(JSON.stringify(message));
    } else {
        console.warn('WebSocket not connected - cannot cancel task');
    }
}

/**
 * Request a refresh of current task state
 * Triggers a server-side task list update
 */
function refreshTasks() {
    if (ws && ws.readyState === WebSocket.OPEN) {
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