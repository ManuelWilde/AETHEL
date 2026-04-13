# P2-03: Agent Lifecycle — Spawn, Execute, Report

## Prerequisites

P2-01 must be merged to main.

## Context

You are working on the AETHEL project — a Rust workspace.
P2-01 added: FIMAS Decomposer (SubTask, DecompositionPlan).
Now we build the Agent Lifecycle — the state machine for agents that execute sub-tasks.

## Git Branch

```bash
git checkout main && git pull
git checkout -b P2-03-agent-lifecycle
```

## Your Task

1. Create `contracts/src/agent_lifecycle.rs` with:
   - `AgentState` enum (lifecycle states)
   - `AgentReport` struct (execution results from an agent)
   - `AgentSpec` struct (what an agent needs to start)
   - `AgentLifecycle` trait (async, manages agent state transitions)
2. Add `pub mod agent_lifecycle; pub use agent_lifecycle::*;` to `contracts/src/lib.rs`

## Exact Code

### contracts/src/agent_lifecycle.rs:
```rust
//! Agent Lifecycle — spawn, execute, report.
//!
//! Every FIMAS agent follows this lifecycle:
//! 1. Created — agent spec defined, not yet started
//! 2. Initializing — loading model, setting up context
//! 3. Running — executing the sub-task
//! 4. Reporting — generating execution report
//! 5. Completed — finished successfully
//! 6. Failed — finished with error
//! 7. Cancelled — terminated by control plane
//!
//! State transitions are enforced (like ClaimState).

use crate::{
    AethelError, AgentId, CapabilityId, RiskLevel,
    ThoughtEfficiency,
};
use serde::{Deserialize, Serialize};

/// Lifecycle state of a FIMAS agent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentState {
    /// Agent has been defined but not started.
    Created,
    /// Agent is loading its model and context.
    Initializing,
    /// Agent is executing its sub-task.
    Running,
    /// Agent is generating its execution report.
    Reporting,
    /// Agent completed successfully.
    Completed,
    /// Agent failed with an error.
    Failed,
    /// Agent was cancelled by the control plane.
    Cancelled,
}

impl AgentState {
    /// Returns the valid transitions from this state.
    pub fn allowed_transitions(&self) -> &'static [AgentState] {
        use AgentState::*;
        match self {
            Created => &[Initializing, Cancelled],
            Initializing => &[Running, Failed, Cancelled],
            Running => &[Reporting, Failed, Cancelled],
            Reporting => &[Completed, Failed],
            Completed => &[],
            Failed => &[],
            Cancelled => &[],
        }
    }

    /// Check if transitioning to `target` is allowed.
    pub fn can_transition_to(&self, target: AgentState) -> bool {
        self.allowed_transitions().contains(&target)
    }

    /// Attempt to transition to `target`.
    pub fn transition(self, target: AgentState) -> Result<AgentState, AethelError> {
        if self.can_transition_to(target) {
            Ok(target)
        } else {
            Err(AethelError::InvalidTransition {
                from: crate::ClaimState::Generated, // placeholder — we reuse the error type
                to: crate::ClaimState::Generated,
            })
        }
    }

    /// Is this a terminal state?
    pub fn is_terminal(&self) -> bool {
        self.allowed_transitions().is_empty()
    }

    /// Is this an active (non-terminal) state?
    pub fn is_active(&self) -> bool {
        !self.is_terminal()
    }
}

/// Specification for spawning a new agent.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentSpec {
    /// Unique agent ID.
    pub agent_id: AgentId,
    /// The capability this agent will execute.
    pub capability_id: CapabilityId,
    /// The input prompt/data for this agent.
    pub input_prompt: String,
    /// Maximum tokens this agent can consume.
    pub max_tokens: u64,
    /// Maximum cost in cents.
    pub max_cost_cents: f32,
    /// Maximum wall-clock time in milliseconds.
    pub max_duration_ms: u64,
    /// Risk level of this agent's task.
    pub risk_level: RiskLevel,
    /// Fractal depth of this agent in the FIMAS tree.
    pub depth: u32,
    /// Parent agent ID (None if root agent).
    pub parent_agent_id: Option<AgentId>,
}

/// Report from a completed agent execution.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentReport {
    /// The agent's ID.
    pub agent_id: AgentId,
    /// Final state (Completed, Failed, or Cancelled).
    pub final_state: AgentState,
    /// The output text/data (if successful).
    pub output: Option<String>,
    /// Error message (if failed).
    pub error: Option<String>,
    /// Efficiency metrics.
    pub efficiency: Option<ThoughtEfficiency>,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// Tokens actually consumed.
    pub tokens_consumed: u64,
    /// Cost actually consumed in cents.
    pub cost_consumed_cents: f32,
}

impl AgentReport {
    /// Create a success report.
    pub fn success(
        agent_id: AgentId,
        output: String,
        efficiency: ThoughtEfficiency,
        duration_ms: u64,
        tokens: u64,
        cost: f32,
    ) -> Self {
        Self {
            agent_id,
            final_state: AgentState::Completed,
            output: Some(output),
            error: None,
            efficiency: Some(efficiency),
            duration_ms,
            tokens_consumed: tokens,
            cost_consumed_cents: cost,
        }
    }

    /// Create a failure report.
    pub fn failure(
        agent_id: AgentId,
        error: String,
        duration_ms: u64,
        tokens: u64,
        cost: f32,
    ) -> Self {
        Self {
            agent_id,
            final_state: AgentState::Failed,
            output: None,
            error: Some(error),
            efficiency: None,
            duration_ms,
            tokens_consumed: tokens,
            cost_consumed_cents: cost,
        }
    }

    /// Create a cancellation report.
    pub fn cancelled(agent_id: AgentId) -> Self {
        Self {
            agent_id,
            final_state: AgentState::Cancelled,
            output: None,
            error: None,
            efficiency: None,
            duration_ms: 0,
            tokens_consumed: 0,
            cost_consumed_cents: 0.0,
        }
    }

    /// Is this a successful completion?
    pub fn is_success(&self) -> bool {
        self.final_state == AgentState::Completed
    }
}

/// The agent lifecycle trait — manages agent execution.
#[async_trait::async_trait]
pub trait AgentLifecycle: Send + Sync {
    /// Spawn a new agent from a specification.
    async fn spawn(&self, spec: AgentSpec) -> Result<AgentId, AethelError>;

    /// Get the current state of an agent.
    async fn state(&self, agent_id: &AgentId) -> Result<AgentState, AethelError>;

    /// Cancel a running agent.
    async fn cancel(&self, agent_id: &AgentId) -> Result<(), AethelError>;

    /// Wait for an agent to complete and return its report.
    async fn wait_for_report(&self, agent_id: &AgentId) -> Result<AgentReport, AethelError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use AgentState::*;

    // ── State transition tests ──

    #[test]
    fn test_created_to_initializing() {
        assert!(Created.can_transition_to(Initializing));
    }

    #[test]
    fn test_created_to_cancelled() {
        assert!(Created.can_transition_to(Cancelled));
    }

    #[test]
    fn test_created_to_running_invalid() {
        assert!(!Created.can_transition_to(Running));
    }

    #[test]
    fn test_initializing_to_running() {
        assert!(Initializing.can_transition_to(Running));
    }

    #[test]
    fn test_initializing_to_failed() {
        assert!(Initializing.can_transition_to(Failed));
    }

    #[test]
    fn test_running_to_reporting() {
        assert!(Running.can_transition_to(Reporting));
    }

    #[test]
    fn test_running_to_cancelled() {
        assert!(Running.can_transition_to(Cancelled));
    }

    #[test]
    fn test_reporting_to_completed() {
        assert!(Reporting.can_transition_to(Completed));
    }

    #[test]
    fn test_reporting_to_failed() {
        assert!(Reporting.can_transition_to(Failed));
    }

    #[test]
    fn test_completed_is_terminal() {
        assert!(Completed.is_terminal());
        assert!(!Completed.is_active());
    }

    #[test]
    fn test_failed_is_terminal() {
        assert!(Failed.is_terminal());
    }

    #[test]
    fn test_cancelled_is_terminal() {
        assert!(Cancelled.is_terminal());
    }

    #[test]
    fn test_created_is_active() {
        assert!(Created.is_active());
    }

    #[test]
    fn test_transition_success() {
        let state = Created.transition(Initializing).unwrap();
        assert_eq!(state, Initializing);
    }

    #[test]
    fn test_transition_failure() {
        assert!(Created.transition(Completed).is_err());
    }

    #[test]
    fn test_self_transition_invalid() {
        for state in &[Created, Initializing, Running, Reporting, Completed, Failed, Cancelled] {
            assert!(!state.can_transition_to(*state));
        }
    }

    // ── Happy path lifecycle ──

    #[test]
    fn test_happy_path() {
        let s = Created;
        let s = s.transition(Initializing).unwrap();
        let s = s.transition(Running).unwrap();
        let s = s.transition(Reporting).unwrap();
        let s = s.transition(Completed).unwrap();
        assert!(s.is_terminal());
    }

    // ── AgentReport tests ──

    #[test]
    fn test_success_report() {
        let eff = ThoughtEfficiency::compute(0.9, 100, 500, 1.0, 3, 5);
        let report = AgentReport::success(
            AgentId::new("agent-1"), "result".into(), eff, 500, 100, 1.0
        );
        assert!(report.is_success());
        assert_eq!(report.output, Some("result".into()));
    }

    #[test]
    fn test_failure_report() {
        let report = AgentReport::failure(
            AgentId::new("agent-2"), "timeout".into(), 1000, 50, 0.5
        );
        assert!(!report.is_success());
        assert_eq!(report.error, Some("timeout".into()));
    }

    #[test]
    fn test_cancelled_report() {
        let report = AgentReport::cancelled(AgentId::new("agent-3"));
        assert!(!report.is_success());
        assert_eq!(report.final_state, Cancelled);
    }

    // ── Serde tests ──

    #[test]
    fn test_agent_state_serde() {
        let state = Running;
        let json = serde_json::to_string(&state).unwrap();
        let restored: AgentState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, Running);
    }

    #[test]
    fn test_agent_report_serde() {
        let report = AgentReport::cancelled(AgentId::new("a1"));
        let json = serde_json::to_string(&report).unwrap();
        let restored: AgentReport = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.final_state, Cancelled);
    }

    // ── All 49 combinations ──

    #[test]
    fn test_all_49_combinations() {
        let all = [Created, Initializing, Running, Reporting, Completed, Failed, Cancelled];
        let mut valid = 0;
        let mut invalid = 0;
        for from in &all {
            for to in &all {
                if from.can_transition_to(*to) {
                    valid += 1;
                } else {
                    invalid += 1;
                }
            }
        }
        // Expected: 10 valid, 39 invalid
        assert_eq!(valid, 10, "Expected 10 valid transitions");
        assert_eq!(invalid, 39, "Expected 39 invalid transitions");
    }
}
```

### contracts/src/lib.rs — add module declaration:
```rust
pub mod agent_lifecycle;
pub use agent_lifecycle::*;
```

## Validation

```bash
cd contracts && cargo test --workspace 2>&1
```

Expected: All tests pass, zero warnings.

## Done Criteria

- [ ] `contracts/src/agent_lifecycle.rs` exists
- [ ] `AgentState` enum with 7 states and enforced transitions
- [ ] `AgentSpec` struct for spawning agents
- [ ] `AgentReport` with success/failure/cancelled constructors
- [ ] `AgentLifecycle` trait (async)
- [ ] All 49 state combinations tested (10 valid, 39 invalid)
- [ ] Happy path lifecycle test
- [ ] 24+ tests pass
- [ ] All previous tests still pass

## Git

```bash
git add -A
git commit -m "P2-03: Agent Lifecycle — spawn, execute, report

- AgentState: 7 states with enforced transitions (10 valid, 39 invalid)
- AgentSpec: spawning specification with budget and depth
- AgentReport: success/failure/cancelled with efficiency metrics
- AgentLifecycle trait for pluggable agent management
- 24+ tests including full lifecycle and all 49 combinations"
git push -u origin P2-03-agent-lifecycle
gh pr create --title "P2-03: Agent Lifecycle" --body "$(cat tasks/P2-03-agent-lifecycle.md)"
```
