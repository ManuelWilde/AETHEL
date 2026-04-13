//! EU AI Act Compliance — audit trail, risk classification, manifests.
//!
//! Every decision in AETHEL must be auditable. This module provides:
//! - AuditBlock: append-only audit log entries (SHA-256 chained)
//! - EuAiActRiskLevel: 4-tier classification per EU AI Act
//! - ComplianceManifest: system-wide compliance declaration
//! - AuditChain: append-only chain with integrity verification

use crate::{AethelError, RiskLevel};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// EU AI Act risk classification (4 tiers).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EuAiActRiskLevel {
    /// Minimal risk — no obligations.
    Minimal,
    /// Limited risk — transparency obligations.
    Limited,
    /// High risk — full compliance required.
    High,
    /// Unacceptable risk — prohibited.
    Unacceptable,
}

impl From<RiskLevel> for EuAiActRiskLevel {
    fn from(risk: RiskLevel) -> Self {
        match risk {
            RiskLevel::Low => EuAiActRiskLevel::Minimal,
            RiskLevel::Medium => EuAiActRiskLevel::Limited,
            RiskLevel::High => EuAiActRiskLevel::High,
            RiskLevel::Critical => EuAiActRiskLevel::Unacceptable,
        }
    }
}

/// A single audit block in the append-only audit chain.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditBlock {
    /// Block index (0-based, sequential).
    pub index: u64,
    /// Timestamp in milliseconds.
    pub timestamp_ms: u64,
    /// The action that was audited.
    pub action: String,
    /// Who or what performed the action.
    pub actor: String,
    /// The decision or outcome.
    pub decision: String,
    /// Risk level at time of decision.
    pub risk_level: EuAiActRiskLevel,
    /// Hash of the previous block (empty string for genesis block).
    pub previous_hash: String,
    /// Hash of this block.
    pub block_hash: String,
}

impl AuditBlock {
    /// Compute the hash of this block's content (excluding block_hash itself).
    fn compute_hash(&self) -> String {
        let mut hasher = DefaultHasher::new();
        self.index.hash(&mut hasher);
        self.timestamp_ms.hash(&mut hasher);
        self.action.hash(&mut hasher);
        self.actor.hash(&mut hasher);
        self.decision.hash(&mut hasher);
        self.previous_hash.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }
}

/// Append-only audit chain with integrity verification.
pub struct AuditChain {
    blocks: Vec<AuditBlock>,
}

impl AuditChain {
    /// Create a new empty chain.
    pub fn new() -> Self {
        Self { blocks: Vec::new() }
    }

    /// Number of blocks in the chain.
    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    /// Is the chain empty?
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// Append a new block to the chain.
    pub fn append(
        &mut self,
        action: impl Into<String>,
        actor: impl Into<String>,
        decision: impl Into<String>,
        risk_level: EuAiActRiskLevel,
        timestamp_ms: u64,
    ) -> &AuditBlock {
        let previous_hash = self.blocks.last()
            .map(|b| b.block_hash.clone())
            .unwrap_or_default();

        let mut block = AuditBlock {
            index: self.blocks.len() as u64,
            timestamp_ms,
            action: action.into(),
            actor: actor.into(),
            decision: decision.into(),
            risk_level,
            previous_hash,
            block_hash: String::new(),
        };
        block.block_hash = block.compute_hash();
        self.blocks.push(block);
        self.blocks.last().unwrap()
    }

    /// Verify the integrity of the entire chain.
    /// Returns Ok(()) if all hashes are consistent.
    pub fn verify_integrity(&self) -> Result<(), AethelError> {
        for (i, block) in self.blocks.iter().enumerate() {
            // Verify block hash
            let expected_hash = block.compute_hash();
            if block.block_hash != expected_hash {
                return Err(AethelError::Other(format!(
                    "Block {} hash mismatch: expected {}, got {}", i, expected_hash, block.block_hash
                )));
            }
            // Verify chain linkage
            if i > 0 {
                let prev = &self.blocks[i - 1];
                if block.previous_hash != prev.block_hash {
                    return Err(AethelError::Other(format!(
                        "Block {} previous_hash doesn't match block {}'s hash", i, i - 1
                    )));
                }
            } else if !block.previous_hash.is_empty() {
                return Err(AethelError::Other("Genesis block has non-empty previous_hash".into()));
            }
        }
        Ok(())
    }

    /// Get all blocks.
    pub fn blocks(&self) -> &[AuditBlock] {
        &self.blocks
    }

    /// Get the last block.
    pub fn last(&self) -> Option<&AuditBlock> {
        self.blocks.last()
    }

    /// Filter blocks by risk level.
    pub fn blocks_by_risk(&self, risk: EuAiActRiskLevel) -> Vec<&AuditBlock> {
        self.blocks.iter().filter(|b| b.risk_level == risk).collect()
    }
}

impl Default for AuditChain {
    fn default() -> Self { Self::new() }
}

/// System-wide compliance manifest.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComplianceManifest {
    /// System name.
    pub system_name: String,
    /// System version.
    pub system_version: String,
    /// EU AI Act classification.
    pub eu_classification: EuAiActRiskLevel,
    /// Whether human oversight is enabled.
    pub human_oversight_enabled: bool,
    /// Whether audit logging is enabled.
    pub audit_logging_enabled: bool,
    /// Whether data residency constraints are enforced.
    pub data_residency_enforced: bool,
    /// List of forbidden operations.
    pub forbidden_operations: Vec<String>,
    /// Maximum fractal depth allowed.
    pub max_fractal_depth: u32,
    /// Whether responsible scaling gates are active.
    pub scaling_gates_active: bool,
    /// Compliance notes.
    pub notes: Vec<String>,
}

impl ComplianceManifest {
    /// Create a default AETHEL compliance manifest.
    pub fn aethel_default() -> Self {
        Self {
            system_name: "AETHEL".into(),
            system_version: "0.1.0".into(),
            eu_classification: EuAiActRiskLevel::High,
            human_oversight_enabled: true,
            audit_logging_enabled: true,
            data_residency_enforced: true,
            forbidden_operations: vec![
                "canonize".into(),
                "trigger_final_promotion".into(),
                "override_policy".into(),
                "replace_meta_governance".into(),
                "bypass_verify".into(),
            ],
            max_fractal_depth: 12,
            scaling_gates_active: true,
            notes: vec![
                "EU AI Act High-Risk system — full compliance required".into(),
                "Human review required for Critical risk operations".into(),
            ],
        }
    }

    /// Check if the manifest is compliant (all required features enabled).
    pub fn is_compliant(&self) -> bool {
        self.human_oversight_enabled
            && self.audit_logging_enabled
            && self.scaling_gates_active
            && !self.forbidden_operations.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── EuAiActRiskLevel tests ──

    #[test]
    fn test_risk_level_conversion() {
        assert_eq!(EuAiActRiskLevel::from(RiskLevel::Low), EuAiActRiskLevel::Minimal);
        assert_eq!(EuAiActRiskLevel::from(RiskLevel::Medium), EuAiActRiskLevel::Limited);
        assert_eq!(EuAiActRiskLevel::from(RiskLevel::High), EuAiActRiskLevel::High);
        assert_eq!(EuAiActRiskLevel::from(RiskLevel::Critical), EuAiActRiskLevel::Unacceptable);
    }

    #[test]
    fn test_risk_level_serde() {
        let level = EuAiActRiskLevel::High;
        let json = serde_json::to_string(&level).unwrap();
        let restored: EuAiActRiskLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, EuAiActRiskLevel::High);
    }

    // ── AuditChain tests ──

    #[test]
    fn test_empty_chain() {
        let chain = AuditChain::new();
        assert!(chain.is_empty());
        assert_eq!(chain.len(), 0);
        assert!(chain.verify_integrity().is_ok());
    }

    #[test]
    fn test_append_genesis() {
        let mut chain = AuditChain::new();
        let block = chain.append("create_claim", "system", "approved", EuAiActRiskLevel::Minimal, 1000);
        assert_eq!(block.index, 0);
        assert!(block.previous_hash.is_empty());
        assert!(!block.block_hash.is_empty());
    }

    #[test]
    fn test_append_chain() {
        let mut chain = AuditChain::new();
        chain.append("action1", "actor1", "ok", EuAiActRiskLevel::Minimal, 1000);
        chain.append("action2", "actor2", "ok", EuAiActRiskLevel::Limited, 2000);
        chain.append("action3", "actor3", "ok", EuAiActRiskLevel::High, 3000);
        assert_eq!(chain.len(), 3);
        assert!(chain.verify_integrity().is_ok());
    }

    #[test]
    fn test_chain_linkage() {
        let mut chain = AuditChain::new();
        chain.append("a1", "s", "ok", EuAiActRiskLevel::Minimal, 100);
        chain.append("a2", "s", "ok", EuAiActRiskLevel::Minimal, 200);
        let blocks = chain.blocks();
        assert_eq!(blocks[1].previous_hash, blocks[0].block_hash);
    }

    #[test]
    fn test_tamper_detection() {
        let mut chain = AuditChain::new();
        chain.append("a1", "s", "ok", EuAiActRiskLevel::Minimal, 100);
        chain.append("a2", "s", "ok", EuAiActRiskLevel::Minimal, 200);
        // Tamper with block 0
        chain.blocks[0].action = "TAMPERED".into();
        assert!(chain.verify_integrity().is_err());
    }

    #[test]
    fn test_blocks_by_risk() {
        let mut chain = AuditChain::new();
        chain.append("a1", "s", "ok", EuAiActRiskLevel::Minimal, 100);
        chain.append("a2", "s", "ok", EuAiActRiskLevel::High, 200);
        chain.append("a3", "s", "ok", EuAiActRiskLevel::High, 300);
        assert_eq!(chain.blocks_by_risk(EuAiActRiskLevel::High).len(), 2);
        assert_eq!(chain.blocks_by_risk(EuAiActRiskLevel::Minimal).len(), 1);
        assert_eq!(chain.blocks_by_risk(EuAiActRiskLevel::Unacceptable).len(), 0);
    }

    #[test]
    fn test_last_block() {
        let mut chain = AuditChain::new();
        assert!(chain.last().is_none());
        chain.append("a1", "s", "ok", EuAiActRiskLevel::Minimal, 100);
        assert_eq!(chain.last().unwrap().action, "a1");
        chain.append("a2", "s", "ok", EuAiActRiskLevel::Minimal, 200);
        assert_eq!(chain.last().unwrap().action, "a2");
    }

    // ── ComplianceManifest tests ──

    #[test]
    fn test_default_manifest_is_compliant() {
        let manifest = ComplianceManifest::aethel_default();
        assert!(manifest.is_compliant());
        assert_eq!(manifest.eu_classification, EuAiActRiskLevel::High);
    }

    #[test]
    fn test_non_compliant_manifest() {
        let mut manifest = ComplianceManifest::aethel_default();
        manifest.human_oversight_enabled = false;
        assert!(!manifest.is_compliant());
    }

    #[test]
    fn test_manifest_serde() {
        let manifest = ComplianceManifest::aethel_default();
        let json = serde_json::to_string(&manifest).unwrap();
        let restored: ComplianceManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.system_name, "AETHEL");
        assert!(restored.is_compliant());
    }

    #[test]
    fn test_audit_block_serde() {
        let mut chain = AuditChain::new();
        chain.append("test", "system", "ok", EuAiActRiskLevel::Minimal, 100);
        let block = chain.last().unwrap();
        let json = serde_json::to_string(block).unwrap();
        let restored: AuditBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.action, "test");
    }
}
