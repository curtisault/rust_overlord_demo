class TaskDashboard {
    constructor() {
        this.eventSource = null;
        this.reconnectAttempts = 0;
        this.maxReconnectAttempts = 5;
        this.reconnectDelay = 1000;
        this.tasks = new Map();

        this.initializeEventListeners();
        this.connectToStream();
        this.loadInitialTasks();
    }

    initializeEventListeners() {
        // Task creation buttons
        document.getElementById('quick-task-btn').addEventListener('click', () => {
            this.openTaskModal('quick');
        });

        document.getElementById('long-task-btn').addEventListener('click', () => {
            this.openTaskModal('long');
        });

        document.getElementById('error-task-btn').addEventListener('click', () => {
            this.openTaskModal('error');
        });

        // Modal controls
        document.getElementById('modal-close').addEventListener('click', () => {
            this.closeTaskModal();
        });

        document.getElementById('modal-cancel').addEventListener('click', () => {
            this.closeTaskModal();
        });

        // Task form submission
        document.getElementById('task-form').addEventListener('submit', (e) => {
            e.preventDefault();
            this.submitTaskForm();
        });

        // Close modal on backdrop click
        document.getElementById('task-modal').addEventListener('click', (e) => {
            if (e.target.id === 'task-modal') {
                this.closeTaskModal();
            }
        });
    }

    connectToStream() {
        this.updateConnectionStatus('connecting');
        this.pollTasks();
    }

    async pollTasks() {
        try {
            const response = await fetch('/api/tasks');
            if (response.ok) {
                const data = await response.json();
                if (data.success) {
                    this.updateTaskList(data.data.tasks);
                    this.updateConnectionStatus('online');
                    this.reconnectAttempts = 0;
                } else {
                    throw new Error('Failed to get tasks');
                }
            } else {
                throw new Error('HTTP error');
            }
        } catch (error) {
            console.error('Error polling tasks:', error);
            this.updateConnectionStatus('offline');
            this.handleReconnection();
        }

        // Poll every 2 seconds
        setTimeout(() => this.pollTasks(), 2000);
    }

    handleReconnection() {
        if (this.reconnectAttempts < this.maxReconnectAttempts) {
            this.reconnectAttempts++;
            console.log(`🔄 Attempting to reconnect (${this.reconnectAttempts}/${this.maxReconnectAttempts})...`);

            setTimeout(() => {
                this.pollTasks();
            }, this.reconnectDelay * this.reconnectAttempts);
        } else {
            console.error('❌ Max reconnection attempts reached');
        }
    }

    updateConnectionStatus(status) {
        const indicator = document.getElementById('connection-indicator');
        const statusText = indicator.querySelector('span');

        indicator.className = status;

        switch (status) {
            case 'online':
                statusText.textContent = 'Connected';
                break;
            case 'connecting':
                statusText.textContent = 'Connecting...';
                break;
            case 'offline':
            default:
                statusText.textContent = 'Disconnected';
                break;
        }
    }

    async loadInitialTasks() {
        try {
            const response = await fetch('/api/tasks');
            if (response.ok) {
                const data = await response.json();
                if (data.success) {
                    this.updateTaskList(data.data.tasks);
                }
            }
        } catch (error) {
            console.error('Failed to load initial tasks:', error);
        }
    }

    updateTaskList(tasks) {
        // Clear current tasks
        this.tasks.clear();

        const columns = {
            'in-progress': document.getElementById('in-progress-tasks'),
            'completed': document.getElementById('completed-tasks'),
            'error': document.getElementById('error-tasks')
        };

        // Clear all columns
        Object.values(columns).forEach(column => {
            column.innerHTML = '';
        });

        // Group tasks by status
        const tasksByStatus = {
            'in-progress': [],
            'completed': [],
            'error': []
        };

        tasks.forEach(task => {
            this.tasks.set(task.id, task);

            const status = task.status.toLowerCase().replace(' ', '-');
            if (tasksByStatus[status]) {
                tasksByStatus[status].push(task);
            }
        });

        // Render tasks in each column
        Object.entries(tasksByStatus).forEach(([status, statusTasks]) => {
            const column = columns[status];
            if (column) {
                statusTasks.forEach(task => {
                    column.appendChild(this.createTaskCard(task));
                });
            }
        });

        // Update counts
        this.updateTaskCounts(tasksByStatus);
    }

    createTaskCard(task) {
        const card = document.createElement('div');
        card.className = `task-card ${task.status.toLowerCase().replace(' ', '-')}`;
        card.dataset.taskId = task.id;

        const statusIcon = this.getStatusIcon(task.status);
        const statusClass = task.status.toLowerCase().replace(' ', '-');

        const startTime = task.started_at ? this.formatDateTime(task.started_at) : 'Not started';
        const endTime = task.finished_at ? this.formatDateTime(task.finished_at) : 'Running...';
        const duration = task.duration_ms ? `${task.duration_ms}ms` : 'Calculating...';

        card.innerHTML = `
            <div class="task-header">
                <div class="task-title">${this.escapeHtml(task.name)}</div>
                <div class="task-actions">
                    ${task.status === 'InProgress' ? `
                        <button class="task-action cancel" onclick="taskDashboard.cancelTask('${task.id}')" title="Cancel Task">
                            <i class="fas fa-times"></i>
                        </button>
                    ` : ''}
                </div>
            </div>

            <div class="task-status ${statusClass}">
                <i class="${statusIcon}"></i>
                ${task.status}
            </div>

            <div class="task-message">
                ${this.escapeHtml(task.message)}
            </div>

            <div class="task-meta">
                <div class="meta-item">
                    <div class="meta-label">Started</div>
                    <div class="meta-value">${startTime}</div>
                </div>
                <div class="meta-item">
                    <div class="meta-label">Finished</div>
                    <div class="meta-value">${endTime}</div>
                </div>
                <div class="meta-item">
                    <div class="meta-label">Duration</div>
                    <div class="meta-value">${duration}</div>
                </div>
                <div class="meta-item">
                    <div class="meta-label">Result</div>
                    <div class="meta-value">${task.result ? this.escapeHtml(task.result) : 'Pending...'}</div>
                </div>
            </div>
        `;

        return card;
    }

    getStatusIcon(status) {
        switch (status.toLowerCase()) {
            case 'inprogress':
                return 'fas fa-spinner fa-spin';
            case 'completed':
                return 'fas fa-check-circle';
            case 'error':
                return 'fas fa-times-circle';
            default:
                return 'fas fa-question-circle';
        }
    }

    updateTaskCounts(tasksByStatus) {
        document.getElementById('in-progress-count').textContent = tasksByStatus['in-progress'].length;
        document.getElementById('completed-count').textContent = tasksByStatus['completed'].length;
        document.getElementById('error-count').textContent = tasksByStatus['error'].length;
    }

    openTaskModal(taskType) {
        const modal = document.getElementById('task-modal');
        const title = document.getElementById('modal-title');
        const options = document.getElementById('task-options');

        // Reset form
        document.getElementById('task-form').reset();

        // Set modal title and create task-specific options
        switch (taskType) {
            case 'quick':
                title.textContent = 'Create Quick Task';
                options.innerHTML = `
                    <div class="form-group">
                        <label for="timeout">Timeout (ms, optional):</label>
                        <input type="number" id="timeout" placeholder="Leave empty for default (2000ms)" min="100">
                    </div>
                `;
                break;

            case 'long':
                title.textContent = 'Create Long Task';
                options.innerHTML = `
                    <div class="form-group">
                        <label for="timeout">Timeout (ms, optional):</label>
                        <input type="number" id="timeout" placeholder="Leave empty for default (10000ms)" min="100">
                    </div>
                `;
                break;

            case 'error':
                title.textContent = 'Create Error Task';
                options.innerHTML = `
                    <div class="form-group">
                        <label for="timeout">Timeout (ms, optional):</label>
                        <input type="number" id="timeout" placeholder="Leave empty for default" min="100">
                    </div>
                    <div class="form-group">
                        <label for="error-type">Error Type:</label>
                        <select id="error-type">
                            <option value="immediate">Immediate Error</option>
                            <option value="timeout">Timeout Error</option>
                            <option value="random">Random Error</option>
                            <option value="network">Network Error</option>
                            <option value="validation">Validation Error</option>
                        </select>
                    </div>
                `;
                break;
        }

        modal.dataset.taskType = taskType;
        modal.classList.add('show');
    }

    closeTaskModal() {
        const modal = document.getElementById('task-modal');
        modal.classList.remove('show');
    }

    async submitTaskForm() {
        const modal = document.getElementById('task-modal');
        const taskType = modal.dataset.taskType;

        const name = document.getElementById('task-name').value.trim();
        const message = document.getElementById('task-message').value.trim();

        if (!message) {
            alert('Please enter a task message');
            return;
        }

        let taskData = {
            name: name,
            message: message,
            task_type: { type: taskType }
        };

        // Add task-specific options
        const timeoutInput = document.getElementById('timeout');
        if (timeoutInput && timeoutInput.value) {
            taskData.task_type.timeout_ms = parseInt(timeoutInput.value);
        }

        if (taskType === 'error') {
            const errorType = document.getElementById('error-type').value;
            taskData.task_type.error_type = errorType;
        }

        try {
            const response = await fetch('/api/tasks', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify(taskData)
            });

            if (response.ok) {
                const result = await response.json();
                if (result.success) {
                    console.log('✅ Task created:', result.data);
                    this.closeTaskModal();
                } else {
                    alert('Failed to create task: ' + result.error);
                }
            } else {
                alert('Failed to create task. Please try again.');
            }
        } catch (error) {
            console.error('Error creating task:', error);
            alert('Failed to create task. Please check your connection.');
        }
    }

    async cancelTask(taskId) {
        if (!confirm('Are you sure you want to cancel this task?')) {
            return;
        }

        try {
            const response = await fetch(`/api/tasks/${taskId}`, {
                method: 'DELETE'
            });

            if (response.ok) {
                const result = await response.json();
                if (result.success) {
                    console.log('✅ Task cancelled:', taskId);
                } else {
                    alert('Failed to cancel task: ' + result.error);
                }
            } else {
                alert('Failed to cancel task. Please try again.');
            }
        } catch (error) {
            console.error('Error cancelling task:', error);
            alert('Failed to cancel task. Please check your connection.');
        }
    }

    formatDateTime(dateString) {
        const date = new Date(dateString);
        return date.toLocaleTimeString() + ' ' + date.toLocaleDateString();
    }

    escapeHtml(text) {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    }
}

// Initialize the dashboard when the page loads
let taskDashboard;
document.addEventListener('DOMContentLoaded', () => {
    taskDashboard = new TaskDashboard();
});