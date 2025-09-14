use task_core::*;
use uuid::Uuid;
use chrono::Utc;

#[test]
fn test_task_metadata_serialization() {
    let metadata = TaskMetadata {
        id: Uuid::new_v4(),
        name: "Serialization Test".to_string(),
        message: "Test serialization".to_string(),
        status: TaskStatus::Completed,
        started_at: Utc::now(),
        finished_at: Some(Utc::now()),
        result: Some("Success".to_string()),
        error: None,
        timeout_ms: 5000,
        actual_duration_ms: Some(1000),
        cancelled_at: None,
        timeout_at: None,
    };

    // Test JSON serialization
    let json = serde_json::to_string(&metadata).unwrap();
    let deserialized: TaskMetadata = serde_json::from_str(&json).unwrap();

    assert_eq!(metadata.id, deserialized.id);
    assert_eq!(metadata.name, deserialized.name);
    assert_eq!(metadata.status, deserialized.status);
    assert_eq!(metadata.result, deserialized.result);
}

#[test]
fn test_task_status_transitions() {
    // Test that status enum values are as expected
    assert_eq!(TaskStatus::InProgress, TaskStatus::InProgress);
    assert_ne!(TaskStatus::InProgress, TaskStatus::Completed);
    assert_ne!(TaskStatus::InProgress, TaskStatus::Error);
    assert_ne!(TaskStatus::Completed, TaskStatus::Error);
}

#[test]
fn test_task_status_serialization() {
    // Test each status variant
    let statuses = vec![
        TaskStatus::InProgress,
        TaskStatus::Completed,
        TaskStatus::Error,
    ];

    for status in statuses {
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: TaskStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, deserialized);
    }
}