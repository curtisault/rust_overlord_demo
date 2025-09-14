use task_core::*;
use std::time::Duration;
use uuid::Uuid;

#[test]
fn test_create_task_message_serialization() {
    let msg = CreateTask {
        name: "Test Task".to_string(),
        message: "Test message".to_string(),
        task_type: TaskType::Quick,
    };

    let json = serde_json::to_string(&msg).unwrap();
    let deserialized: CreateTask = serde_json::from_str(&json).unwrap();

    assert_eq!(msg.name, deserialized.name);
    assert_eq!(msg.message, deserialized.message);
}

#[test]
fn test_task_type_serialization() {
    let types = vec![TaskType::Quick, TaskType::Long, TaskType::Error];

    for task_type in types {
        let json = serde_json::to_string(&task_type).unwrap();
        let deserialized: TaskType = serde_json::from_str(&json).unwrap();

        // TaskType doesn't implement PartialEq, so test by re-serializing
        let json2 = serde_json::to_string(&deserialized).unwrap();
        assert_eq!(json, json2);
    }
}

#[test]
fn test_start_task_duration_serialization() {
    let msg = StartTask {
        duration: Duration::from_secs(5),
    };

    let json = serde_json::to_string(&msg).unwrap();
    let deserialized: StartTask = serde_json::from_str(&json).unwrap();

    assert_eq!(msg.duration, deserialized.duration);
}

#[test]
fn test_get_task_message_serialization() {
    let test_id = Uuid::new_v4();
    let msg = GetTask { id: test_id };

    let json = serde_json::to_string(&msg).unwrap();
    let deserialized: GetTask = serde_json::from_str(&json).unwrap();

    assert_eq!(msg.id, deserialized.id);
}

#[test]
fn test_cancel_task_message_serialization() {
    let test_id = Uuid::new_v4();
    let msg = CancelTaskById { id: test_id };

    let json = serde_json::to_string(&msg).unwrap();
    let deserialized: CancelTaskById = serde_json::from_str(&json).unwrap();

    assert_eq!(msg.id, deserialized.id);
}

#[test]
fn test_api_response_serialization() {
    // Test success response
    let success_response = ApiResponse::success("test data".to_string());
    let json = serde_json::to_string(&success_response).unwrap();
    let deserialized: ApiResponse<String> = serde_json::from_str(&json).unwrap();

    assert!(deserialized.success);
    assert_eq!(deserialized.data, Some("test data".to_string()));
    assert!(deserialized.error.is_none());

    // Test error response
    let error_response: ApiResponse<String> = ApiResponse::error("test error".to_string());
    let json = serde_json::to_string(&error_response).unwrap();
    let deserialized: ApiResponse<String> = serde_json::from_str(&json).unwrap();

    assert!(!deserialized.success);
    assert!(deserialized.data.is_none());
    assert_eq!(deserialized.error, Some("test error".to_string()));
}

#[test]
fn test_task_create_response_serialization() {
    let response = TaskCreateResponse {
        id: Uuid::new_v4(),
        name: "Test Task".to_string(),
        status: TaskStatus::InProgress,
        created_at: chrono::Utc::now(),
    };

    let json = serde_json::to_string(&response).unwrap();
    let deserialized: TaskCreateResponse = serde_json::from_str(&json).unwrap();

    assert_eq!(response.id, deserialized.id);
    assert_eq!(response.name, deserialized.name);
    assert_eq!(response.status, deserialized.status);
}

#[test]
fn test_task_list_response_serialization() {
    let metadata = TaskMetadata {
        id: Uuid::new_v4(),
        name: "Test Task".to_string(),
        message: "Test message".to_string(),
        status: TaskStatus::Completed,
        started_at: chrono::Utc::now(),
        finished_at: Some(chrono::Utc::now()),
        result: Some("Success".to_string()),
        error: None,
    };

    let response = TaskListResponse {
        tasks: vec![metadata],
        total: 1,
    };

    let json = serde_json::to_string(&response).unwrap();
    let deserialized: TaskListResponse = serde_json::from_str(&json).unwrap();

    assert_eq!(response.total, deserialized.total);
    assert_eq!(response.tasks.len(), deserialized.tasks.len());
    assert_eq!(response.tasks[0].id, deserialized.tasks[0].id);
}

#[test]
fn test_task_status_update_serialization() {
    let update = TaskStatusUpdate {
        id: Uuid::new_v4(),
        status: TaskStatus::Completed,
        updated_at: chrono::Utc::now(),
        result: Some("Task completed successfully".to_string()),
        error: None,
    };

    let json = serde_json::to_string(&update).unwrap();
    let deserialized: TaskStatusUpdate = serde_json::from_str(&json).unwrap();

    assert_eq!(update.id, deserialized.id);
    assert_eq!(update.status, deserialized.status);
    assert_eq!(update.result, deserialized.result);
}