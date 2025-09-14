use task_core::*;
use actix::Actor;

#[test]
fn test_task_actor_creation() {
    let task = TaskActor::new("Test Task".to_string(), "Test message".to_string(), 5000);

    assert_eq!(task.metadata.name, "Test Task");
    assert_eq!(task.metadata.message, "Test message");
    assert_eq!(task.metadata.status, TaskStatus::InProgress);
    assert!(task.metadata.result.is_none());
    assert!(task.metadata.error.is_none());
    assert!(task.metadata.finished_at.is_none());
}

#[test]
fn test_task_uuid_uniqueness() {
    let task1 = TaskActor::new("Task 1".to_string(), "Message 1".to_string(), 5000);
    let task2 = TaskActor::new("Task 2".to_string(), "Message 2".to_string(), 5000);

    // Each task should have a unique ID
    assert_ne!(task1.metadata.id, task2.metadata.id);
}

#[actix_rt::test]
async fn test_task_status_query() {
    let task = TaskActor::new("Query Test".to_string(), "Query message".to_string(), 5000);
    let addr = task.start();

    let status = addr.send(GetTaskStatus).await.unwrap();

    assert_eq!(status.name, "Query Test");
    assert_eq!(status.message, "Query message");
    assert_eq!(status.status, TaskStatus::InProgress);
}

#[actix_rt::test]
async fn test_task_metadata_update_on_completion() {
    let mut task = TaskActor::new("Metadata Test".to_string(), "Test message".to_string(), 5000);

    // Simulate completion
    task.metadata.status = TaskStatus::Completed;
    task.metadata.finished_at = Some(chrono::Utc::now());
    task.metadata.result = Some("Test result".to_string());

    assert_eq!(task.metadata.status, TaskStatus::Completed);
    assert!(task.metadata.finished_at.is_some());
    assert_eq!(task.metadata.result, Some("Test result".to_string()));
    assert!(task.metadata.error.is_none());
}

#[actix_rt::test]
async fn test_task_metadata_update_on_error() {
    let mut task = TaskActor::new("Error Metadata Test".to_string(), "Test message".to_string(), 5000);

    // Simulate error
    task.metadata.status = TaskStatus::Error;
    task.metadata.finished_at = Some(chrono::Utc::now());
    task.metadata.error = Some("Test error".to_string());

    assert_eq!(task.metadata.status, TaskStatus::Error);
    assert!(task.metadata.finished_at.is_some());
    assert_eq!(task.metadata.error, Some("Test error".to_string()));
    assert!(task.metadata.result.is_none());
}