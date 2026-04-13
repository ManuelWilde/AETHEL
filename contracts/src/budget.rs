//! Budget enforcement for FIMAS agents.
//!
//! Every agent gets a BudgetLease from the control plane.
//! This module adds methods to enforce budget limits:
//! - `consume()`: attempt to use tokens/cost, fail if over limit
//! - `remaining_tokens()`: how many tokens are left
//! - `remaining_cost()`: how much cost is left
//! - `is_exhausted()`: is the budget used up
//! - `utilization()`: what fraction of budget is used (0.0 to 1.0)

use crate::{BudgetLease, AethelError};

impl BudgetLease {
    /// Attempt to consume tokens and cost. Returns Ok(()) if within budget,
    /// Err(BudgetExceeded) if this would exceed limits.
    ///
    /// IMPORTANT: On error, the budget is NOT modified (transactional).
    pub fn consume(&mut self, tokens: u64, cost_cents: f32) -> Result<(), AethelError> {
        if self.tokens_used + tokens > self.max_tokens {
            return Err(AethelError::BudgetExceeded(format!(
                "tokens: {} + {} > max {}",
                self.tokens_used, tokens, self.max_tokens
            )));
        }
        if self.cost_used_cents + cost_cents > self.max_cost_cents {
            return Err(AethelError::BudgetExceeded(format!(
                "cost: {:.2} + {:.2} > max {:.2}",
                self.cost_used_cents, cost_cents, self.max_cost_cents
            )));
        }
        self.tokens_used += tokens;
        self.cost_used_cents += cost_cents;
        Ok(())
    }

    /// Remaining token budget.
    pub fn remaining_tokens(&self) -> u64 {
        self.max_tokens.saturating_sub(self.tokens_used)
    }

    /// Remaining cost budget in cents.
    pub fn remaining_cost(&self) -> f32 {
        (self.max_cost_cents - self.cost_used_cents).max(0.0)
    }

    /// True if either token or cost budget is fully used.
    pub fn is_exhausted(&self) -> bool {
        self.tokens_used >= self.max_tokens || self.cost_used_cents >= self.max_cost_cents
    }

    /// Budget utilization as a fraction (0.0 = unused, 1.0 = fully used).
    /// Takes the maximum of token utilization and cost utilization.
    pub fn utilization(&self) -> f32 {
        let token_util = if self.max_tokens > 0 {
            self.tokens_used as f32 / self.max_tokens as f32
        } else {
            0.0
        };
        let cost_util = if self.max_cost_cents > 0.0 {
            self.cost_used_cents / self.max_cost_cents
        } else {
            0.0
        };
        token_util.max(cost_util).clamp(0.0, 1.0)
    }

    /// Create a sub-lease: a new BudgetLease carved from this one.
    /// The parent's budget is reduced by the sub-lease amount.
    pub fn sub_lease(
        &mut self,
        lease_id: String,
        max_tokens: u64,
        max_cost_cents: f32,
    ) -> Result<BudgetLease, AethelError> {
        if max_tokens > self.remaining_tokens() {
            return Err(AethelError::BudgetExceeded(format!(
                "sub-lease tokens {} > remaining {}",
                max_tokens,
                self.remaining_tokens()
            )));
        }
        if max_cost_cents > self.remaining_cost() {
            return Err(AethelError::BudgetExceeded(format!(
                "sub-lease cost {:.2} > remaining {:.2}",
                max_cost_cents,
                self.remaining_cost()
            )));
        }
        // Reserve the budget in the parent
        self.tokens_used += max_tokens;
        self.cost_used_cents += max_cost_cents;

        Ok(BudgetLease {
            lease_id,
            mission_id: self.mission_id.clone(),
            max_tokens,
            max_cost_cents,
            max_duration_ms: self.max_duration_ms,
            tokens_used: 0,
            cost_used_cents: 0.0,
            granted_at_ms: self.granted_at_ms,
            expires_at_ms: self.expires_at_ms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_lease(max_tokens: u64, max_cost: f32) -> BudgetLease {
        BudgetLease {
            lease_id: "test-lease".into(),
            mission_id: "test-mission".into(),
            max_tokens,
            max_cost_cents: max_cost,
            max_duration_ms: 60000,
            tokens_used: 0,
            cost_used_cents: 0.0,
            granted_at_ms: 0,
            expires_at_ms: 60000,
        }
    }

    #[test]
    fn test_consume_within_budget() {
        let mut lease = make_lease(1000, 10.0);
        assert!(lease.consume(500, 5.0).is_ok());
        assert_eq!(lease.tokens_used, 500);
        assert!((lease.cost_used_cents - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_consume_exact_limit() {
        let mut lease = make_lease(1000, 10.0);
        assert!(lease.consume(1000, 10.0).is_ok());
        assert!(lease.is_exhausted());
    }

    #[test]
    fn test_consume_exceeds_tokens() {
        let mut lease = make_lease(1000, 10.0);
        let result = lease.consume(1001, 5.0);
        assert!(result.is_err());
        // Budget must NOT be modified on error
        assert_eq!(lease.tokens_used, 0);
    }

    #[test]
    fn test_consume_exceeds_cost() {
        let mut lease = make_lease(1000, 10.0);
        let result = lease.consume(500, 10.01);
        assert!(result.is_err());
        assert!((lease.cost_used_cents - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_consume_multiple_times() {
        let mut lease = make_lease(1000, 10.0);
        assert!(lease.consume(300, 3.0).is_ok());
        assert!(lease.consume(300, 3.0).is_ok());
        assert!(lease.consume(300, 3.0).is_ok());
        // 900 used, 100 remaining
        assert!(lease.consume(101, 0.0).is_err()); // 101 > 100 remaining
        assert!(lease.consume(100, 0.0).is_ok());   // exactly at limit
    }

    #[test]
    fn test_remaining_tokens() {
        let mut lease = make_lease(1000, 10.0);
        assert_eq!(lease.remaining_tokens(), 1000);
        lease.consume(600, 0.0).unwrap();
        assert_eq!(lease.remaining_tokens(), 400);
    }

    #[test]
    fn test_remaining_cost() {
        let mut lease = make_lease(1000, 10.0);
        lease.consume(0, 7.5).unwrap();
        assert!((lease.remaining_cost() - 2.5).abs() < 0.01);
    }

    #[test]
    fn test_remaining_never_negative() {
        let mut lease = make_lease(100, 1.0);
        lease.tokens_used = 200; // simulate overflow (shouldn't happen but defensive)
        assert_eq!(lease.remaining_tokens(), 0); // saturating_sub prevents underflow
    }

    #[test]
    fn test_is_exhausted_tokens() {
        let mut lease = make_lease(100, 100.0);
        lease.consume(100, 0.0).unwrap();
        assert!(lease.is_exhausted());
    }

    #[test]
    fn test_is_exhausted_cost() {
        let mut lease = make_lease(10000, 1.0);
        lease.consume(0, 1.0).unwrap();
        assert!(lease.is_exhausted());
    }

    #[test]
    fn test_not_exhausted() {
        let lease = make_lease(1000, 10.0);
        assert!(!lease.is_exhausted());
    }

    #[test]
    fn test_utilization_empty() {
        let lease = make_lease(1000, 10.0);
        assert!((lease.utilization() - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_utilization_half() {
        let mut lease = make_lease(1000, 10.0);
        lease.consume(500, 5.0).unwrap();
        assert!((lease.utilization() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_utilization_full() {
        let mut lease = make_lease(1000, 10.0);
        lease.consume(1000, 10.0).unwrap();
        assert!((lease.utilization() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_sub_lease_success() {
        let mut parent = make_lease(1000, 10.0);
        let child = parent.sub_lease("child-1".into(), 300, 3.0).unwrap();
        assert_eq!(child.max_tokens, 300);
        assert_eq!(child.tokens_used, 0);
        assert_eq!(parent.remaining_tokens(), 700);
    }

    #[test]
    fn test_sub_lease_exceeds_remaining() {
        let mut parent = make_lease(1000, 10.0);
        parent.consume(800, 0.0).unwrap();
        let result = parent.sub_lease("child".into(), 300, 0.0); // 300 > 200 remaining
        assert!(result.is_err());
    }

    #[test]
    fn test_multiple_sub_leases() {
        let mut parent = make_lease(1000, 10.0);
        let _c1 = parent.sub_lease("c1".into(), 300, 3.0).unwrap();
        let _c2 = parent.sub_lease("c2".into(), 300, 3.0).unwrap();
        let _c3 = parent.sub_lease("c3".into(), 300, 3.0).unwrap();
        // 900 reserved, 100 remaining
        assert!(parent.sub_lease("c4".into(), 200, 0.0).is_err());
        assert!(parent.sub_lease("c4".into(), 100, 1.0).is_ok());
    }
}
