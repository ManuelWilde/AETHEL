//! State machine enforcement for ClaimState.
//!
//! The claim lifecycle is a directed graph. Not every transition is valid.
//! This module defines which transitions are allowed and provides a safe
//! transition function that returns `Err` for invalid transitions.
//!
//! ## Valid Transitions
//! ```text
//! Generated → Supported
//! Supported → Accepted | Rejected | Deferred | Escalated
//! Accepted  → Revised | Retired
//! Deferred  → Supported | Retired
//! Escalated → Accepted | Rejected
//! Revised   → Supported
//! Rejected  → Retired
//! Retired   → (nothing — terminal state)
//! ```

use crate::{ClaimState, AethelError};

impl ClaimState {
    /// Returns the list of states this state can transition TO.
    pub fn allowed_transitions(&self) -> &'static [ClaimState] {
        use ClaimState::*;
        match self {
            Generated => &[Supported],
            Supported => &[Accepted, Rejected, Deferred, Escalated],
            Accepted  => &[Revised, Retired],
            Deferred  => &[Supported, Retired],
            Escalated => &[Accepted, Rejected],
            Revised   => &[Supported],
            Rejected  => &[Retired],
            Retired   => &[],
        }
    }

    /// Check if transitioning from this state to `target` is allowed.
    pub fn can_transition_to(&self, target: ClaimState) -> bool {
        self.allowed_transitions().contains(&target)
    }

    /// Attempt to transition to `target`. Returns the new state on success,
    /// or `AethelError::InvalidTransition` on failure.
    ///
    /// # Example
    /// ```
    /// use aethel_contracts::{ClaimState, AethelError};
    /// let state = ClaimState::Generated;
    /// assert!(state.transition(ClaimState::Supported).is_ok());
    /// assert!(state.transition(ClaimState::Accepted).is_err());
    /// ```
    pub fn transition(self, target: ClaimState) -> Result<ClaimState, AethelError> {
        if self.can_transition_to(target) {
            Ok(target)
        } else {
            Err(AethelError::InvalidTransition {
                from: self,
                to: target,
            })
        }
    }

    /// Returns true if this is a terminal state (no further transitions possible).
    pub fn is_terminal(&self) -> bool {
        self.allowed_transitions().is_empty()
    }

    /// Returns true if this is the initial state.
    pub fn is_initial(&self) -> bool {
        matches!(self, ClaimState::Generated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ClaimState::*;

    // ── Valid transitions ──

    #[test]
    fn test_generated_to_supported() {
        assert!(Generated.transition(Supported).is_ok());
    }

    #[test]
    fn test_supported_to_accepted() {
        assert!(Supported.transition(Accepted).is_ok());
    }

    #[test]
    fn test_supported_to_rejected() {
        assert!(Supported.transition(Rejected).is_ok());
    }

    #[test]
    fn test_supported_to_deferred() {
        assert!(Supported.transition(Deferred).is_ok());
    }

    #[test]
    fn test_supported_to_escalated() {
        assert!(Supported.transition(Escalated).is_ok());
    }

    #[test]
    fn test_accepted_to_revised() {
        assert!(Accepted.transition(Revised).is_ok());
    }

    #[test]
    fn test_accepted_to_retired() {
        assert!(Accepted.transition(Retired).is_ok());
    }

    #[test]
    fn test_deferred_to_supported() {
        assert!(Deferred.transition(Supported).is_ok());
    }

    #[test]
    fn test_deferred_to_retired() {
        assert!(Deferred.transition(Retired).is_ok());
    }

    #[test]
    fn test_escalated_to_accepted() {
        assert!(Escalated.transition(Accepted).is_ok());
    }

    #[test]
    fn test_escalated_to_rejected() {
        assert!(Escalated.transition(Rejected).is_ok());
    }

    #[test]
    fn test_revised_to_supported() {
        assert!(Revised.transition(Supported).is_ok());
    }

    #[test]
    fn test_rejected_to_retired() {
        assert!(Rejected.transition(Retired).is_ok());
    }

    // ── Invalid transitions ──

    #[test]
    fn test_generated_to_accepted_invalid() {
        assert!(Generated.transition(Accepted).is_err());
    }

    #[test]
    fn test_generated_to_retired_invalid() {
        assert!(Generated.transition(Retired).is_err());
    }

    #[test]
    fn test_retired_to_anything_invalid() {
        for target in &[Generated, Supported, Accepted, Deferred, Escalated, Revised, Rejected] {
            assert!(Retired.transition(*target).is_err(),
                "Retired should not transition to {:?}", target);
        }
    }

    #[test]
    fn test_self_transition_invalid() {
        for state in &[Generated, Supported, Accepted, Deferred, Escalated, Revised, Rejected, Retired] {
            assert!(state.transition(*state).is_err(),
                "{:?} should not transition to itself", state);
        }
    }

    // ── Exhaustive: all 64 combinations ──

    #[test]
    fn test_all_64_combinations() {
        let all = [Generated, Supported, Accepted, Deferred, Escalated, Revised, Rejected, Retired];
        let mut valid_count = 0;
        let mut invalid_count = 0;

        for from in &all {
            for to in &all {
                if from.can_transition_to(*to) {
                    valid_count += 1;
                } else {
                    invalid_count += 1;
                }
            }
        }

        // Expected: 13 valid transitions, 51 invalid (64 - 13)
        assert_eq!(valid_count, 13, "Expected 13 valid transitions");
        assert_eq!(invalid_count, 51, "Expected 51 invalid transitions");
    }

    // ── Helper methods ──

    #[test]
    fn test_retired_is_terminal() {
        assert!(Retired.is_terminal());
    }

    #[test]
    fn test_generated_is_initial() {
        assert!(Generated.is_initial());
    }

    #[test]
    fn test_non_terminal_states() {
        for state in &[Generated, Supported, Accepted, Deferred, Escalated, Revised, Rejected] {
            assert!(!state.is_terminal(), "{:?} should not be terminal", state);
        }
    }

    // ── Full lifecycle ──

    #[test]
    fn test_happy_path_lifecycle() {
        let s = Generated;
        let s = s.transition(Supported).unwrap();
        let s = s.transition(Accepted).unwrap();
        let s = s.transition(Retired).unwrap();
        assert!(s.is_terminal());
    }

    #[test]
    fn test_revision_lifecycle() {
        let s = Generated;
        let s = s.transition(Supported).unwrap();
        let s = s.transition(Accepted).unwrap();
        let s = s.transition(Revised).unwrap();
        let s = s.transition(Supported).unwrap();
        let s = s.transition(Accepted).unwrap();
        assert_eq!(s, Accepted);
    }

    #[test]
    fn test_escalation_lifecycle() {
        let s = Generated;
        let s = s.transition(Supported).unwrap();
        let s = s.transition(Escalated).unwrap();
        let s = s.transition(Accepted).unwrap();
        assert_eq!(s, Accepted);
    }
}
