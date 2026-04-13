//! Agent runner — executes individual agents with lifecycle management.
//!
//! Each agent runs in isolation with its own budget, state machine, and
//! capability bindings. The runner manages the full lifecycle from
//! Created → Initializing → Running → Reporting → Completed/Failed.

use aethel_contracts::{
    AgentId, AgentReport, AgentSpec, AgentState, AethelError, BudgetLease,
    CapValue, CapabilityId, RiskLevel, ThoughtEfficiency,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Result of an agent execution.
#[derive(Clone, Debug)]
pub struct AgentResult {
    pub agent_id: AgentId,
    pub state: AgentState,
    pub report: Option<AgentReport>,
    pub output: Option<CapValue>,
    pub elapsed: Duration,
    pub tokens_used: u64,
    pub cost_used_cents: u64,
}

impl AgentResult {
    pub fn is_success(&self) -> bool {
        self.state == AgentState::Completed
    }
}

/// Configuration for running an agent.
#[derive(Clone, Debug)]
pub struct AgentRunConfig {
    pub agent_id: AgentId,
    pub spec: AgentSpec,
    pub budget: BudgetLease,
    pub input: CapValue,
    pub timeout: Duration,
}

impl AgentRunConfig {
    pub fn new(agent_id: AgentId, spec: AgentSpec, budget: BudgetLease, input: CapValue) -> Self {
        Self {
            agent_id,
            spec,
            budget,
            input,
            timeout: Duration::from_secs(300),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

/// Trait for executing agent work. Implementations connect to actual LLMs.
#[async_trait]
pub trait AgentBackend: Send + Sync {
    /// Execute a single agent task. Returns the output value, tokens used, and cost.
    async fn execute(
        &self,
        spec: &AgentSpec,
        input: &CapValue,
        budget: &BudgetLease,
    ) -> Result<(CapValue, u64, u64), AethelError>;
}

/// Runs agents through their lifecycle with budget tracking.
pub struct AgentRunner {
    backend: Arc<dyn AgentBackend>,
    active_agents: Arc<Mutex<HashMap<AgentId, AgentState>>>,
}

impl AgentRunner {
    pub fn new(backend: Arc<dyn AgentBackend>) -> Self {
        Self {
            backend,
            active_agents: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn run(&self, config: AgentRunConfig) -> AgentResult {
        let start = Instant::now();
        let agent_id = config.agent_id.clone();

        // Created → Initializing
        {
            let mut agents = self.active_agents.lock().await;
            agents.insert(agent_id.clone(), AgentState::Initializing);
        }
        // Initializing → Running
        {
            let mut agents = self.active_agents.lock().await;
            agents.insert(agent_id.clone(), AgentState::Running);
        }

        // Execute with timeout
        let result = tokio::time::timeout(
            config.timeout,
            self.backend.execute(&config.spec, &config.input, &config.budget),
        )
        .await;

        let elapsed = start.elapsed();
        let elapsed_ms = elapsed.as_millis() as u64;

        let (state, report, output, tokens, cost) = match result {
            Ok(Ok((output, tokens_used, cost_used))) => {
                let mut agents = self.active_agents.lock().await;
                agents.insert(agent_id.clone(), AgentState::Reporting);

                let efficiency = ThoughtEfficiency {
                    confidence_achieved: 0.8,
                    tokens_consumed: tokens_used as u32,
                    duration_ms: elapsed_ms,
                    cost_cents: cost_used as f32,
                    confidence_per_token: if tokens_used > 0 { 0.8 / tokens_used as f32 } else { 0.0 },
                    confidence_per_cent: if cost_used > 0 { 0.8 / cost_used as f32 } else { 0.0 },
                };

                let report = AgentReport::success(
                    agent_id.clone(),
                    format!("Completed in {:?}", elapsed),
                    efficiency,
                    elapsed_ms,
                    tokens_used,
                    cost_used as f32,
                );

                agents.insert(agent_id.clone(), AgentState::Completed);
                (AgentState::Completed, Some(report), Some(output), tokens_used, cost_used)
            }
            Ok(Err(e)) => {
                let mut agents = self.active_agents.lock().await;
                agents.insert(agent_id.clone(), AgentState::Reporting);

                let report = AgentReport::failure(
                    agent_id.clone(),
                    format!("Agent error: {}", e),
                    elapsed_ms,
                    0,
                    0.0,
                );

                agents.insert(agent_id.clone(), AgentState::Failed);
                (AgentState::Failed, Some(report), None, 0, 0)
            }
            Err(_) => {
                let mut agents = self.active_agents.lock().await;
                let report = AgentReport::cancelled(agent_id.clone());
                agents.insert(agent_id.clone(), AgentState::Failed);
                (AgentState::Failed, Some(report), None, 0, 0)
            }
        };

        AgentResult {
            agent_id,
            state,
            report,
            output,
            elapsed,
            tokens_used: tokens,
            cost_used_cents: cost,
        }
    }

    pub async fn get_state(&self, agent_id: &AgentId) -> Option<AgentState> {
        let agents = self.active_agents.lock().await;
        agents.get(agent_id).copied()
    }

    pub async fn active_count(&self) -> usize {
        let agents = self.active_agents.lock().await;
        agents
            .values()
            .filter(|s| !matches!(s, AgentState::Completed | AgentState::Failed | AgentState::Cancelled))
            .count()
    }
}

// ─── Test Backend ───────────────────────────────────────────

/// A simple test backend that echoes input or fails on command.
pub struct EchoBackend {
    pub should_fail: bool,
    pub tokens_per_call: u64,
    pub cost_per_call: u64,
}

impl EchoBackend {
    pub fn new() -> Self {
        Self {
            should_fail: false,
            tokens_per_call: 100,
            cost_per_call: 5,
        }
    }

    pub fn failing() -> Self {
        Self {
            should_fail: true,
            tokens_per_call: 0,
            cost_per_call: 0,
        }
    }
}

#[async_trait]
impl AgentBackend for EchoBackend {
    async fn execute(
        &self,
        _spec: &AgentSpec,
        input: &CapValue,
        _budget: &BudgetLease,
    ) -> Result<(CapValue, u64, u64), AethelError> {
        if self.should_fail {
            return Err(AethelError::Other("Simulated failure".to_string()));
        }
        Ok((input.clone(), self.tokens_per_call, self.cost_per_call))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_spec() -> AgentSpec {
        AgentSpec {
            agent_id: AgentId::new("agent-1"),
            capability_id: CapabilityId::new("echo"),
            input_prompt: "test".to_string(),
            max_tokens: 10_000,
            max_cost_cents: 500.0,
            max_duration_ms: 60_000,
            risk_level: RiskLevel::Low,
            depth: 0,
            parent_agent_id: None,
        }
    }

    fn make_budget() -> BudgetLease {
        BudgetLease {
            lease_id: "lease-1".to_string(),
            mission_id: "mission-1".to_string(),
            max_tokens: 10_000,
            max_cost_cents: 500.0,
            max_duration_ms: 60_000,
            tokens_used: 0,
            cost_used_cents: 0.0,
            granted_at_ms: 0,
            expires_at_ms: 0,
        }
    }

    #[tokio::test]
    async fn test_successful_run() {
        let backend = Arc::new(EchoBackend::new());
        let runner = AgentRunner::new(backend);
        let config = AgentRunConfig::new(
            AgentId::new("agent-1"),
            make_spec(),
            make_budget(),
            CapValue::Text("hello".to_string()),
        );
        let result = runner.run(config).await;
        assert!(result.is_success());
        assert_eq!(result.tokens_used, 100);
        assert_eq!(result.cost_used_cents, 5);
        assert!(result.output.is_some());
    }

    #[tokio::test]
    async fn test_failed_run() {
        let backend = Arc::new(EchoBackend::failing());
        let runner = AgentRunner::new(backend);
        let config = AgentRunConfig::new(
            AgentId::new("agent-1"),
            make_spec(),
            make_budget(),
            CapValue::Text("hello".to_string()),
        );
        let result = runner.run(config).await;
        assert!(!result.is_success());
        assert_eq!(result.state, AgentState::Failed);
    }

    #[tokio::test]
    async fn test_timeout() {
        struct SlowBackend;
        #[async_trait]
        impl AgentBackend for SlowBackend {
            async fn execute(&self, _: &AgentSpec, _: &CapValue, _: &BudgetLease) -> Result<(CapValue, u64, u64), AethelError> {
                tokio::time::sleep(Duration::from_secs(999)).await;
                Ok((CapValue::Nothing, 0, 0))
            }
        }
        let runner = AgentRunner::new(Arc::new(SlowBackend));
        let config = AgentRunConfig::new(
            AgentId::new("agent-1"), make_spec(), make_budget(), CapValue::Text("hello".into()),
        ).with_timeout(Duration::from_millis(50));
        let result = runner.run(config).await;
        assert_eq!(result.state, AgentState::Failed);
    }

    #[tokio::test]
    async fn test_active_count() {
        let runner = AgentRunner::new(Arc::new(EchoBackend::new()));
        assert_eq!(runner.active_count().await, 0);
        let config = AgentRunConfig::new(
            AgentId::new("agent-1"), make_spec(), make_budget(), CapValue::Text("hello".into()),
        );
        runner.run(config).await;
        assert_eq!(runner.active_count().await, 0);
    }
}
