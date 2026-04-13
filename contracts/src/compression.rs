//! Thought Compression — budget-adaptive prompt optimization.
//!
//! Under high pressure, the system compresses thinking:
//! - Full: complete prompts, all verification layers, best models
//! - Moderate: shortened prompts, skip low-value verification
//! - Aggressive: minimal prompts, essential verification only
//! - Emergency: single-shot, no verification (blocked for high-risk)

use crate::{RiskLevel, ThoughtPressure};
use serde::{Deserialize, Serialize};

/// Compression levels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CompressionLevel {
    /// No compression. Full prompts, all verification.
    Full,
    /// Shorter prompts, skip low-value verification.
    Moderate,
    /// Minimal prompts, essential verification only.
    Aggressive,
    /// Single-shot, no verification. Only for low-risk.
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
    /// Max prompt tokens at each level.
    pub full_max_tokens: u32,
    /// Max prompt tokens at Moderate.
    pub moderate_max_tokens: u32,
    /// Max prompt tokens at Aggressive.
    pub aggressive_max_tokens: u32,
    /// Max prompt tokens at Emergency.
    pub emergency_max_tokens: u32,
    /// Verification layers at each level.
    pub full_verification_layers: u8,
    /// Verification layers at Moderate.
    pub moderate_verification_layers: u8,
    /// Verification layers at Aggressive.
    pub aggressive_verification_layers: u8,
    /// Verification layers at Emergency.
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

/// Result of applying compression.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompressionResult {
    /// The compression level applied.
    pub level: CompressionLevel,
    /// Maximum tokens allowed for the prompt.
    pub max_prompt_tokens: u32,
    /// Number of verification layers to use.
    pub verification_layers: u8,
    /// The input pressure.
    pub input_pressure: f32,
    /// Whether emergency was blocked due to high risk.
    pub emergency_blocked: bool,
}

/// The thought compressor.
pub struct ThoughtCompressor {
    config: CompressionConfig,
}

impl ThoughtCompressor {
    /// Create with default config.
    pub fn new() -> Self {
        Self { config: CompressionConfig::default() }
    }

    /// Create with custom config.
    pub fn with_config(config: CompressionConfig) -> Self {
        Self { config }
    }

    /// Get the current configuration.
    pub fn config(&self) -> &CompressionConfig {
        &self.config
    }

    /// Determine compression level from pressure and risk.
    /// High/Critical risk NEVER goes to Emergency.
    /// Critical risk downgrades Aggressive to Moderate.
    pub fn determine_level(&self, pressure: &ThoughtPressure, risk: RiskLevel) -> CompressionLevel {
        let raw_level = if pressure.pressure_normalized >= self.config.emergency_threshold {
            CompressionLevel::Emergency
        } else if pressure.pressure_normalized >= self.config.aggressive_threshold {
            CompressionLevel::Aggressive
        } else if pressure.pressure_normalized >= self.config.moderate_threshold {
            CompressionLevel::Moderate
        } else {
            CompressionLevel::Full
        };

        match (raw_level, risk) {
            (CompressionLevel::Emergency, RiskLevel::High | RiskLevel::Critical) => CompressionLevel::Aggressive,
            (CompressionLevel::Aggressive, RiskLevel::Critical) => CompressionLevel::Moderate,
            _ => raw_level,
        }
    }

    /// Compress: given pressure and risk, return compression parameters.
    pub fn compress(&self, pressure: &ThoughtPressure, risk: RiskLevel) -> CompressionResult {
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
            CompressionLevel::Full => (self.config.full_max_tokens, self.config.full_verification_layers),
            CompressionLevel::Moderate => (self.config.moderate_max_tokens, self.config.moderate_verification_layers),
            CompressionLevel::Aggressive => (self.config.aggressive_max_tokens, self.config.aggressive_verification_layers),
            CompressionLevel::Emergency => (self.config.emergency_max_tokens, self.config.emergency_verification_layers),
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
    fn default() -> Self { Self::new() }
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
    fn test_low_pressure_full() {
        let c = ThoughtCompressor::new();
        assert_eq!(c.determine_level(&make_pressure(0.1), RiskLevel::Low), CompressionLevel::Full);
    }

    #[test]
    fn test_moderate_pressure() {
        let c = ThoughtCompressor::new();
        assert_eq!(c.determine_level(&make_pressure(0.4), RiskLevel::Low), CompressionLevel::Moderate);
    }

    #[test]
    fn test_aggressive_pressure() {
        let c = ThoughtCompressor::new();
        assert_eq!(c.determine_level(&make_pressure(0.7), RiskLevel::Low), CompressionLevel::Aggressive);
    }

    #[test]
    fn test_emergency_low_risk() {
        let c = ThoughtCompressor::new();
        assert_eq!(c.determine_level(&make_pressure(0.9), RiskLevel::Low), CompressionLevel::Emergency);
    }

    #[test]
    fn test_emergency_blocked_high_risk() {
        let c = ThoughtCompressor::new();
        assert_eq!(c.determine_level(&make_pressure(0.9), RiskLevel::High), CompressionLevel::Aggressive);
    }

    #[test]
    fn test_emergency_blocked_critical() {
        let c = ThoughtCompressor::new();
        assert_eq!(c.determine_level(&make_pressure(0.9), RiskLevel::Critical), CompressionLevel::Aggressive);
    }

    #[test]
    fn test_aggressive_downgraded_critical() {
        let c = ThoughtCompressor::new();
        assert_eq!(c.determine_level(&make_pressure(0.7), RiskLevel::Critical), CompressionLevel::Moderate);
    }

    #[test]
    fn test_compress_full_tokens() {
        let c = ThoughtCompressor::new();
        let r = c.compress(&make_pressure(0.1), RiskLevel::Low);
        assert_eq!(r.max_prompt_tokens, 4096);
        assert_eq!(r.verification_layers, 5);
        assert!(!r.emergency_blocked);
    }

    #[test]
    fn test_compress_emergency_blocked_flag() {
        let c = ThoughtCompressor::new();
        let r = c.compress(&make_pressure(0.9), RiskLevel::High);
        assert!(r.emergency_blocked);
        assert_eq!(r.level, CompressionLevel::Aggressive);
    }

    #[test]
    fn test_compress_moderate_tokens() {
        let c = ThoughtCompressor::new();
        let r = c.compress(&make_pressure(0.4), RiskLevel::Medium);
        assert_eq!(r.max_prompt_tokens, 2048);
        assert_eq!(r.verification_layers, 3);
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
        let c = ThoughtCompressor::with_config(config);
        assert_eq!(c.determine_level(&make_pressure(0.4), RiskLevel::Low), CompressionLevel::Full);
        assert_eq!(c.determine_level(&make_pressure(0.6), RiskLevel::Low), CompressionLevel::Moderate);
    }

    #[test]
    fn test_zero_pressure() {
        let c = ThoughtCompressor::new();
        assert_eq!(c.determine_level(&make_pressure(0.0), RiskLevel::Low), CompressionLevel::Full);
    }

    #[test]
    fn test_max_pressure() {
        let c = ThoughtCompressor::new();
        assert_eq!(c.determine_level(&make_pressure(1.0), RiskLevel::Low), CompressionLevel::Emergency);
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
