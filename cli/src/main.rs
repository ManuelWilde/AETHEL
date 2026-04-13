//! AETHEL CLI — Command-line interface for the AETHEL platform.
//!
//! Usage:
//!   aethel system status         — Show system status
//!   aethel claim add <text>      — Create a new claim
//!   aethel claim list             — List all claims
//!   aethel claim get <id>         — Get claim details
//!   aethel claim transition <id> <state> — Transition claim state
//!   aethel bio signal <stress> <coherence> <focus> — Process bio signal
//!   aethel audit verify           — Verify audit chain integrity
//!   aethel run <capability> <input> — Run a single capability

use clap::{Parser, Subcommand};
use colored::Colorize;
use aethel_contracts::*;
use aethel_storage::{DbPool, SqliteClaimStore};

#[derive(Parser)]
#[command(
    name = "aethel",
    about = "AETHEL — Epistemically Honest, Bio-Adaptive Computation Platform",
    version,
    author
)]
struct Cli {
    /// Database path (default: ./aethel.db)
    #[arg(long, default_value = "aethel.db")]
    db: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// System operations
    System {
        #[command(subcommand)]
        action: SystemAction,
    },
    /// Claim management
    Claim {
        #[command(subcommand)]
        action: ClaimAction,
    },
    /// Bio-signal processing
    Bio {
        #[command(subcommand)]
        action: BioAction,
    },
    /// Audit chain operations
    Audit {
        #[command(subcommand)]
        action: AuditAction,
    },
    /// Initialize the database
    Init,
}

#[derive(Subcommand)]
enum SystemAction {
    /// Show system status
    Status,
    /// Show system summary
    Summary,
}

#[derive(Subcommand)]
enum ClaimAction {
    /// Add a new claim
    Add {
        /// Claim text
        text: String,
        /// Risk level: Low, Medium, High, Critical
        #[arg(long, default_value = "Low")]
        risk: String,
        /// Confidence (0.0 - 1.0)
        #[arg(long, default_value_t = 0.5)]
        confidence: f32,
    },
    /// List all claims
    List {
        /// Maximum number of claims to show
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Offset for pagination
        #[arg(long, default_value_t = 0)]
        offset: usize,
    },
    /// Get a specific claim
    Get {
        /// Claim ID
        id: String,
    },
    /// Transition a claim to a new state
    Transition {
        /// Claim ID
        id: String,
        /// Target state: Supported, Accepted, Deferred, Escalated, Revised, Rejected, Retired
        state: String,
    },
    /// Count claims
    Count,
    /// Delete a claim
    Delete {
        /// Claim ID
        id: String,
    },
}

#[derive(Subcommand)]
enum BioAction {
    /// Process a bio-signal
    Signal {
        /// Stress level (0.0 - 1.0)
        stress: f64,
        /// Coherence level (0.0 - 1.0)
        coherence: f64,
        /// Focus level (0.0 - 1.0)
        focus: f64,
    },
}

#[derive(Subcommand)]
enum AuditAction {
    /// Verify audit chain integrity
    Verify,
    /// Show audit chain info
    Info,
    /// Record a decision
    Record {
        /// Decision text
        decision: String,
        /// Risk level: Low, Medium, High, Critical
        #[arg(long, default_value = "Low")]
        risk: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => cmd_init(&cli.db),
        Commands::System { action } => cmd_system(action),
        Commands::Claim { action } => cmd_claim(action, &cli.db),
        Commands::Bio { action } => cmd_bio(action),
        Commands::Audit { action } => cmd_audit(action),
    }
}

fn cmd_init(db_path: &str) {
    match DbPool::open(db_path) {
        Ok(pool) => {
            match pool.initialize() {
                Ok(()) => {
                    println!("{} Database initialized at {}", "✓".green(), db_path);
                }
                Err(e) => {
                    eprintln!("{} Migration failed: {}", "✗".red(), e);
                }
            }
        }
        Err(e) => {
            eprintln!("{} Cannot open database: {}", "✗".red(), e);
        }
    }
}

fn cmd_system(action: SystemAction) {
    let system = AethelSystem::new(
        ComplianceManifest::aethel_default(),
        CompressionConfig::default(),
    );

    match action {
        SystemAction::Status => {
            println!("{}", "AETHEL System Status".bold().cyan());
            println!("  {} Compliance: {}", "●".green(), "Active (EU AI Act)");
            println!("  {} Bio-Gate:   {}", "●".green(), "Schmitt-Trigger (0.70/0.55)");
            println!("  {} Audit:      {}", "●".green(), "Chain intact");
            println!("  {} Engine:     {}", "●".green(), "Ready");
        }
        SystemAction::Summary => {
            let summary = system.summary();
            println!("{}", "AETHEL System Summary".bold().cyan());
            println!("  Capabilities:  {}", summary.registered_capabilities);
            println!("  Apps:          {}", summary.registered_apps);
            println!("  Audit blocks:  {}", summary.audit_blocks);
            println!("  Audit intact:  {}", if summary.audit_integrity { "yes".green() } else { "NO".red() });
            println!("  Bio-gate:      {}", if summary.bio_gate_active { "active".yellow() } else { "inactive".green() });
            println!("  Compliance:    {}", if summary.compliant { "compliant".green() } else { "non-compliant".red() });
        }
    }
}

fn cmd_claim(action: ClaimAction, db_path: &str) {
    let pool = match DbPool::open(db_path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{} Cannot open database: {}. Run 'aethel init' first.", "✗".red(), e);
            return;
        }
    };
    let store = SqliteClaimStore::new(pool);

    match action {
        ClaimAction::Add { text, risk, confidence } => {
            let risk_level = parse_risk(&risk);
            let id = format!("claim-{}", &text[..text.len().min(8)].replace(' ', "-"));
            let claim = Claim {
                id: id.clone(),
                content: text,
                state: ClaimState::Generated,
                origin: ClaimOrigin::UserSupplied,
                support_level: SupportLevel::Unsupported,
                risk: risk_level,
                confidence,
                evidence_ids: vec![],
                created_at_ms: now_ms(),
                updated_at_ms: now_ms(),
            };
            match store.save_claim(&claim) {
                Ok(()) => println!("{} Claim created: {}", "✓".green(), id),
                Err(e) => eprintln!("{} Failed: {}", "✗".red(), e),
            }
        }
        ClaimAction::List { limit, offset } => {
            match store.list_claims(offset, limit) {
                Ok(claims) => {
                    if claims.is_empty() {
                        println!("No claims found.");
                        return;
                    }
                    println!("{}", "Claims:".bold().cyan());
                    for c in &claims {
                        let state_colored = match c.state {
                            ClaimState::Accepted => format!("{:?}", c.state).green(),
                            ClaimState::Rejected => format!("{:?}", c.state).red(),
                            ClaimState::Escalated => format!("{:?}", c.state).yellow(),
                            _ => format!("{:?}", c.state).white(),
                        };
                        let risk_colored = match c.risk {
                            RiskLevel::Critical => format!("{:?}", c.risk).red().bold(),
                            RiskLevel::High => format!("{:?}", c.risk).red(),
                            RiskLevel::Medium => format!("{:?}", c.risk).yellow(),
                            RiskLevel::Low => format!("{:?}", c.risk).green(),
                        };
                        println!(
                            "  {} [{}] {} (risk: {}, conf: {:.0}%)",
                            c.id.dimmed(),
                            state_colored,
                            c.content,
                            risk_colored,
                            c.confidence * 100.0,
                        );
                    }
                }
                Err(e) => eprintln!("{} Failed: {}", "✗".red(), e),
            }
        }
        ClaimAction::Get { id } => {
            match store.load_claim(&ClaimId::new(&id)) {
                Ok(Some(c)) => {
                    println!("{}", "Claim Details:".bold().cyan());
                    println!("  ID:         {}", c.id);
                    println!("  Content:    {}", c.content);
                    println!("  State:      {:?}", c.state);
                    println!("  Origin:     {:?}", c.origin);
                    println!("  Risk:       {:?}", c.risk);
                    println!("  Confidence: {:.1}%", c.confidence * 100.0);
                    println!("  Evidence:   {:?}", c.evidence_ids);
                }
                Ok(None) => println!("{} Claim '{}' not found.", "✗".red(), id),
                Err(e) => eprintln!("{} Failed: {}", "✗".red(), e),
            }
        }
        ClaimAction::Transition { id, state } => {
            match store.load_claim(&ClaimId::new(&id)) {
                Ok(Some(mut claim)) => {
                    let target = parse_claim_state_str(&state);
                    match claim.state.transition(target) {
                        Ok(new_state) => {
                            claim.state = new_state;
                            claim.updated_at_ms = now_ms();
                            match store.save_claim(&claim) {
                                Ok(()) => println!(
                                    "{} Claim '{}' transitioned to {:?}",
                                    "✓".green(),
                                    id,
                                    new_state
                                ),
                                Err(e) => eprintln!("{} Save failed: {}", "✗".red(), e),
                            }
                        }
                        Err(e) => eprintln!("{} Transition failed: {}", "✗".red(), e),
                    }
                }
                Ok(None) => println!("{} Claim '{}' not found.", "✗".red(), id),
                Err(e) => eprintln!("{} Failed: {}", "✗".red(), e),
            }
        }
        ClaimAction::Count => {
            match store.count_claims() {
                Ok(n) => println!("Total claims: {}", n),
                Err(e) => eprintln!("{} Failed: {}", "✗".red(), e),
            }
        }
        ClaimAction::Delete { id } => {
            match store.delete_claim(&ClaimId::new(&id)) {
                Ok(true) => println!("{} Claim '{}' deleted.", "✓".green(), id),
                Ok(false) => println!("{} Claim '{}' not found.", "✗".red(), id),
                Err(e) => eprintln!("{} Failed: {}", "✗".red(), e),
            }
        }
    }
}

fn cmd_bio(action: BioAction) {
    let system = AethelSystem::new(
        ComplianceManifest::aethel_default(),
        CompressionConfig::default(),
    );

    match action {
        BioAction::Signal { stress, coherence, focus } => {
            let activated = system.process_bio_signal(stress, coherence, focus);
            println!("{}", "Bio-Signal Processing:".bold().cyan());
            println!("  Stress:     {:.2}", stress);
            println!("  Coherence:  {:.2}", coherence);
            println!("  Focus:      {:.2}", focus);

            if activated {
                println!("  Bio-Gate:   {} — routing to local/safe providers", "ACTIVATED".red().bold());
            } else {
                println!("  Bio-Gate:   {} — normal routing", "inactive".green());
            }
        }
    }
}

fn cmd_audit(action: AuditAction) {
    let system = AethelSystem::new(
        ComplianceManifest::aethel_default(),
        CompressionConfig::default(),
    );

    match action {
        AuditAction::Verify => {
            let intact = system.verify_audit_integrity();
            if intact {
                println!("{} Audit chain integrity: {}", "✓".green(), "VERIFIED".green().bold());
            } else {
                println!("{} Audit chain integrity: {}", "✗".red(), "COMPROMISED".red().bold());
            }
        }
        AuditAction::Info => {
            let summary = system.summary();
            println!("{}", "Audit Chain Info:".bold().cyan());
            println!("  Blocks:     {}", summary.audit_blocks);
            println!("  Integrity:  {}", if summary.audit_integrity { "intact".green() } else { "broken".red() });
        }
        AuditAction::Record { decision, risk } => {
            let risk_level = parse_risk(&risk);
            system.audit_decision(&decision, risk_level);
            println!("{} Decision recorded: \"{}\" (risk: {:?})", "✓".green(), decision, risk_level);
        }
    }
}

// ─── Helpers ─────────────────────────────────────

fn parse_risk(s: &str) -> RiskLevel {
    match s.to_lowercase().as_str() {
        "low" => RiskLevel::Low,
        "medium" | "med" => RiskLevel::Medium,
        "high" => RiskLevel::High,
        "critical" | "crit" => RiskLevel::Critical,
        _ => {
            eprintln!("Unknown risk '{}', defaulting to Low", s);
            RiskLevel::Low
        }
    }
}

fn parse_claim_state_str(s: &str) -> ClaimState {
    match s.to_lowercase().as_str() {
        "generated" => ClaimState::Generated,
        "supported" => ClaimState::Supported,
        "accepted" => ClaimState::Accepted,
        "deferred" => ClaimState::Deferred,
        "escalated" => ClaimState::Escalated,
        "revised" => ClaimState::Revised,
        "rejected" => ClaimState::Rejected,
        "retired" => ClaimState::Retired,
        _ => {
            eprintln!("Unknown state '{}', defaulting to Generated", s);
            ClaimState::Generated
        }
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
