use std::sync::Arc;

use rmcp::model::{
    CallToolResponse, CallToolResult, CancelTaskParams, CreateTaskResult, GetTaskParams,
    GetTaskResult, UpdateTaskParams,
};
use rmcp::task_manager::{TaskContext, TaskExit, TaskFuture, TaskManager, TaskOptions};
use rmcp::ErrorData as McpError;

use crate::core::config::McpTasksSection;

pub const MAX_TASK_ELIGIBLE_BATCH_ITEMS: usize = 8;

#[derive(Clone)]
pub struct TaskRuntime {
    manager: Arc<TaskManager>,
    ttl_ms: u64,
    poll_interval_ms: u64,
    max_concurrent: usize,
}

impl std::fmt::Debug for TaskRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskRuntime")
            .field("ttl_ms", &self.ttl_ms)
            .field("poll_interval_ms", &self.poll_interval_ms)
            .field("max_concurrent", &self.max_concurrent)
            .finish()
    }
}

impl TaskRuntime {
    pub fn from_config(config: &McpTasksSection) -> Option<Self> {
        if !config.enabled {
            return None;
        }
        Some(Self {
            manager: Arc::new(TaskManager::new()),
            ttl_ms: config.ttl_ms,
            poll_interval_ms: config.poll_interval_ms,
            max_concurrent: config.max_concurrent,
        })
    }

    pub fn manager(&self) -> &Arc<TaskManager> {
        &self.manager
    }

    pub fn max_concurrent(&self) -> usize {
        self.max_concurrent
    }

    pub fn ttl_ms(&self) -> u64 {
        self.ttl_ms
    }

    pub fn poll_interval_ms(&self) -> u64 {
        self.poll_interval_ms
    }

    pub fn at_capacity(&self) -> bool {
        self.manager.running_task_count() >= self.max_concurrent
    }

    pub fn options(&self, status_message: &str) -> TaskOptions {
        TaskOptions::new()
            .with_ttl_ms(self.ttl_ms)
            .with_poll_interval_ms(self.poll_interval_ms)
            .with_status_message(status_message.to_string())
    }

    pub fn spawn<F>(&self, status_message: &str, operation: F) -> CreateTaskResult
    where
        F: FnOnce(TaskContext) -> TaskFuture + Send + 'static,
    {
        let task = self.manager.spawn(self.options(status_message), operation);
        CreateTaskResult::new(task)
    }

    pub fn get_task(&self, request: &GetTaskParams) -> Result<GetTaskResult, McpError> {
        let task = self.manager.get_task(&request.task_id)?;
        Ok(GetTaskResult::new(task))
    }

    pub fn update_task(&self, request: &UpdateTaskParams) -> Result<(), McpError> {
        self.manager
            .update_task(&request.task_id, request.input_responses.clone())
    }

    pub fn cancel_task(&self, request: &CancelTaskParams) -> Result<(), McpError> {
        self.manager.cancel_task(&request.task_id)
    }
}

pub fn is_eligible(tool: &str, depth: Option<&str>, item_count: usize) -> bool {
    match tool {
        "research_search" => matches!(depth, Some("deep")),
        "batch_fetch" => item_count > MAX_TASK_ELIGIBLE_BATCH_ITEMS,
        _ => false,
    }
}

pub fn task_response(result: CreateTaskResult) -> Result<CallToolResponse, McpError> {
    Ok(CallToolResponse::Task(result))
}

pub fn complete_response(result: CallToolResult) -> Result<CallToolResponse, McpError> {
    Ok(CallToolResponse::Complete(result))
}

pub async fn run_cancellable<T>(
    context: Option<TaskContext>,
    work: impl std::future::Future<Output = Result<T, McpError>>,
) -> Result<T, TaskExit> {
    let Some(context) = context else {
        return work.await.map_err(TaskExit::Error);
    };
    if context.is_cancel_requested() {
        return Err(TaskExit::Cancelled);
    }
    tokio::select! {
        biased;
        () = context.cancelled() => Err(TaskExit::Cancelled),
        result = work => result.map_err(TaskExit::Error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::McpTasksSection;

    fn settled(runtime: &TaskRuntime, task_id: &str) -> Option<rmcp::model::TaskStatus> {
        let detailed = runtime.manager().get_task(task_id).ok()?;
        detailed
            .task
            .status
            .is_terminal()
            .then_some(detailed.task.status)
    }

    #[test]
    fn disabled_by_default() {
        let config = McpTasksSection::default();
        assert!(!config.enabled);
        assert!(TaskRuntime::from_config(&config).is_none());
    }

    #[test]
    fn enabled_runtime_applies_configured_bounds() {
        let config = McpTasksSection {
            enabled: true,
            ttl_ms: 60_000,
            max_concurrent: 2,
            poll_interval_ms: 500,
        };
        let runtime = TaskRuntime::from_config(&config).expect("enabled");
        assert_eq!(runtime.ttl_ms(), 60_000);
        assert_eq!(runtime.max_concurrent(), 2);
        assert_eq!(runtime.poll_interval_ms(), 500);
        assert!(!runtime.at_capacity());
    }

    #[test]
    fn eligibility_rules_are_explicit() {
        assert!(is_eligible("research_search", Some("deep"), 0));
        assert!(!is_eligible("research_search", Some("standard"), 0));
        assert!(!is_eligible("research_search", None, 0));
        assert!(is_eligible("batch_fetch", None, 9));
        assert!(!is_eligible("batch_fetch", None, 8));
        assert!(!is_eligible("web_fetch", None, 100));
    }

    fn enabled_runtime(max_concurrent: usize) -> TaskRuntime {
        TaskRuntime::from_config(&McpTasksSection {
            enabled: true,
            ttl_ms: 60_000,
            max_concurrent,
            poll_interval_ms: 250,
        })
        .expect("enabled")
    }

    #[tokio::test]
    async fn spawn_returns_a_task_handle_and_completes() {
        let runtime = enabled_runtime(2);
        let created = runtime.spawn("working", |context| {
            Box::pin(async move {
                run_cancellable(Some(context), async {
                    Ok(rmcp::model::CallToolResult::structured(
                        serde_json::json!({"ok": true}),
                    ))
                })
                .await
            })
        });
        let task_id = created.task.task_id.clone();
        for _ in 0..100 {
            if settled(&runtime, &task_id) == Some(rmcp::model::TaskStatus::Completed) {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!("task did not settle as completed");
    }

    #[tokio::test]
    async fn cancel_settles_as_cancelled() {
        let runtime = enabled_runtime(2);
        let created = runtime.spawn("working", |context| {
            Box::pin(async move {
                run_cancellable(Some(context), async {
                    std::future::pending::<Result<CallToolResult, McpError>>().await
                })
                .await
            })
        });
        let task_id = created.task.task_id.clone();
        runtime
            .manager()
            .cancel_task(&task_id)
            .expect("cancel accepted");
        for _ in 0..100 {
            if settled(&runtime, &task_id) == Some(rmcp::model::TaskStatus::Cancelled) {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!("task did not settle as cancelled");
    }

    #[tokio::test]
    async fn saturation_reports_capacity_without_error() {
        let runtime = enabled_runtime(1);
        let _created = runtime.spawn("working", |context| {
            Box::pin(async move {
                run_cancellable(Some(context), async {
                    std::future::pending::<Result<CallToolResult, McpError>>().await
                })
                .await
            })
        });
        assert!(runtime.at_capacity());
    }
}
