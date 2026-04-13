//! FIMAS Decomposer — fractal task decomposition.
//!
//! The decomposer takes a complex task and breaks it into sub-tasks
//! that can be handled by smaller, cheaper LLMs.

use crate::{
    AethelError, CapabilityId, RiskLevel,
};
use serde::{Deserialize, Serialize};

/// How a task should be decomposed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DecompositionStrategy {
    /// No decomposition — run directly.
    Direct,
    /// Split into sequential sub-tasks (pipeline).
    Sequential,
    /// Split into parallel independent sub-tasks.
    Parallel,
    /// Recursive: sub-tasks can themselves be decomposed further.
    Recursive,
    /// Map-reduce: split input, process in parallel, aggregate.
    MapReduce,
}

/// A single sub-task in a decomposition plan.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubTask {
    /// Unique ID within the plan.
    pub id: String,
    /// Human-readable description.
    pub description: String,
    /// The capability to use.
    pub capability_id: CapabilityId,
    /// Dependencies: IDs of sub-tasks that must complete first.
    pub depends_on: Vec<String>,
    /// Maximum tokens allocated.
    pub max_tokens: u64,
    /// Maximum cost allocated in cents.
    pub max_cost_cents: f32,
    /// Risk level.
    pub risk_level: RiskLevel,
    /// Fractal depth (0 = leaf).
    pub depth: u32,
    /// Whether this can be further decomposed.
    pub can_decompose_further: bool,
    /// The input prompt/data.
    pub input_prompt: String,
}

/// A complete decomposition plan.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecompositionPlan {
    /// Unique plan ID.
    pub plan_id: String,
    /// The original task description.
    pub original_task: String,
    /// Strategy used.
    pub strategy: DecompositionStrategy,
    /// The sub-tasks in execution order.
    pub sub_tasks: Vec<SubTask>,
    /// Total token budget.
    pub total_budget_tokens: u64,
    /// Total cost budget.
    pub total_budget_cost_cents: f32,
    /// Maximum fractal depth.
    pub max_depth: u32,
}

impl DecompositionPlan {
    /// Validate the plan: budget constraints and dependencies.
    pub fn validate(&self) -> Result<(), AethelError> {
        if self.sub_tasks.is_empty() {
            return Err(AethelError::Other("Decomposition plan has no sub-tasks".into()));
        }

        let total_tokens: u64 = self.sub_tasks.iter().map(|s| s.max_tokens).sum();
        if total_tokens > self.total_budget_tokens {
            return Err(AethelError::BudgetExceeded(format!(
                "Sub-task token sum {} > plan budget {}", total_tokens, self.total_budget_tokens
            )));
        }

        let total_cost: f32 = self.sub_tasks.iter().map(|s| s.max_cost_cents).sum();
        if total_cost > self.total_budget_cost_cents {
            return Err(AethelError::BudgetExceeded(format!(
                "Sub-task cost sum {:.2} > plan budget {:.2}", total_cost, self.total_budget_cost_cents
            )));
        }

        let ids: Vec<&str> = self.sub_tasks.iter().map(|s| s.id.as_str()).collect();
        for (i, task) in self.sub_tasks.iter().enumerate() {
            for dep in &task.depends_on {
                if !ids.contains(&dep.as_str()) {
                    return Err(AethelError::Other(format!(
                        "Sub-task '{}' depends on unknown task '{}'", task.id, dep
                    )));
                }
                let dep_idx = ids.iter().position(|id| *id == dep.as_str());
                if let Some(dep_idx) = dep_idx {
                    if dep_idx >= i {
                        return Err(AethelError::Other(format!(
                            "Sub-task '{}' depends on '{}' which is defined later", task.id, dep
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    /// Get sub-tasks with no dependencies (can start immediately).
    pub fn root_tasks(&self) -> Vec<&SubTask> {
        self.sub_tasks.iter().filter(|t| t.depends_on.is_empty()).collect()
    }

    /// Get sub-tasks that depend on a given task ID.
    pub fn dependents_of(&self, task_id: &str) -> Vec<&SubTask> {
        self.sub_tasks.iter()
            .filter(|t| t.depends_on.iter().any(|d| d == task_id))
            .collect()
    }

    /// Maximum depth across all sub-tasks.
    pub fn actual_max_depth(&self) -> u32 {
        self.sub_tasks.iter().map(|t| t.depth).max().unwrap_or(0)
    }
}

/// The decomposer trait — splits a task into sub-tasks.
#[async_trait::async_trait]
pub trait Decomposer: Send + Sync {
    /// Decompose a task into a plan of sub-tasks.
    async fn decompose(
        &self,
        task_description: &str,
        available_budget_tokens: u64,
        available_budget_cost: f32,
        max_depth: u32,
    ) -> Result<DecompositionPlan, AethelError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sub_task(id: &str, tokens: u64, cost: f32, deps: Vec<&str>) -> SubTask {
        SubTask {
            id: id.into(),
            description: format!("Task {}", id),
            capability_id: CapabilityId::new("mock"),
            depends_on: deps.into_iter().map(String::from).collect(),
            max_tokens: tokens,
            max_cost_cents: cost,
            risk_level: RiskLevel::Low,
            depth: 0,
            can_decompose_further: false,
            input_prompt: "do something".into(),
        }
    }

    fn make_plan(sub_tasks: Vec<SubTask>, total_tokens: u64, total_cost: f32) -> DecompositionPlan {
        DecompositionPlan {
            plan_id: "plan-1".into(),
            original_task: "test task".into(),
            strategy: DecompositionStrategy::Sequential,
            sub_tasks,
            total_budget_tokens: total_tokens,
            total_budget_cost_cents: total_cost,
            max_depth: 3,
        }
    }

    #[test]
    fn test_valid_plan() {
        let plan = make_plan(
            vec![make_sub_task("a", 100, 1.0, vec![]), make_sub_task("b", 200, 2.0, vec!["a"])],
            500, 5.0,
        );
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn test_empty_plan_invalid() {
        let plan = make_plan(vec![], 500, 5.0);
        assert!(plan.validate().is_err());
    }

    #[test]
    fn test_budget_exceeded_tokens() {
        let plan = make_plan(
            vec![make_sub_task("a", 300, 1.0, vec![]), make_sub_task("b", 300, 1.0, vec!["a"])],
            500, 5.0,
        );
        assert!(plan.validate().is_err());
    }

    #[test]
    fn test_budget_exceeded_cost() {
        let plan = make_plan(
            vec![make_sub_task("a", 100, 3.0, vec![]), make_sub_task("b", 100, 3.0, vec!["a"])],
            500, 5.0,
        );
        assert!(plan.validate().is_err());
    }

    #[test]
    fn test_unknown_dependency() {
        let plan = make_plan(vec![make_sub_task("a", 100, 1.0, vec!["nonexistent"])], 500, 5.0);
        assert!(plan.validate().is_err());
    }

    #[test]
    fn test_forward_dependency_invalid() {
        let plan = make_plan(
            vec![make_sub_task("a", 100, 1.0, vec!["b"]), make_sub_task("b", 100, 1.0, vec![])],
            500, 5.0,
        );
        assert!(plan.validate().is_err());
    }

    #[test]
    fn test_root_tasks() {
        let plan = make_plan(
            vec![
                make_sub_task("a", 100, 1.0, vec![]),
                make_sub_task("b", 100, 1.0, vec![]),
                make_sub_task("c", 100, 1.0, vec!["a", "b"]),
            ],
            500, 5.0,
        );
        assert_eq!(plan.root_tasks().len(), 2);
    }

    #[test]
    fn test_dependents_of() {
        let plan = make_plan(
            vec![
                make_sub_task("a", 100, 1.0, vec![]),
                make_sub_task("b", 100, 1.0, vec!["a"]),
                make_sub_task("c", 100, 1.0, vec!["a"]),
            ],
            500, 5.0,
        );
        assert_eq!(plan.dependents_of("a").len(), 2);
    }

    #[test]
    fn test_actual_max_depth() {
        let mut plan = make_plan(
            vec![make_sub_task("a", 100, 1.0, vec![]), make_sub_task("b", 100, 1.0, vec!["a"])],
            500, 5.0,
        );
        plan.sub_tasks[0].depth = 1;
        plan.sub_tasks[1].depth = 2;
        assert_eq!(plan.actual_max_depth(), 2);
    }

    #[test]
    fn test_strategy_serde() {
        let s = DecompositionStrategy::MapReduce;
        let json = serde_json::to_string(&s).unwrap();
        let restored: DecompositionStrategy = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, DecompositionStrategy::MapReduce);
    }

    #[test]
    fn test_plan_serde() {
        let plan = make_plan(vec![make_sub_task("a", 100, 1.0, vec![])], 500, 5.0);
        let json = serde_json::to_string(&plan).unwrap();
        let restored: DecompositionPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.plan_id, "plan-1");
    }

    #[test]
    fn test_parallel_all_roots() {
        let plan = DecompositionPlan {
            plan_id: "p1".into(),
            original_task: "parallel task".into(),
            strategy: DecompositionStrategy::Parallel,
            sub_tasks: vec![
                make_sub_task("a", 100, 1.0, vec![]),
                make_sub_task("b", 100, 1.0, vec![]),
                make_sub_task("c", 100, 1.0, vec![]),
            ],
            total_budget_tokens: 500,
            total_budget_cost_cents: 5.0,
            max_depth: 1,
        };
        assert!(plan.validate().is_ok());
        assert_eq!(plan.root_tasks().len(), 3);
    }
}
