use actix::Actor;
use std::time::Duration;
use task_core::*;

#[actix_rt::test]
async fn test_task_completion() {
    let task = TaskActor::new(
        "Complete Test".to_string(),
        "Complete message".to_string(),
        5000,
    );
    let addr = task.start();

    // Send completion message
    addr.send(CompleteTask {
        result: "Task finished successfully".to_string(),
    })
    .await
    .unwrap();

    // Give a moment for the actor to process and stop
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Try to get status - this should fail as actor is stopped
    let result = addr.send(GetTaskStatus).await;
    assert!(result.is_err()); // Actor should be stopped
}

#[actix_rt::test]
async fn test_task_error_handling() {
    let task = TaskActor::new("Error Test".to_string(), "Error message".to_string(), 5000);
    let addr = task.start();

    // Send error message
    addr.send(ErrorTask {
        error: "Something went wrong".to_string(),
    })
    .await
    .unwrap();

    // Give a moment for the actor to process and stop
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Try to get status - this should fail as actor is stopped
    let result = addr.send(GetTaskStatus).await;
    assert!(result.is_err()); // Actor should be stopped
}

#[actix_rt::test]
async fn test_task_cancellation() {
    let task = TaskActor::new(
        "Cancel Test".to_string(),
        "Cancel message".to_string(),
        5000,
    );
    let addr = task.start();

    // Send cancel message
    addr.send(CancelTask).await.unwrap();

    // Give a moment for the actor to process and stop
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Try to get status - this should fail as actor is stopped
    let result = addr.send(GetTaskStatus).await;
    assert!(result.is_err()); // Actor should be stopped
}

#[actix_rt::test]
async fn test_start_task_message() {
    let task = TaskActor::new("Start Test".to_string(), "Start message".to_string(), 5000);
    let addr = task.start();

    // Send start message with very short duration
    addr.send(StartTask {
        duration: Duration::from_millis(50),
    })
    .await
    .unwrap();

    // Wait a bit longer than the task duration
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Try to get status - actor should be stopped after completion
    let result = addr.send(GetTaskStatus).await;
    assert!(result.is_err()); // Actor should be stopped after completing
}
