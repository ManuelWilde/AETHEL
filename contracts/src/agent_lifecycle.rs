//! Agent Lifecycle — spawn, execute, report.
//!
//! Every FIMAS agent follows this lifecycle:
//! Created → Initializing → Running → Reporting → Completed
//! With failure/cancellation paths at each active state.

use crate::{
    AethelError, AgentId, CapabilityId, ClaimState, RiskLevel,
    ThoughtEfficiency,
};
use serde::{Deserialize, Serialize};

/// Lifecycle state of a FIMAS agent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentState {
    /// Defined but not started.
    Created,
    /// Loading model and context.
    Initializing,
    /// Executing sub-task.
    Running,
    /// Generating execution report.
    Reporting,
    /// Completed successfully.
    Completed,
    /// Failed with error.
    Failed,
    /// Cancelled by control plane.
    Cancelled,
}

impl AgentState {
    /// Valid transitions from this state.
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
                from: ClaimState::Generated,
                to: ClaimState::Generated,
            })
        }
    }

    /// Is this a terminal state?
    pub fn is_terminal(&self) -> bool {
        self.allowed_transitions().is_empty()
    }

    /// Is this an active state?
    pub fn is_active(&self) -> bool {
        !self.is_terminal()
    }
}

/// Specification for spawning a new agent.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentSpec {
    /// Unique agent ID.
    pub agent_id: AgentId,
    /// The capability to execute.
    pub capability_id: CapabilityId,
    /// Input prompt/data.
    pub input_prompt: String,
    /// Max tokens.
    pub max_tokens: u64,
    /// Max cost in cents.
    pub max_cost_cents: f32,
    /// Max wall-clock time in ms.
    pub max_duration_ms: u64,
    /// Risk level.
    pub risk_level: RiskLevel,
    /// Fractal depth.
    pub depth: u32,
    /// Parent agent (None if root).
    pub parent_agent_id: Option<AgentId>,
}

/// Report from a completed agent execution.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentReport {
    /// Agent ID.
    pub agent_id: AgentId,
    /// Final state.
    pub final_state: AgentState,
    /// Output (if successful).
    pub output: Option<String>,
    /// Error (if failed).
    pub error: Option<String>,
    /// Efficiency metrics.
    pub efficiency: Option<ThoughtEfficiency>,
    /// Duration in ms.
    pub duration_ms: u64,
    /// Tokens consumed.
    pub tokens_consumed: u64,
    /// Cost consumed in cents.
    pub cost_consumed_cents: f32,
}

impl AgentReport {
    /// Create a success report.
    pub fn success(agent_id: AgentId, output: String, efficiency: ThoughtEfficiency, duration_ms: u64, tokens: u64, cost: f32) -> Self {
        Self { agent_id, final_state: AgentState::Completed, output: Some(output), error: None, efficiency: Some(efficiency), duration_ms, tokens_consumed: tokens, cost_consumed_cents: cost }
    }

    /// Create a failure report.
    pub fn failure(agent_id: AgentId, error: String, duration_ms: u64, tokens: u64, cost: f32) -> Self {
        Self { agent_id, final_state: AgentState::Failed, output: None, error: Some(error), efficiency: None, duration_ms, tokens_consumed: tokens, cost_consumed_cents: cost }
    }

    /// Create a cancellation report.
    pub fn cancelled(agent_id: AgentId) -> Self {
        Self { agent_id, final_state: AgentState::Cancelled, output: None, error: None, efficiency: None, duration_ms: 0, tokens_consumed: 0, cost_consumed_cents: 0.0 }
    }

    /// Is this a successful completion?
    pub fn is_success(&self) -> bool {
        self.final_state == AgentState::Completed
    }
}

/// The agent lifecycle trait.
#[async_trait::async_trait]
pub trait AgentLifecycle: Send + Sync {
    /// Spawn a new agent.
    async fn spawn(&self, spec: AgentSpec) -> Result<AgentId, AethelError>;
    /// Get current state.
    async fn state(&self, agent_id: &AgentId) -> Result<AgentState, AethelError>;
    /// Cancel a running agent.
    async fn cancel(&self, agent_id: &AgentId) -> Result<(), AethelError>;
    /// Wait for completion and return report.
    async fn wait_for_report(&self, agent_id: &AgentId) -> Result<AgentReport, AethelError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use AgentState::*;

    #[test]
    fn test_created_to_initializing() { assert!(Created.can_transition_to(Initializing)); }
    #[test]
    fn test_created_to_cancelled() { assert!(Created.can_transition_to(Cancelled)); }
    #[test]
    fn test_created_to_running_invalid() { assert!(!Created.can_transition_to(Running)); }
    #[test]
    fn test_initializing_to_running() { assert!(Initializing.can_transition_to(Running)); }
    #[test]
    fn test_initializing_to_failed() { assert!(Initializing.can_transition_to(Failed)); }
    #[test]
    fn test_running_to_reporting() { assert!(Running.can_transition_to(Reporting)); }
    #[test]
    fn test_running_to_cancelled() { assert!(Running.can_transition_to(Cancelled)); }
    #[test]
    fn test_reporting_to_completed() { assert!(Reporting.can_transition_to(Completed)); }
    #[test]
    fn test_reporting_to_failed() { assert!(Reporting.can_transition_to(Failed)); }
    #[test]
    fn test_completed_is_terminal() { assert!(Completed.is_terminal()); assert!(!Completed.is_active()); }
    #[test]
    fn test_failed_is_terminal() { assert!(Failed.is_terminal()); }
    #[test]
    fn test_cancelled_is_terminal() { assert!(Cancelled.is_terminal()); }
    #[test]
    fn test_created_is_active() { assert!(Created.is_active()); }

    #[test]
    fn test_transition_success() {
        assert_eq!(Created.transition(Initializing).unwrap(), Initializing);
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

    #[test]
    fn test_happy_path() {
        let s = Created;
        let s = s.transition(Initializing).unwrap();
        let s = s.transition(Running).unwrap();
        let s = s.transition(Reporting).unwrap();
        let s = s.transition(Completed).unwrap();
        assert!(s.is_terminal());
    }

    #[test]
    fn test_success_report() {
        let eff = ThoughtEfficiency::compute(0.9, 100, 500, 1.0, 3, 5);
        let report = AgentReport::success(AgentId::new("a1"), "result".into(), eff, 500, 100, 1.0);
        assert!(report.is_success());
        assert_eq!(report.output, Some("result".into()));
    }

    #[test]
    fn test_failure_report() {
        let report = AgentReport::failure(AgentId::new("a2"), "timeout".into(), 1000, 50, 0.5);
        assert!(!report.is_success());
        assert_eq!(report.error, Some("timeout".into()));
    }

    #[test]
    fn test_cancelled_report() {
        let report = AgentReport::cancelled(AgentId::new("a3"));
        assert!(!report.is_success());
        assert_eq!(report.final_state, Cancelled);
    }

    #[test]
    fn test_agent_state_serde() {
        let state = Running;
        let json = serde_json::to_string(&state).unwrap();
        let restored: AgentState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, Running);
    }

    #[test]
    fn test_all_49_combinations() {
        let all = [Created, Initializing, Running, Reporting, Completed, Failed, Cancelled];
        let mut valid = 0;
        let mut invalid = 0;
        for from in &all {
            for to in &all {
                if from.can_transition_to(*to) { valid += 1; } else { invalid += 1; }
            }
        }
        assert_eq!(valid, 10);
        assert_eq!(invalid, 39);
    }
}
