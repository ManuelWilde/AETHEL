# P2-01: FIMAS Decomposer — Fractal Task Decomposition

## Prerequisites

P1-04 must be merged to main.

## Context

You are working on the AETHEL project — a Rust workspace.
Phase 0+1 built: AethelError, IDs, ClaimState, BudgetLease, Capability, Pipeline, Registry, Executor.
Now we build the FIMAS Decomposer — the component that takes a large task and fractally decomposes it into sub-tasks that small LLMs can handle.

Key insight: The decomposer itself uses a LARGE model ONCE to create the decomposition plan. Then each sub-task runs on a SMALL model. This is the core FIMAS economy.

## Git Branch

```bash
git checkout main && git pull
git checkout -b P2-01-fimas-decomposer
```

## Your Task

1. Create `contracts/src/decomposer.rs` with:
   - `DecompositionStrategy` enum (how to split tasks)
   - `SubTask` struct (a single decomposed unit of work)
   - `DecompositionPlan` struct (the full plan)
   - `Decomposer` trait (async, the decomposition abstraction)
   - Validation: no sub-task exceeds parent budget, depth limits respected
2. Add `pub mod decomposer; pub use decomposer::*;` to `contracts/src/lib.rs`

## Exact Code

### contracts/src/decomposer.rs:
```rust
//! FIMAS Decomposer — fractal task decomposition.
//!
//! The decomposer takes a complex task and breaks it into sub-tasks
//! that can be handled by smaller, cheaper LLMs. This is the core
//! of the FIMAS fractal architecture:
//!
//! 1. A LARGE model (expensive) creates the decomposition plan ONCE
//! 2. Each sub-task is routed to a SMALL model (cheap) for execution
//! 3. Results are aggregated back up the fractal tree
//!
//! # Budget Rule
//! The sum of all sub-task budgets must NOT exceed the parent budget.
//! Each sub-task gets a BudgetLease carved from the parent via sub_lease().
//!
//! # Depth Rule
//! Fractal depth is bounded by OmegaSpectrum24.fractal_depth().
//! The ResponsibleScalingGate ensures depth ≤ verification capacity.

use crate::{
    AethelError, CapabilityId, RiskLevel,
};
use serde::{Deserialize, Serialize};

/// How a task should be decomposed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DecompositionStrategy {
    /// No decomposition — run directly on a single model.
    Direct,
    /// Split into sequential sub-tasks (pipeline).
    Sequential,
    /// Split into parallel independent sub-tasks.
    Parallel,
    /// Recursive: sub-tasks can themselves be decomposed further.
    Recursive,
    /// Map-reduce: split input, process in parallel, aggregate results.
    MapReduce,
}

/// A single sub-task in a decomposition plan.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubTask {
    /// Unique ID within the plan.
    pub id: String,
    /// Human-readable description.
    pub description: String,
    /// The capability to use for this sub-task.
    pub capability_id: CapabilityId,
    /// Dependencies: IDs of sub-tasks that must complete before this one.
    pub depends_on: Vec<String>,
    /// Maximum tokens allocated for this sub-task.
    pub max_tokens: u64,
    /// Maximum cost allocated in cents.
    pub max_cost_cents: f32,
    /// Risk level of this sub-task.
    pub risk_level: RiskLevel,
    /// Fractal depth of this sub-task (0 = leaf).
    pub depth: u32,
    /// Whether this sub-task can be further decomposed.
    pub can_decompose_further: bool,
    /// The input prompt/data for this sub-task.
    pub input_prompt: String,
}

/// A complete decomposition plan.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecompositionPlan {
    /// Unique plan ID.
    pub plan_id: String,
    /// The original task description.
    pub original_task: String,
    /// Strategy used for decomposition.
    pub strategy: DecompositionStrategy,
    /// The sub-tasks in execution order.
    pub sub_tasks: Vec<SubTask>,
    /// Total budget allocated across all sub-tasks (tokens).
    pub total_budget_tokens: u64,
    /// Total budget allocated across all sub-tasks (cost).
    pub total_budget_cost_cents: f32,
    /// Maximum fractal depth in this plan.
    pub max_depth: u32,
}

impl DecompositionPlan {
    /// Validate the plan: check budget constraints and dependencies.
    ///
    /// Rules:
    /// 1. Sum of sub-task token budgets ≤ total_budget_tokens
    /// 2. Sum of sub-task cost budgets ≤ total_budget_cost_cents
    /// 3. All dependency IDs must reference existing sub-tasks
    /// 4. No circular dependencies (simple check: depends_on IDs must be earlier)
    /// 5. At least one sub-task
    pub fn validate(&self) -> Result<(), AethelError> {
        if self.sub_tasks.is_empty() {
            return Err(AethelError::Other(
                "Decomposition plan has no sub-tasks".into()
            ));
        }

        // Budget check
        let total_tokens: u64 = self.sub_tasks.iter().map(|s| s.max_tokens).sum();
        if total_tokens > self.total_budget_tokens {
            return Err(AethelError::BudgetExceeded(format!(
                "Sub-task token sum {} > plan budget {}",
                total_tokens, self.total_budget_tokens
            )));
        }

        let total_cost: f32 = self.sub_tasks.iter().map(|s| s.max_cost_cents).sum();
        if total_cost > self.total_budget_cost_cents {
            return Err(AethelError::BudgetExceeded(format!(
                "Sub-task cost sum {:.2} > plan budget {:.2}",
                total_cost, self.total_budget_cost_cents
            )));
        }

        // Dependency check
        let ids: Vec<&str> = self.sub_tasks.iter().map(|s| s.id.as_str()).collect();
        for (i, task) in self.sub_tasks.iter().enumerate() {
            for dep in &task.depends_on {
                if !ids.contains(&dep.as_str()) {
                    return Err(AethelError::Other(format!(
                        "Sub-task '{}' depends on unknown task '{}'",
                        task.id, dep
                    )));
                }
                // Simple ordering check: dependency must be defined before this task
                let dep_idx = ids.iter().position(|id| *id == dep.as_str());
                if let Some(dep_idx) = dep_idx {
                    if dep_idx >= i {
                        return Err(AethelError::Other(format!(
                            "Sub-task '{}' depends on '{}' which is defined later (circular or misordered)",
                            task.id, dep
                        )));
                    }
                }
            }
        }

        Ok(())
    }

    /// Get sub-tasks that have no dependencies (can start immediately).
    pub fn root_tasks(&self) -> Vec<&SubTask> {
        self.sub_tasks.iter().filter(|t| t.depends_on.is_empty()).collect()
    }

    /// Get sub-tasks that depend on a given task ID.
    pub fn dependents_of(&self, task_id: &str) -> Vec<&SubTask> {
        self.sub_tasks.iter()
            .filter(|t| t.depends_on.iter().any(|d| d == task_id))
            .collect()
    }

    /// Get the maximum depth across all sub-tasks.
    pub fn actual_max_depth(&self) -> u32 {
        self.sub_tasks.iter().map(|t| t.depth).max().unwrap_or(0)
    }
}

/// The decomposer trait — splits a task into sub-tasks.
///
/// Implementations use a LARGE model to create the plan,
/// then return the plan for execution by SMALL models.
#[async_trait::async_trait]
pub trait Decomposer: Send + Sync {
    /// Decompose a task into a plan of sub-tasks.
    ///
    /// # Arguments
    /// - `task_description`: What needs to be done
    /// - `available_budget_tokens`: Maximum tokens for the entire task
    /// - `available_budget_cost`: Maximum cost in cents
    /// - `max_depth`: Maximum fractal depth allowed
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
            vec![
                make_sub_task("a", 100, 1.0, vec![]),
                make_sub_task("b", 200, 2.0, vec!["a"]),
            ],
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
            vec![
                make_sub_task("a", 300, 1.0, vec![]),
                make_sub_task("b", 300, 1.0, vec!["a"]),
            ],
            500, 5.0, // 600 > 500
        );
        assert!(plan.validate().is_err());
    }

    #[test]
    fn test_budget_exceeded_cost() {
        let plan = make_plan(
            vec![
                make_sub_task("a", 100, 3.0, vec![]),
                make_sub_task("b", 100, 3.0, vec!["a"]),
            ],
            500, 5.0, // 6.0 > 5.0
        );
        assert!(plan.validate().is_err());
    }

    #[test]
    fn test_unknown_dependency() {
        let plan = make_plan(
            vec![
                make_sub_task("a", 100, 1.0, vec!["nonexistent"]),
            ],
            500, 5.0,
        );
        assert!(plan.validate().is_err());
    }

    #[test]
    fn test_forward_dependency_invalid() {
        let plan = make_plan(
            vec![
                make_sub_task("a", 100, 1.0, vec!["b"]), // b defined after a
                make_sub_task("b", 100, 1.0, vec![]),
            ],
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
        let roots = plan.root_tasks();
        assert_eq!(roots.len(), 2);
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
        let deps = plan.dependents_of("a");
        assert_eq!(deps.len(), 2);
    }

    #[test]
    fn test_actual_max_depth() {
        let mut plan = make_plan(
            vec![
                make_sub_task("a", 100, 1.0, vec![]),
                make_sub_task("b", 100, 1.0, vec!["a"]),
            ],
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
        let plan = make_plan(
            vec![make_sub_task("a", 100, 1.0, vec![])],
            500, 5.0,
        );
        let json = serde_json::to_string(&plan).unwrap();
        let restored: DecompositionPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.plan_id, "plan-1");
        assert_eq!(restored.sub_tasks.len(), 1);
    }

    #[test]
    fn test_parallel_strategy_all_roots() {
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
```

### contracts/src/lib.rs — add module declaration:
```rust
pub mod decomposer;
pub use decomposer::*;
```

## Validation

```bash
cd contracts && cargo test --workspace 2>&1
```

Expected: All tests pass, zero warnings.

## Done Criteria

- [ ] `contracts/src/decomposer.rs` exists
- [ ] `DecompositionStrategy` enum with 5 variants
- [ ] `SubTask` struct with budget, deps, depth
- [ ] `DecompositionPlan` with validate(), root_tasks(), dependents_of()
- [ ] `Decomposer` trait (async)
- [ ] Budget validation: sum ≤ total
- [ ] Dependency validation: no unknowns, no forward refs
- [ ] 12+ tests pass
- [ ] All previous tests still pass

## Git

```bash
git add -A
git commit -m "P2-01: FIMAS Decomposer — fractal task decomposition

- DecompositionStrategy: Direct, Sequential, Parallel, Recursive, MapReduce
- SubTask with budget allocation and dependency graph
- DecompositionPlan with validate(), root_tasks(), dependents_of()
- Decomposer trait for pluggable decomposition implementations
- 12+ tests including budget and dependency validation"
git push -u origin P2-01-fimas-decomposer
gh pr create --title "P2-01: FIMAS Decomposer" --body "$(cat tasks/P2-01-fimas-decomposer.md)"
```
