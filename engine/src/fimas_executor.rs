//! FIMAS Executor — Fractal Intelligent Multi-Agent System orchestration.
//!
//! Takes a DecompositionPlan and executes it by spawning agents for each sub-task,
//! respecting dependencies, managing budgets, and collecting results.

use crate::agent_runner::{AgentBackend, AgentResult, AgentRunConfig, AgentRunner};
use crate::task_queue::{QueuedTask, TaskPriority, TaskQueue};
use aethel_contracts::{
    AgentId, AgentSpec, BudgetLease, CapValue, CapabilityId,
    DecompositionPlan, RiskLevel, AethelError,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Result of a full FIMAS execution run.
#[derive(Clone, Debug)]
pub struct FimasResult {
    pub plan_id: String,
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
    pub plan_id: String,
    pub max_concurrent_agents: usize,
    pub agent_timeout: Duration,
    pub fail_fast: bool,
}

impl Default for FimasConfig {
    fn default() -> Self {
        Self {
            plan_id: "default".to_string(),
            max_concurrent_agents: 4,
            agent_timeout: Duration::from_secs(300),
            fail_fast: false,
        }
    }
}

impl FimasConfig {
    pub fn new(plan_id: impl Into<String>) -> Self {
        Self {
            plan_id: plan_id.into(),
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
    pub fn new(backend: Arc<dyn AgentBackend>, config: FimasConfig) -> Self {
        let runner = Arc::new(AgentRunner::new(backend));
        Self { runner, config }
    }

    /// Execute a decomposition plan.
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
            let mut qt = QueuedTask::new(&sub_task.id, sub_task.capability_id.as_str());
            qt = qt.with_input(&sub_task.description);
            qt = qt.with_budget(sub_task.max_tokens, sub_task.max_cost_cents as u64);

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
            let mut handles = Vec::new();
            let active = self.runner.active_count().await;
            let slots = self.config.max_concurrent_agents.saturating_sub(active);

            for _ in 0..slots {
                if let Some(task) = queue.dequeue() {
                    let runner = self.runner.clone();
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
                            capability_id: CapabilityId::new(&task.capability_name),
                            input_prompt: task.input_description.clone(),
                            max_tokens: task.max_tokens,
                            max_cost_cents: task.max_cost_cents as f32,
                            max_duration_ms: 60_000,
                            risk_level: RiskLevel::Low,
                            depth: 0,
                            parent_agent_id: None,
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

            if handles.is_empty() {
                tokio::time::sleep(Duration::from_millis(10)).await;
                continue;
            }

            for handle in handles {
                let _ = handle.await;
            }

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
            plan_id: plan.plan_id.clone(),
            agent_results,
            total_tokens,
            total_cost_cents: total_cost,
            elapsed: start.elapsed(),
            success: failed.is_empty(),
            failed_tasks: failed,
        }
    }

    fn create_sub_lease(&self, root: &BudgetLease, task: &QueuedTask) -> BudgetLease {
        BudgetLease {
            lease_id: format!("lease-{}", task.id),
            mission_id: root.mission_id.clone(),
            max_tokens: task.max_tokens,
            max_cost_cents: task.max_cost_cents as f32,
            max_duration_ms: root.max_duration_ms,
            tokens_used: 0,
            cost_used_cents: 0.0,
            granted_at_ms: 0,
            expires_at_ms: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_runner::EchoBackend;
    use aethel_contracts::{DecompositionStrategy, SubTask};

    fn make_budget() -> BudgetLease {
        BudgetLease {
            lease_id: "root-lease".to_string(),
            mission_id: "test-mission".to_string(),
            max_tokens: 100_000,
            max_cost_cents: 5000.0,
            max_duration_ms: 300_000,
            tokens_used: 0,
            cost_used_cents: 0.0,
            granted_at_ms: 0,
            expires_at_ms: 0,
        }
    }

    fn make_plan_linear() -> DecompositionPlan {
        DecompositionPlan {
            plan_id: "plan-linear".to_string(),
            original_task: "Linear test".to_string(),
            strategy: DecompositionStrategy::Sequential,
            sub_tasks: vec![
                SubTask {
                    id: "step-1".to_string(),
                    description: "First step".to_string(),
                    capability_id: CapabilityId::new("cap_a"),
                    depends_on: vec![],
                    max_tokens: 1000,
                    max_cost_cents: 10.0,
                    risk_level: RiskLevel::Low,
                    depth: 1,
                    can_decompose_further: false,
                    input_prompt: "do step 1".to_string(),
                },
                SubTask {
                    id: "step-2".to_string(),
                    description: "Second step".to_string(),
                    capability_id: CapabilityId::new("cap_b"),
                    depends_on: vec!["step-1".to_string()],
                    max_tokens: 2000,
                    max_cost_cents: 20.0,
                    risk_level: RiskLevel::Low,
                    depth: 1,
                    can_decompose_further: false,
                    input_prompt: "do step 2".to_string(),
                },
            ],
            total_budget_tokens: 3000,
            total_budget_cost_cents: 30.0,
            max_depth: 2,
        }
    }

    fn make_plan_parallel() -> DecompositionPlan {
        let tasks: Vec<SubTask> = ["a", "b", "c"]
            .iter()
            .map(|name| SubTask {
                id: name.to_string(),
                description: format!("Task {}", name),
                capability_id: CapabilityId::new("cap"),
                depends_on: vec![],
                max_tokens: 500,
                max_cost_cents: 5.0,
                risk_level: RiskLevel::Low,
                depth: 1,
                can_decompose_further: false,
                input_prompt: format!("do {}", name),
            })
            .collect();

        DecompositionPlan {
            plan_id: "plan-parallel".to_string(),
            original_task: "Parallel test".to_string(),
            strategy: DecompositionStrategy::Parallel,
            sub_tasks: tasks,
            total_budget_tokens: 1500,
            total_budget_cost_cents: 15.0,
            max_depth: 1,
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
    }

    #[tokio::test]
    async fn test_fail_fast() {
        let backend = Arc::new(EchoBackend::failing());
        let config = FimasConfig::new("test").with_concurrency(1).with_fail_fast(true);
        let executor = FimasExecutor::new(backend, config);
        let result = executor
            .execute_plan(&make_plan_linear(), &make_budget(), CapValue::Text("input".into()))
            .await;
        assert!(!result.success);
    }

    #[tokio::test]
    async fn test_budget_sub_leasing() {
        let backend = Arc::new(EchoBackend::new());
        let config = FimasConfig::new("test").with_concurrency(2);
        let executor = FimasExecutor::new(backend, config);
        let result = executor
            .execute_plan(&make_plan_parallel(), &make_budget(), CapValue::Text("input".into()))
            .await;
        assert!(result.success);
        assert_eq!(result.total_cost_cents, 15); // 5 per call * 3 agents
    }
}
