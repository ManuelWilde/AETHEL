# AETHEL Task Protocol

## What is this?

Each file in this directory is a **self-contained task** for building the AETHEL platform.
Each task can be executed by ANY AI coding agent (aider, Claude, GLM, Composer, SWP, Cursor, etc.) or by a human developer.

## Rules

1. **Execute tasks IN ORDER** (P0-01 before P0-02 before P1-01 etc.)
2. **Each task is self-contained** — all context needed is in the file
3. **Do not skip tasks** — each builds on the previous
4. **Run the validation command** at the end of each task before moving to the next
5. **Do not modify files outside the scope** listed in the task

## Task Naming

```
P{phase}-{number}-{short-name}.md
```

- P0 = Phase 0: Foundation (fix broken stuff)
- P1 = Phase 1: Capability System (the core)
- P2 = Phase 2: FIMAS + Thought Compression
- P3 = Phase 3: Persistence + Audit
- P4 = Phase 4: LLM + Routing
- P5 = Phase 5: First Real Capabilities
- P6 = Phase 6: Flutter UI
- P7 = Phase 7: Voice AI
- P8 = Phase 8: Advanced

## Project Structure

```
AETHEL/AETHEL/
├── Cargo.toml              (workspace root)
├── contracts/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs           (existing: 1037 lines, 51 types)
│       ├── error.rs          (NEW in P0-01)
│       ├── ids.rs            (NEW in P0-02)
│       ├── transitions.rs    (NEW in P0-03)
│       ├── budget.rs         (NEW in P0-04)
│       ├── capability.rs     (NEW in P1-01)
│       ├── pipeline.rs       (NEW in P1-02)
│       ├── registry.rs       (NEW in P1-03)
│       ├── fimas.rs          (NEW in P2-01)
│       ├── compression.rs    (NEW in P2-02)
│       ├── agent.rs          (NEW in P2-03)
│       └── storage.rs        (NEW in P3-01)
└── tasks/                   (this directory)
```

## Current State (as of task creation)

- `contracts/src/lib.rs`: 1037 lines, 51 public types, compiles, zero internal deps (only serde + serde_json)
- `contracts/Cargo.toml`: edition 2021, serde + serde_json dependencies
- Workspace `Cargo.toml`: contracts as sole member
- NO traits defined anywhere in contracts/
- NO error types in contracts/
- NO async anywhere in contracts/
- All IDs are plain `String` (not typesafe)
- ClaimState has 8 variants but no transition enforcement
- BudgetLease has no enforcement methods

## Validation

After each task, the agent should run:
```bash
cd /path/to/AETHEL/AETHEL && cargo test --workspace 2>&1
```

If cargo is not available, use:
```bash
python3 -c "
import re, sys
code = open('contracts/src/lib.rs').read()
# ... validation specific to the task
"
```
