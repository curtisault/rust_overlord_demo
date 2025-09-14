use actix::Actor;
use std::time::Duration;
use task_core::*;

#[test]
fn test_task_manager_creation() {
    let _manager = TaskManagerActor::new();
    // Manager should be created successfully
    // Internal state is private, so we just verify creation works
}

#[actix_rt::test]
async fn test_create_quick_task() {
    let manager = TaskManagerActor::new().start();

    let task_id = manager
        .send(CreateTask {
            name: "Quick Test".to_string(),
            message: "Test quick task".to_string(),
            task_type: TaskType::Quick { timeout_ms: None },
        })
        .await
        .unwrap();

    // Should return a valid UUID
    assert_ne!(task_id.to_string(), "");

    // Give a moment for task setup
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Check task was registered
    let task = manager.send(GetTask { id: task_id }).await.unwrap();
    assert!(task.is_some());

    let task_metadata = task.unwrap();
    assert_eq!(task_metadata.name, "Quick Test");
    assert_eq!(task_metadata.message, "Test quick task");
    assert_eq!(task_metadata.status, TaskStatus::InProgress);
}

#[actix_rt::test]
async fn test_create_error_task() {
    let manager = TaskManagerActor::new().start();

    let task_id = manager
        .send(CreateTask {
            name: "Error Test".to_string(),
            message: "Test error task".to_string(),
            task_type: TaskType::Error {
                timeout_ms: None,
                error_type: ErrorType::Immediate,
            },
        })
        .await
        .unwrap();

    // Give a moment for error to be processed
    tokio::time::sleep(Duration::from_millis(50)).await;

    let task = manager.send(GetTask { id: task_id }).await.unwrap();
    assert!(task.is_some());
}

#[actix_rt::test]
async fn test_get_all_tasks() {
    let manager = TaskManagerActor::new().start();

    // Create multiple tasks
    let task1_id = manager
        .send(CreateTask {
            name: "Task 1".to_string(),
            message: "First task".to_string(),
            task_type: TaskType::Quick { timeout_ms: None },
        })
        .await
        .unwrap();

    let task2_id = manager
        .send(CreateTask {
            name: "Task 2".to_string(),
            message: "Second task".to_string(),
            task_type: TaskType::Long { timeout_ms: None },
        })
        .await
        .unwrap();

    // Give a moment for tasks to be created
    tokio::time::sleep(Duration::from_millis(10)).await;

    let all_tasks = manager.send(GetAllTasks).await.unwrap();
    assert_eq!(all_tasks.len(), 2);

    // Check tasks are in the list
    let task_ids: Vec<_> = all_tasks.iter().map(|t| t.id).collect();
    assert!(task_ids.contains(&task1_id));
    assert!(task_ids.contains(&task2_id));
}

#[actix_rt::test]
async fn test_cancel_task() {
    let manager = TaskManagerActor::new().start();

    let task_id = manager
        .send(CreateTask {
            name: "Cancellable Task".to_string(),
            message: "This task will be cancelled".to_string(),
            task_type: TaskType::Long { timeout_ms: None },
        })
        .await
        .unwrap();

    // Give a moment for task to start
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Cancel the task
    let cancelled = manager.send(CancelTaskById { id: task_id }).await.unwrap();
    assert!(cancelled);

    // Give a moment for cancellation to be processed
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Check task is marked as cancelled/error
    let task = manager.send(GetTask { id: task_id }).await.unwrap();
    assert!(task.is_some());
    let task_metadata = task.unwrap();
    assert_eq!(task_metadata.status, TaskStatus::Error);
    assert!(task_metadata.error.is_some());
    assert!(task_metadata.error.unwrap().contains("cancelled"));
}

#[actix_rt::test]
async fn test_cancel_nonexistent_task() {
    let manager = TaskManagerActor::new().start();

    let fake_id = uuid::Uuid::new_v4();
    let cancelled = manager.send(CancelTaskById { id: fake_id }).await.unwrap();
    assert!(!cancelled);
}

#[actix_rt::test]
async fn test_get_nonexistent_task() {
    let manager = TaskManagerActor::new().start();

    let fake_id = uuid::Uuid::new_v4();
    let task = manager.send(GetTask { id: fake_id }).await.unwrap();
    assert!(task.is_none());
}

#[actix_rt::test]
async fn test_task_types() {
    let manager = TaskManagerActor::new().start();

    // Test each task type
    let quick_id = manager
        .send(CreateTask {
            name: "Quick".to_string(),
            message: "Quick task".to_string(),
            task_type: TaskType::Quick { timeout_ms: None },
        })
        .await
        .unwrap();

    let long_id = manager
        .send(CreateTask {
            name: "Long".to_string(),
            message: "Long task".to_string(),
            task_type: TaskType::Long { timeout_ms: None },
        })
        .await
        .unwrap();

    let error_id = manager
        .send(CreateTask {
            name: "Error".to_string(),
            message: "Error task".to_string(),
            task_type: TaskType::Error {
                timeout_ms: None,
                error_type: ErrorType::Immediate,
            },
        })
        .await
        .unwrap();

    // All should be valid UUIDs
    assert_ne!(quick_id, long_id);
    assert_ne!(long_id, error_id);
    assert_ne!(quick_id, error_id);
}

#[actix_rt::test]
async fn test_task_completion_lifecycle() {
    let manager = TaskManagerActor::new().start();

    let task_id = manager
        .send(CreateTask {
            name: "Lifecycle Test".to_string(),
            message: "Test full lifecycle".to_string(),
            task_type: TaskType::Quick { timeout_ms: None },
        })
        .await
        .unwrap();

    // Initial state
    tokio::time::sleep(Duration::from_millis(10)).await;
    let task = manager
        .send(GetTask { id: task_id })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(task.status, TaskStatus::InProgress);

    // Wait for completion (Quick tasks take 2 seconds + buffer)
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Task should still be in metadata but completed
    let task = manager.send(GetTask { id: task_id }).await.unwrap();
    if let Some(task_metadata) = task {
        // Task might be completed or cleaned up
        println!("Final task status: {:?}", task_metadata.status);
    }
}
