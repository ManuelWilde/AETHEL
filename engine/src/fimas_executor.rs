//! FIMAS Executor — Fractal Intelligent Multi-Agent System orchestration.
//!
//! Takes a DecompositionPlan and executes it by spawning agents for each sub-task,
//! respecting dependencies, managing budgets, and collecting results.

use crate::agent_runner::{AgentBackend, AgentResult, AgentRunConfig, AgentRunner};
use crate::task_queue::{QueuedTask, TaskId, TaskPriority, TaskQueue};
use aethel_contracts::{
    AgentId, AgentSpec, AgentState, BudgetLease, CapValue, DecompositionPlan,
    LeaseId, MissionId, SubTask, AethelError,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Result of a full FIMAS execution run.
#[derive(Clone, Debug)]
pub struct FimasResult {
    pub mission_id: MissionId,
    pub agent_results: Vec<AgentResult>,
    pub total_tokens: u64,
    pub total_cost_cents: u64,
    pub elapsed: Duration,
    pub success: bool,
    pub failed_tasks: Vec<String>,
}

impl FimasResult {
    pub fn success_rate(&self) -> f64 {
        if self.agent_results.is_empty() {
            return 0.0;
        }
        let successes = self.agent_results.iter().filter(|r| r.is_success()).count();
        successes as f64 / self.agent_results.len() as f64
    }
}

/// Configuration for a FIMAS execution run.
#[derive(Clone, Debug)]
pub struct FimasConfig {
    pub mission_id: MissionId,
    pub max_concurrent_agents: usize,
    pub agent_timeout: Duration,
    pub fail_fast: bool, // stop on first failure
}

impl Default for FimasConfig {
    fn default() -> Self {
        Self {
            mission_id: MissionId::new("default-mission"),
            max_concurrent_agents: 4,
            agent_timeout: Duration::from_secs(300),
            fail_fast: false,
        }
    }
}

impl FimasConfig {
    pub fn new(mission_id: impl Into<String>) -> Self {
        Self {
            mission_id: MissionId::new(mission_id),
            ..Default::default()
        }
    }

    pub fn with_concurrency(mut self, n: usize) -> Self {
        self.max_concurrent_agents = n;
        self
    }

    pub fn with_fail_fast(mut self, ff: bool) -> Self {
        self.fail_fast = ff;
        self
    }
}

/// The FIMAS executor orchestrates multi-agent plan execution.
pub struct FimasExecutor {
    runner: Arc<AgentRunner>,
    config: FimasConfig,
}

impl FimasExecutor {
    /// Create a new executor with the given agent backend and config.
    pub fn new(backend: Arc<dyn AgentBackend>, config: FimasConfig) -> Self {
        let runner = Arc::new(AgentRunner::new(backend));
        Self { runner, config }
    }

    /// Execute a decomposition plan.
    ///
    /// Converts SubTasks into QueuedTasks, respects dependencies,
    /// runs agents with budget sub-leases, and collects results.
    pub async fn execute_plan(
        &self,
        plan: &DecompositionPlan,
        root_budget: &BudgetLease,
        root_input: CapValue,
    ) -> FimasResult {
        let start = Instant::now();
        let queue = TaskQueue::new();
        let results: Arc<Mutex<Vec<AgentResult>>> = Arc::new(Mutex::new(Vec::new()));
        let failed_tasks: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        // Enqueue all sub-tasks from the plan
        for sub_task in &plan.sub_tasks {
            let mut qt = QueuedTask::new(&sub_task.id, &sub_task.capability_name);
            qt = qt.with_input(&sub_task.description);
            qt = qt.with_budget(sub_task.estimated_tokens, sub_task.estimated_cost_cents);

            // Map priority from depth: root tasks get High, deeper get Normal/Low
            qt = if sub_task.depends_on.is_empty() {
                qt.with_priority(TaskPriority::High)
            } else {
                qt.with_priority(TaskPriority::Normal)
            };

            for dep in &sub_task.depends_on {
                qt = qt.depends_on(dep.as_str());
            }

            queue.enqueue(qt);
        }

        // Process queue until all done
        let mut abort = false;
        while !queue.is_all_done() && !abort {
            // Dequeue ready tasks up to concurrency limit
            let mut handles = Vec::new();
            let active = self.runner.active_count().await;
            let slots = self.config.max_concurrent_agents.saturating_sub(active);

            for _ in 0..slots {
                if let Some(task) = queue.dequeue() {
                    let runner = self.runner.clone();
                    let mission_id = self.config.mission_id.clone();
                    let timeout = self.config.agent_timeout;
                    let budget = self.create_sub_lease(root_budget, &task);
                    let input = root_input.clone();
                    let queue_ref = queue.clone();
                    let results_ref = results.clone();
                    let failed_ref = failed_tasks.clone();
                    let task_id = task.id.clone();

                    let handle = tokio::spawn(async move {
                        let spec = AgentSpec {
                            agent_id: AgentId::new(format!("agent-{}", task.id)),
                            mission_id,
                            capability_name: task.capability_name.clone(),
                            model_preference: None,
                            max_depth: 3,
                        };

                        let config = AgentRunConfig::new(
                            spec.agent_id.clone(),
                            spec,
                            budget,
                            input,
                        )
                        .with_timeout(timeout);

                        let result = runner.run(config).await;

                        if result.is_success() {
                            queue_ref.complete(&task_id);
                        } else {
                            queue_ref.fail(&task_id);
                            let mut f = failed_ref.lock().await;
                            f.push(task.id.0.clone());
                        }

                        let mut r = results_ref.lock().await;
                        r.push(result);
                    });

                    handles.push(handle);
                } else {
                    break;
                }
            }

            // If no handles spawned and nothing dequeued, wait briefly
            if handles.is_empty() {
                tokio::time::sleep(Duration::from_millis(10)).await;
                continue;
            }

            // Wait for this batch
            for handle in handles {
                let _ = handle.await;
            }

            // Check fail_fast
            if self.config.fail_fast {
                let f = failed_tasks.lock().await;
                if !f.is_empty() {
                    abort = true;
                }
            }
        }

        let agent_results = results.lock().await.clone();
        let total_tokens: u64 = agent_results.iter().map(|r| r.tokens_used).sum();
        let total_cost: u64 = agent_results.iter().map(|r| r.cost_used_cents).sum();
        let failed = failed_tasks.lock().await.clone();

        FimasResult {
            mission_id: self.config.mission_id.clone(),
            agent_results,
            total_tokens,
            total_cost_cents: total_cost,
            elapsed: start.elapsed(),
            success: failed.is_empty(),
            failed_tasks: failed,
        }
    }

    /// Create a sub-lease for a specific task from the root budget.
    fn create_sub_lease(&self, root: &BudgetLease, task: &QueuedTask) -> BudgetLease {
        BudgetLease {
            lease_id: LeaseId::new(format!("lease-{}", task.id)),
            parent_lease: Some(root.lease_id.clone()),
            max_tokens: task.max_tokens,
            max_cost_cents: task.max_cost_cents,
            used_tokens: 0,
            used_cost_cents: 0,
            max_depth: root.max_depth.saturating_sub(1),
            current_depth: root.current_depth + 1,
            max_duration_ms: root.max_duration_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_runner::EchoBackend;
    use aethel_contracts::DecompositionStrategy;

    fn make_budget() -> BudgetLease {
        BudgetLease {
            lease_id: LeaseId::new("root-lease"),
            parent_lease: None,
            max_tokens: 100_000,
            max_cost_cents: 5_000,
            used_tokens: 0,
            used_cost_cents: 0,
            max_depth: 5,
            current_depth: 0,
            max_duration_ms: 300_000,
        }
    }

    fn make_plan_linear() -> DecompositionPlan {
        DecompositionPlan {
            mission_id: MissionId::new("test-mission"),
            strategy: DecompositionStrategy::Sequential,
            sub_tasks: vec![
                SubTask {
                    id: "step-1".to_string(),
                    description: "First step".to_string(),
                    capability_name: "cap_a".to_string(),
                    depends_on: vec![],
                    estimated_tokens: 1000,
                    estimated_cost_cents: 10,
                    depth: 1,
                },
                SubTask {
                    id: "step-2".to_string(),
                    description: "Second step".to_string(),
                    capability_name: "cap_b".to_string(),
                    depends_on: vec!["step-1".to_string()],
                    estimated_tokens: 2000,
                    estimated_cost_cents: 20,
                    depth: 1,
                },
            ],
        }
    }

    fn make_plan_parallel() -> DecompositionPlan {
        DecompositionPlan {
            mission_id: MissionId::new("test-mission"),
            strategy: DecompositionStrategy::Parallel,
            sub_tasks: vec![
                SubTask {
                    id: "a".to_string(),
                    description: "Task A".to_string(),
                    capability_name: "cap".to_string(),
                    depends_on: vec![],
                    estimated_tokens: 500,
                    estimated_cost_cents: 5,
                    depth: 1,
                },
                SubTask {
                    id: "b".to_string(),
                    description: "Task B".to_string(),
                    capability_name: "cap".to_string(),
                    depends_on: vec![],
                    estimated_tokens: 500,
                    estimated_cost_cents: 5,
                    depth: 1,
                },
                SubTask {
                    id: "c".to_string(),
                    description: "Task C".to_string(),
                    capability_name: "cap".to_string(),
                    depends_on: vec![],
                    estimated_tokens: 500,
                    estimated_cost_cents: 5,
                    depth: 1,
                },
            ],
        }
    }

    #[tokio::test]
    async fn test_linear_execution() {
        let backend = Arc::new(EchoBackend::new());
        let config = FimasConfig::new("test").with_concurrency(1);
        let executor = FimasExecutor::new(backend, config);

        let result = executor
            .execute_plan(&make_plan_linear(), &make_budget(), CapValue::Text("input".into()))
            .await;

        assert!(result.success);
        assert_eq!(result.agent_results.len(), 2);
        assert_eq!(result.total_tokens, 200); // 100 per call from EchoBackend
    }

    #[tokio::test]
    async fn test_parallel_execution() {
        let backend = Arc::new(EchoBackend::new());
        let config = FimasConfig::new("test").with_concurrency(4);
        let executor = FimasExecutor::new(backend, config);

        let result = executor
            .execute_plan(&make_plan_parallel(), &make_budget(), CapValue::Text("input".into()))
            .await;

        assert!(result.success);
        assert_eq!(result.agent_results.len(), 3);
        assert!(result.success_rate() > 0.99);
    }

    #[tokio::test]
    async fn test_fail_fast() {
        let backend = Arc::new(crate::agent_runner::EchoBackend::failing());
        let config = FimasConfig::new("test").with_concurrency(1).with_fail_fast(true);
        let executor = FimasExecutor::new(backend, config);

        let result = executor
            .execute_plan(&make_plan_linear(), &make_budget(), CapValue::Text("input".into()))
            .await;

        assert!(!result.success);
        assert!(!result.failed_tasks.is_empty());
    }

    #[tokio::test]
    async fn test_budget_sub_leasing() {
        let backend = Arc::new(EchoBackend::new());
        let config = FimasConfig::new("test").with_concurrency(2);
        let executor = FimasExecutor::new(backend, config);

        let root = make_budget();
        let result = executor
            .execute_plan(&make_plan_parallel(), &root, CapValue::Text("input".into()))
            .await;

        assert!(result.success);
        // Total cost should be sum of per-agent costs
        assert_eq!(result.total_cost_cents, 15); // 5 per call * 3 agents
    }
}
