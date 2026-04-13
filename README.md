# AETHEL

**Epistemically Honest, Ontologically Routed, Bio-Adaptive Computation Platform**

AETHEL orchestrates small/own LLMs through a fractal control plane (FIMAS), routing tasks based on ontological dimensions, bio-signals, and epistemic integrity — fully EU AI Act compliant.

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                    AETHEL Runtime                     │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────┐ │
│  │ Bio-Gate  │ │ Triplex  │ │  Thought │ │ Audit  │ │
│  │ (Schmitt) │ │   Via    │ │ Compress │ │ Chain  │ │
│  └──────────┘ └──────────┘ └──────────┘ └────────┘ │
│  ┌──────────────────────────────────────────────────┐│
│  │              FIMAS Executor                       ││
│  │  TaskQueue → AgentRunner → BudgetLease → Report  ││
│  └──────────────────────────────────────────────────┘│
│  ┌──────────┐ ┌──────────┐ ┌──────────┐            │
│  │CapRegistry│ │ Pipeline │ │ App Comp │            │
│  └──────────┘ └──────────┘ └──────────┘            │
└─────────────────────────────────────────────────────┘
         │                        │
    ┌────┴────┐             ┌────┴────┐
    │ SQLite  │             │  REST   │
    │ Storage │             │  API    │
    └─────────┘             └─────────┘
```

## Workspace Crates

| Crate | Purpose |
|-------|---------|
| `aethel_contracts` | Single source of truth — all types, traits, state machines |
| `aethel_engine` | Runtime: FIMAS executor, agent runner, task queue |
| `aethel_storage` | SQLite persistence for claims, traces, reports |
| `aethel_cli` | Command-line interface (`aethel`) |
| `aethel_api` | REST API server (axum) |

## Key Concepts

**OmegaSpectrum24** — 32-element `[f32; 32]` vector (128 bytes, cache-line aligned) encoding 12 ontological dimensions (WeltΩ: Hyleron through Noetikon), 7 Spheres, Apeiron, Meta, and reserved slots.

**FIMAS** — Fractal Intelligent Multi-Agent System. Large tasks are decomposed into sub-tasks via `DecompositionPlan`, executed by agents with individual `BudgetLease` allocations. Supports sequential, parallel, hierarchical, and adaptive strategies.

**ClaimState Machine** — 8 states (Generated → Supported → Accepted/Deferred/Escalated/Revised/Rejected/Retired) with 13 valid transitions and 51 explicitly blocked invalid ones.

**Bio-Gate** — Schmitt-Trigger hysteresis (activate at 0.70, deactivate at 0.55) that routes to local/safe providers when the user is under stress.

**Triplex Via** — Three-phase routing: Purificatio (policy constraints) → Illuminatio (bio + ontology scoring) → Unio (optimal provider selection).

**Thought Compression** — 4 levels (Full/Moderate/Aggressive/Emergency) with risk-based safety overrides preventing aggressive compression on high-risk tasks.

**EU AI Act Compliance** — `AuditChain` (append-only, hash-chained, tamper detection), `ComplianceManifest` with risk tier classification.

## Quick Start

```bash
# Build everything
cargo build --release

# Run tests (234 unit + 22 integration)
cargo test --workspace

# Initialize database
./target/release/aethel init

# System status
./target/release/aethel system status

# Create a claim
./target/release/aethel claim add "The sky is blue" --risk Low --confidence 0.95

# List claims
./target/release/aethel claim list

# Process bio-signal
./target/release/aethel bio signal 0.8 0.3 0.4

# Verify audit chain
./target/release/aethel audit verify

# Start API server
./target/release/aethel-server
# → http://localhost:3000/health
```

## API Endpoints

```
GET    /health                       Health check
GET    /api/v1/system/summary        System summary
POST   /api/v1/claims                Create claim
GET    /api/v1/claims                List claims (?offset=0&limit=20)
GET    /api/v1/claims/:id            Get claim
DELETE /api/v1/claims/:id            Delete claim
POST   /api/v1/claims/:id/transition Transition state {"target_state": "Supported"}
POST   /api/v1/bio/signal            Bio-signal {"stress": 0.8, "coherence": 0.3, "focus": 0.4}
GET    /api/v1/audit/verify          Verify audit chain integrity
POST   /api/v1/audit/record          Record decision {"decision": "...", "risk": "Medium"}
```

## Docker

```bash
# Build and run
docker compose up -d

# Or standalone
docker build -t aethel .
docker run -p 3000:3000 -v aethel-data:/app/data aethel
```

## Project Stats

- **6 crates**, **31 .rs files**, **~9,000 lines**, **256 tests**
- 51 core types in contracts
- 17 modules in contracts alone
- All state machines fully tested (every valid + invalid transition)
- Zero `unsafe` code (`#![forbid(unsafe_code)]`)

## License

MIT — Manuel Wilde
