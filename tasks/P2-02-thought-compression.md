# P2-02: Thought Compression — Budget-Adaptive Prompt Optimization

## Prerequisites

P1-04 must be merged to main. (Can run parallel to P2-01.)

## Context

You are working on the AETHEL project — a Rust workspace.
ThoughtPressure and ThoughtEfficiency are defined in contracts/src/lib.rs.
Now we build Thought Compression — the system that adapts prompt size and verification depth based on budget pressure.

Key insight: Under high pressure, the system compresses prompts, reduces verification layers, and uses cheaper models. Under low pressure, it uses full prompts and maximum verification. There's a "phase transition" where compressed thinking becomes MORE efficient.

## Git Branch

```bash
git checkout main && git pull
git checkout -b P2-02-thought-compression
```

## Your Task

1. Create `contracts/src/compression.rs` with:
   - `CompressionLevel` enum (4 levels)
   - `CompressionConfig` struct (thresholds and parameters)
   - `CompressionResult` struct (what was compressed and how)
   - `ThoughtCompressor` struct with `compress()` method
2. Add `pub mod compression; pub use compression::*;` to `contracts/src/lib.rs`

## Exact Code

### contracts/src/compression.rs:
```rust
//! Thought Compression — budget-adaptive prompt optimization.
//!
//! When budget pressure increases, the system compresses thinking:
//! - Full: complete prompts, all verification layers, best models
//! - Moderate: shortened prompts, skip low-value verification, mid-tier models
//! - Aggressive: minimal prompts, essential verification only, cheap models
//! - Emergency: single-shot prompts, no verification, fastest/cheapest model
//!
//! # Phase Transition
//! Empirically observed: at some pressure level, compressed prompts
//! produce BETTER results because they force the model to focus.
//! This is analogous to Meta's finding that smaller, well-trained models
//! can outperform larger ones on specific tasks.

use crate::{RiskLevel, ThoughtPressure};
use serde::{Deserialize, Serialize};

/// Compression levels — how aggressively to compress thinking.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CompressionLevel {
    /// No compression. Full prompts, all verification, best models.
    /// Used when budget is plentiful and risk is high.
    Full,
    /// Moderate compression. Shorter prompts, skip low-value verification.
    /// Used for medium-budget, medium-risk tasks.
    Moderate,
    /// Aggressive compression. Minimal prompts, essential verification only.
    /// Used when budget is tight but task is still important.
    Aggressive,
    /// Emergency compression. Single-shot, no verification.
    /// Used only for low-risk tasks when budget is nearly exhausted.
    Emergency,
}

/// Configuration for the compression system.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompressionConfig {
    /// Pressure threshold for Moderate (default 0.3).
    pub moderate_threshold: f32,
    /// Pressure threshold for Aggressive (default 0.6).
    pub aggressive_threshold: f32,
    /// Pressure threshold for Emergency (default 0.85).
    pub emergency_threshold: f32,
    /// Maximum prompt tokens at Full compression.
    pub full_max_tokens: u32,
    /// Maximum prompt tokens at Moderate compression.
    pub moderate_max_tokens: u32,
    /// Maximum prompt tokens at Aggressive compression.
    pub aggressive_max_tokens: u32,
    /// Maximum prompt tokens at Emergency compression.
    pub emergency_max_tokens: u32,
    /// Number of verification layers at Full compression.
    pub full_verification_layers: u8,
    /// Number of verification layers at Moderate compression.
    pub moderate_verification_layers: u8,
    /// Number of verification layers at Aggressive compression.
    pub aggressive_verification_layers: u8,
    /// Number of verification layers at Emergency compression.
    pub emergency_verification_layers: u8,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            moderate_threshold: 0.3,
            aggressive_threshold: 0.6,
            emergency_threshold: 0.85,
            full_max_tokens: 4096,
            moderate_max_tokens: 2048,
            aggressive_max_tokens: 1024,
            emergency_max_tokens: 256,
            full_verification_layers: 5,
            moderate_verification_layers: 3,
            aggressive_verification_layers: 1,
            emergency_verification_layers: 0,
        }
    }
}

/// Result of applying compression to a thought.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompressionResult {
    /// The compression level applied.
    pub level: CompressionLevel,
    /// Maximum tokens allowed for the prompt.
    pub max_prompt_tokens: u32,
    /// Number of verification layers to use.
    pub verification_layers: u8,
    /// The input pressure that triggered this compression.
    pub input_pressure: f32,
    /// Whether emergency was blocked due to high risk.
    pub emergency_blocked: bool,
}

/// The thought compressor — determines compression level from pressure and risk.
pub struct ThoughtCompressor {
    config: CompressionConfig,
}

impl ThoughtCompressor {
    /// Create a new compressor with default config.
    pub fn new() -> Self {
        Self {
            config: CompressionConfig::default(),
        }
    }

    /// Create a compressor with custom config.
    pub fn with_config(config: CompressionConfig) -> Self {
        Self { config }
    }

    /// Get the current configuration.
    pub fn config(&self) -> &CompressionConfig {
        &self.config
    }

    /// Determine compression level from pressure and risk.
    ///
    /// # Rules
    /// - High/Critical risk tasks NEVER go to Emergency compression
    /// - The level is determined by pressure_normalized thresholds
    /// - Risk can only REDUCE compression (never increase it)
    pub fn determine_level(
        &self,
        pressure: &ThoughtPressure,
        risk: RiskLevel,
    ) -> CompressionLevel {
        let raw_level = if pressure.pressure_normalized >= self.config.emergency_threshold {
            CompressionLevel::Emergency
        } else if pressure.pressure_normalized >= self.config.aggressive_threshold {
            CompressionLevel::Aggressive
        } else if pressure.pressure_normalized >= self.config.moderate_threshold {
            CompressionLevel::Moderate
        } else {
            CompressionLevel::Full
        };

        // Risk override: high-risk tasks cannot go to Emergency
        match (raw_level, risk) {
            (CompressionLevel::Emergency, RiskLevel::High | RiskLevel::Critical) => {
                CompressionLevel::Aggressive
            }
            (CompressionLevel::Aggressive, RiskLevel::Critical) => {
                CompressionLevel::Moderate
            }
            _ => raw_level,
        }
    }

    /// Compress a thought: given pressure and risk, return compression parameters.
    pub fn compress(
        &self,
        pressure: &ThoughtPressure,
        risk: RiskLevel,
    ) -> CompressionResult {
        let raw_level = if pressure.pressure_normalized >= self.config.emergency_threshold {
            CompressionLevel::Emergency
        } else if pressure.pressure_normalized >= self.config.aggressive_threshold {
            CompressionLevel::Aggressive
        } else if pressure.pressure_normalized >= self.config.moderate_threshold {
            CompressionLevel::Moderate
        } else {
            CompressionLevel::Full
        };

        let level = self.determine_level(pressure, risk);
        let emergency_blocked = raw_level == CompressionLevel::Emergency && level != CompressionLevel::Emergency;

        let (max_prompt_tokens, verification_layers) = match level {
            CompressionLevel::Full => (
                self.config.full_max_tokens,
                self.config.full_verification_layers,
            ),
            CompressionLevel::Moderate => (
                self.config.moderate_max_tokens,
                self.config.moderate_verification_layers,
            ),
            CompressionLevel::Aggressive => (
                self.config.aggressive_max_tokens,
                self.config.aggressive_verification_layers,
            ),
            CompressionLevel::Emergency => (
                self.config.emergency_max_tokens,
                self.config.emergency_verification_layers,
            ),
        };

        CompressionResult {
            level,
            max_prompt_tokens,
            verification_layers,
            input_pressure: pressure.pressure_normalized,
            emergency_blocked,
        }
    }
}

impl Default for ThoughtCompressor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pressure(normalized: f32) -> ThoughtPressure {
        ThoughtPressure {
            token_budget: 1000,
            time_budget_ms: 5000,
            pressure_normalized: normalized,
            phase_transitioned: false,
        }
    }

    #[test]
    fn test_low_pressure_full_compression() {
        let compressor = ThoughtCompressor::new();
        let level = compressor.determine_level(&make_pressure(0.1), RiskLevel::Low);
        assert_eq!(level, CompressionLevel::Full);
    }

    #[test]
    fn test_moderate_pressure() {
        let compressor = ThoughtCompressor::new();
        let level = compressor.determine_level(&make_pressure(0.4), RiskLevel::Low);
        assert_eq!(level, CompressionLevel::Moderate);
    }

    #[test]
    fn test_aggressive_pressure() {
        let compressor = ThoughtCompressor::new();
        let level = compressor.determine_level(&make_pressure(0.7), RiskLevel::Low);
        assert_eq!(level, CompressionLevel::Aggressive);
    }

    #[test]
    fn test_emergency_pressure_low_risk() {
        let compressor = ThoughtCompressor::new();
        let level = compressor.determine_level(&make_pressure(0.9), RiskLevel::Low);
        assert_eq!(level, CompressionLevel::Emergency);
    }

    #[test]
    fn test_emergency_blocked_high_risk() {
        let compressor = ThoughtCompressor::new();
        let level = compressor.determine_level(&make_pressure(0.9), RiskLevel::High);
        assert_eq!(level, CompressionLevel::Aggressive);
    }

    #[test]
    fn test_emergency_blocked_critical_risk() {
        let compressor = ThoughtCompressor::new();
        let level = compressor.determine_level(&make_pressure(0.9), RiskLevel::Critical);
        assert_eq!(level, CompressionLevel::Aggressive);
    }

    #[test]
    fn test_aggressive_downgraded_critical_risk() {
        let compressor = ThoughtCompressor::new();
        let level = compressor.determine_level(&make_pressure(0.7), RiskLevel::Critical);
        assert_eq!(level, CompressionLevel::Moderate);
    }

    #[test]
    fn test_compress_returns_correct_tokens() {
        let compressor = ThoughtCompressor::new();
        let result = compressor.compress(&make_pressure(0.1), RiskLevel::Low);
        assert_eq!(result.max_prompt_tokens, 4096);
        assert_eq!(result.verification_layers, 5);
        assert!(!result.emergency_blocked);
    }

    #[test]
    fn test_compress_emergency_blocked_flag() {
        let compressor = ThoughtCompressor::new();
        let result = compressor.compress(&make_pressure(0.9), RiskLevel::High);
        assert!(result.emergency_blocked);
        assert_eq!(result.level, CompressionLevel::Aggressive);
    }

    #[test]
    fn test_compress_moderate_tokens() {
        let compressor = ThoughtCompressor::new();
        let result = compressor.compress(&make_pressure(0.4), RiskLevel::Medium);
        assert_eq!(result.max_prompt_tokens, 2048);
        assert_eq!(result.verification_layers, 3);
    }

    #[test]
    fn test_compression_level_serde() {
        let level = CompressionLevel::Aggressive;
        let json = serde_json::to_string(&level).unwrap();
        let restored: CompressionLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, CompressionLevel::Aggressive);
    }

    #[test]
    fn test_config_serde() {
        let config = CompressionConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let restored: CompressionConfig = serde_json::from_str(&json).unwrap();
        assert!((restored.moderate_threshold - 0.3).abs() < 0.01);
    }

    #[test]
    fn test_custom_config() {
        let config = CompressionConfig {
            moderate_threshold: 0.5,
            aggressive_threshold: 0.8,
            emergency_threshold: 0.95,
            ..CompressionConfig::default()
        };
        let compressor = ThoughtCompressor::with_config(config);
        // At 0.4, should still be Full (threshold is 0.5)
        let level = compressor.determine_level(&make_pressure(0.4), RiskLevel::Low);
        assert_eq!(level, CompressionLevel::Full);
        // At 0.6, should be Moderate (threshold is 0.5)
        let level = compressor.determine_level(&make_pressure(0.6), RiskLevel::Low);
        assert_eq!(level, CompressionLevel::Moderate);
    }

    #[test]
    fn test_zero_pressure() {
        let compressor = ThoughtCompressor::new();
        let level = compressor.determine_level(&make_pressure(0.0), RiskLevel::Low);
        assert_eq!(level, CompressionLevel::Full);
    }

    #[test]
    fn test_max_pressure() {
        let compressor = ThoughtCompressor::new();
        let level = compressor.determine_level(&make_pressure(1.0), RiskLevel::Low);
        assert_eq!(level, CompressionLevel::Emergency);
    }

    #[test]
    fn test_result_serde() {
        let result = CompressionResult {
            level: CompressionLevel::Moderate,
            max_prompt_tokens: 2048,
            verification_layers: 3,
            input_pressure: 0.45,
            emergency_blocked: false,
        };
        let json = serde_json::to_string(&result).unwrap();
        let restored: CompressionResult = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.level, CompressionLevel::Moderate);
    }
}
```

### contracts/src/lib.rs — add module declaration:
```rust
pub mod compression;
pub use compression::*;
```

## Validation

```bash
cd contracts && cargo test --workspace 2>&1
```

Expected: All tests pass, zero warnings.

## Done Criteria

- [ ] `contracts/src/compression.rs` exists
- [ ] `CompressionLevel` enum with 4 levels (Full, Moderate, Aggressive, Emergency)
- [ ] `CompressionConfig` with configurable thresholds and token limits
- [ ] `ThoughtCompressor` with determine_level() and compress()
- [ ] High/Critical risk blocks Emergency compression
- [ ] Critical risk downgrades Aggressive to Moderate
- [ ] 16+ tests pass
- [ ] All previous tests still pass

## Git

```bash
git add -A
git commit -m "P2-02: Thought Compression — budget-adaptive prompt optimization

- CompressionLevel: Full, Moderate, Aggressive, Emergency
- ThoughtCompressor: pressure→level with risk override
- High/Critical risk blocks Emergency, Critical downgrades Aggressive
- CompressionConfig with configurable thresholds
- 16+ tests"
git push -u origin P2-02-thought-compression
gh pr create --title "P2-02: Thought Compression" --body "$(cat tasks/P2-02-thought-compression.md)"
```
