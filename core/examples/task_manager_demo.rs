use actix::Actor;
use std::time::Duration;
use task_core::*;

#[actix_rt::main]
async fn main() {
    println!("=== Task Manager Demo ===");

    let manager = TaskManagerActor::new().start();

    println!("Creating tasks...");

    // Create different types of tasks
    let quick_task = manager
        .send(CreateTask {
            name: "Quick Task".to_string(),
            message: "A simple 2-second task".to_string(),
            task_type: TaskType::Quick { timeout_ms: None },
        })
        .await
        .unwrap();
    println!("Created Quick Task: {}", quick_task);

    let long_task = manager
        .send(CreateTask {
            name: "Long Task".to_string(),
            message: "A background 10-second task".to_string(),
            task_type: TaskType::Long { timeout_ms: None },
        })
        .await
        .unwrap();
    println!("Created Long Task: {}", long_task);

    let error_task = manager
        .send(CreateTask {
            name: "Error Task".to_string(),
            message: "This task will fail".to_string(),
            task_type: TaskType::Error {
                timeout_ms: None,
                error_type: ErrorType::Immediate,
            },
        })
        .await
        .unwrap();
    println!("Created Error Task: {}", error_task);

    // Wait a moment for tasks to be registered
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Query all tasks
    let all_tasks = manager.send(GetAllTasks).await.unwrap();
    println!("\n=== All Tasks ===");
    for task in &all_tasks {
        println!(
            "Task '{}': {} ({})",
            task.name,
            task.message,
            match task.status {
                TaskStatus::InProgress => "In Progress",
                TaskStatus::Completed => "Completed",
                TaskStatus::Error => "Error",
            }
        );
    }

    // Wait for quick task to complete
    println!("\nWaiting for quick task to complete...");
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Check quick task status
    if let Some(task) = manager.send(GetTask { id: quick_task }).await.unwrap() {
        println!("Quick task status: {:?}", task.status);
        if let Some(result) = &task.result {
            println!("Quick task result: {}", result);
        }
    }

    // Cancel the long task
    println!("\nCancelling long task...");
    let cancelled = manager
        .send(CancelTaskById { id: long_task })
        .await
        .unwrap();
    println!("Long task cancelled: {}", cancelled);

    // Final status check
    tokio::time::sleep(Duration::from_millis(100)).await;
    let final_tasks = manager.send(GetAllTasks).await.unwrap();
    println!("\n=== Final Status ===");
    for task in &final_tasks {
        println!("Task '{}': {:?}", task.name, task.status);
        if let Some(error) = &task.error {
            println!("  Error: {}", error);
        }
        if let Some(result) = &task.result {
            println!("  Result: {}", result);
        }
    }

    println!("\nDemo completed!");
}
