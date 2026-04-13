//! Triplex Via Router — the three-phase routing algorithm.
//!
//! Phase 1: Purificatio — Policy filter (what is ALLOWED?)
//! Phase 2: Illuminatio — Bio-gating + ontological fit (what is APPROPRIATE?)
//! Phase 3: Unio — Selection (what is OPTIMAL?)

use crate::{
    AethelError, BioSignal, BioGateState, OmegaSpectrum24,
    ProviderProfile, ProviderKind, RoutingResidency, RiskLevel,
};
use serde::{Deserialize, Serialize};

/// Policy constraint for Phase 1 filtering.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PolicyConstraint {
    /// Constraint name.
    pub name: String,
    /// Allowed residency modes.
    pub allowed_residency: Vec<RoutingResidency>,
    /// Blocked providers.
    pub blocked_providers: Vec<ProviderKind>,
    /// Minimum risk level that triggers this constraint.
    pub applies_at_risk: RiskLevel,
}

/// Result of Phase 1: Purificatio.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PurificatioResult {
    /// Providers that survived the policy filter.
    pub survivors: Vec<ProviderProfile>,
    /// Providers eliminated and why.
    pub eliminated: Vec<(ProviderProfile, String)>,
}

/// Result of Phase 2: Illuminatio.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IlluminatioResult {
    /// Providers ranked by ontological fit + bio-gating.
    pub ranked: Vec<(ProviderProfile, f32)>,
    /// Bio-gate state used for ranking.
    pub bio_gate_state: BioGateState,
}

/// Result of Phase 3: Unio.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnioResult {
    /// The selected provider.
    pub selected: ProviderProfile,
    /// Why this was chosen.
    pub reason: String,
    /// Fallback chain.
    pub fallbacks: Vec<ProviderProfile>,
}

/// Full Triplex Via routing result.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TriplexViaResult {
    /// Phase 1 result.
    pub purificatio: PurificatioResult,
    /// Phase 2 result.
    pub illuminatio: IlluminatioResult,
    /// Phase 3 result.
    pub unio: UnioResult,
    /// Total routing time in microseconds.
    pub routing_duration_us: u64,
}

/// Schmitt-Trigger bio-gating implementation.
pub struct BioGate {
    /// Current gate state.
    state: BioGateState,
    /// High threshold to activate (default 0.70).
    pub activate_threshold: f32,
    /// Low threshold to deactivate (default 0.55).
    pub deactivate_threshold: f32,
}

impl BioGate {
    /// Create a new bio-gate with default thresholds.
    pub fn new() -> Self {
        Self {
            state: BioGateState::Reduced,
            activate_threshold: 0.70,
            deactivate_threshold: 0.55,
        }
    }

    /// Get current state.
    pub fn state(&self) -> BioGateState {
        self.state
    }

    /// Update gate based on bio signal.
    /// Schmitt-Trigger: hysteresis prevents oscillation.
    pub fn update(&mut self, signal: &BioSignal) -> BioGateState {
        let coherence = signal.hrv_coherence;
        match self.state {
            BioGateState::Reduced | BioGateState::Holding => {
                if coherence >= self.activate_threshold {
                    self.state = BioGateState::Active;
                } else if coherence <= self.deactivate_threshold {
                    self.state = BioGateState::Reduced;
                } else {
                    self.state = BioGateState::Holding;
                }
            }
            BioGateState::Active => {
                if coherence <= self.deactivate_threshold {
                    self.state = BioGateState::Reduced;
                } else if coherence < self.activate_threshold {
                    self.state = BioGateState::Holding;
                }
                // stays Active if still above activate
            }
        }
        self.state
    }
}

impl Default for BioGate {
    fn default() -> Self { Self::new() }
}

/// Ontological fit score between a task spectrum and a provider.
pub fn ontological_fit(task_spectrum: &OmegaSpectrum24, provider: &ProviderProfile) -> f32 {
    // Simple heuristic: local providers get higher fit for bio-sensitive tasks
    let bio_sensitivity = task_spectrum.bio_sensitivity();
    let base_fit = if provider.is_local { 0.7 } else { 0.5 };
    let bio_bonus = if provider.is_local && bio_sensitivity > 0.5 { 0.2 } else { 0.0 };
    let cost_penalty = (provider.cost_per_1k_tokens * 0.1).min(0.3);
    (base_fit + bio_bonus - cost_penalty).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_signal(hrv: f32) -> BioSignal {
        BioSignal {
            stress: 0.3,
            focus: 0.7,
            hrv_coherence: hrv,
            measured_at_ms: 0,
        }
    }

    // ── BioGate tests ──

    #[test]
    fn test_initial_state_reduced() {
        let gate = BioGate::new();
        assert_eq!(gate.state(), BioGateState::Reduced);
    }

    #[test]
    fn test_activate_above_threshold() {
        let mut gate = BioGate::new();
        gate.update(&make_signal(0.75));
        assert_eq!(gate.state(), BioGateState::Active);
    }

    #[test]
    fn test_stay_reduced_below_deactivate() {
        let mut gate = BioGate::new();
        gate.update(&make_signal(0.50));
        assert_eq!(gate.state(), BioGateState::Reduced);
    }

    #[test]
    fn test_holding_zone() {
        let mut gate = BioGate::new();
        gate.update(&make_signal(0.60)); // between 0.55 and 0.70
        assert_eq!(gate.state(), BioGateState::Holding);
    }

    #[test]
    fn test_hysteresis_prevents_oscillation() {
        let mut gate = BioGate::new();
        // Activate
        gate.update(&make_signal(0.75));
        assert_eq!(gate.state(), BioGateState::Active);
        // Drop to holding zone — should NOT deactivate immediately
        gate.update(&make_signal(0.60));
        assert_eq!(gate.state(), BioGateState::Holding);
        // Rise back — should activate again
        gate.update(&make_signal(0.72));
        assert_eq!(gate.state(), BioGateState::Active);
        // Drop below deactivate — NOW deactivate
        gate.update(&make_signal(0.50));
        assert_eq!(gate.state(), BioGateState::Reduced);
    }

    #[test]
    fn test_active_stays_active() {
        let mut gate = BioGate::new();
        gate.update(&make_signal(0.80));
        gate.update(&make_signal(0.75));
        assert_eq!(gate.state(), BioGateState::Active);
    }

    // ── Ontological fit tests ──

    #[test]
    fn test_local_provider_higher_fit() {
        let spectrum = OmegaSpectrum24::zero();
        let local = ProviderProfile {
            provider: ProviderKind::Mlx,
            model: "local-7b".into(),
            max_context_tokens: 4096,
            supports_json_schema: false,
            supports_tools: false,
            supports_streaming: true,
            cost_per_1k_tokens: 0.0,
            is_local: true,
        };
        let remote = ProviderProfile {
            provider: ProviderKind::OpenAi,
            model: "gpt-4".into(),
            max_context_tokens: 128000,
            supports_json_schema: true,
            supports_tools: true,
            supports_streaming: true,
            cost_per_1k_tokens: 3.0,
            is_local: false,
        };
        let local_fit = ontological_fit(&spectrum, &local);
        let remote_fit = ontological_fit(&spectrum, &remote);
        assert!(local_fit > remote_fit);
    }

    #[test]
    fn test_bio_sensitive_prefers_local() {
        let mut spectrum = OmegaSpectrum24::zero();
        spectrum.values[22] = 0.8; // bio_sensitivity slot
        let local = ProviderProfile {
            provider: ProviderKind::Mlx,
            model: "local".into(),
            max_context_tokens: 4096,
            supports_json_schema: false,
            supports_tools: false,
            supports_streaming: true,
            cost_per_1k_tokens: 0.0,
            is_local: true,
        };
        let fit = ontological_fit(&spectrum, &local);
        assert!(fit > 0.8); // should get bio bonus
    }

    // ── Serde tests ──

    #[test]
    fn test_policy_constraint_serde() {
        let pc = PolicyConstraint {
            name: "GDPR Local Only".into(),
            allowed_residency: vec![RoutingResidency::LocalOnly],
            blocked_providers: vec![],
            applies_at_risk: RiskLevel::High,
        };
        let json = serde_json::to_string(&pc).unwrap();
        let restored: PolicyConstraint = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.name, "GDPR Local Only");
    }

    #[test]
    fn test_bio_gate_state_serde() {
        let state = BioGateState::Active;
        let json = serde_json::to_string(&state).unwrap();
        let restored: BioGateState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, BioGateState::Active);
    }
}
