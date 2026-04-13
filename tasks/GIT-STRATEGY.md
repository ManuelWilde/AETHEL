# AETHEL Git & GitHub Strategy

## Goal

Multiple AI coding agents (aider, Claude, GLM 5.1, Composer 2.0, SWP 1.5, Cursor) and human developers work **in parallel** on AETHEL. They must not conflict. They must validate each other's work. GitHub enforces quality.

## Branch Strategy

```
main                        ← Protected. Only via PR. Always compiles. All tests green.
├── P0-01-error-type        ← One branch per task
├── P0-02-newtype-ids       ← Can be worked on in parallel IF no file overlap
├── P0-03-claim-transitions
├── P0-04-budget-enforcement
├── P1-01-capability-trait
└── ...
```

### Rules

1. **One branch per task file** (branch name = task name without `.md`)
2. **Branch from `main`** (never from another feature branch)
3. **One PR per branch** (squash-merge into main)
4. **Delete branch after merge**
5. **Rebase on main before PR** (avoid merge commits)

## Parallel Work Rules

### Which tasks can run in parallel?

Tasks within the SAME phase that touch DIFFERENT files can run in parallel.
Tasks that touch the SAME file must run sequentially.

```
P0-01 (error.rs)          ┐
P0-02 (ids.rs)            ├── ALL PARALLEL (different files)
P0-03 (transitions.rs)    │   BUT: all add 1 line to lib.rs
P0-04 (budget.rs)         ┘   → merge conflicts in lib.rs are trivial (just `pub mod X;`)

P1-01 (capability.rs)     ┐
P1-02 (pipeline.rs)       ├── SEQUENTIAL (pipeline depends on capability trait)
P1-03 (registry.rs)       │
P1-04 (executor.rs)       ┘

P2-01 (fimas.rs)          ┐
P2-02 (compression.rs)    ├── PARALLEL (different files, both depend on P1)
P2-03 (agent.rs)          ┘   BUT P2-03 depends on P2-01 types
```

### Conflict Resolution

When two agents modify `lib.rs` (adding `pub mod X;` lines):
- The SECOND agent to merge will have a conflict
- Resolution is always: **keep both lines** (both module declarations are needed)
- Any agent can resolve this — it's a trivial merge

## GitHub Setup

### 1. Create Repository

```bash
gh repo create ManuelWilde/aethel-v7 --private --description "AETHEL Unified Platform" --clone
# OR if repo already exists:
cd AETHEL/AETHEL
git remote add origin https://github.com/ManuelWilde/aethel-v7.git
git push -u origin main
```

### 2. Branch Protection (main)

```bash
gh api repos/ManuelWilde/aethel-v7/branches/main/protection -X PUT -f '{
  "required_status_checks": {
    "strict": true,
    "contexts": ["cargo-test"]
  },
  "enforce_admins": false,
  "required_pull_request_reviews": null,
  "restrictions": null
}'
```

Or via GitHub UI: Settings → Branches → Add rule for `main`:
- [x] Require status checks to pass before merging
- [x] Require branches to be up to date before merging
- [x] Required checks: `cargo-test`

### 3. GitHub Actions CI

Create `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  cargo-test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Check formatting
        run: cargo fmt --all -- --check
      - name: Clippy
        run: cargo clippy --workspace -- -D warnings
      - name: Test
        run: cargo test --workspace
      - name: Doc
        run: cargo doc --workspace --no-deps
```

### 4. GitHub Issues (one per task)

Create issues automatically from task files:

```bash
for task in tasks/P*.md; do
  title=$(head -1 "$task" | sed 's/^# //')
  phase=$(basename "$task" .md | cut -d- -f1)
  gh issue create \
    --title "$title" \
    --body "$(cat $task)" \
    --label "$phase" \
    --label "agent-ready"
done
```

### 5. GitHub Projects Board

```
┌──────────┬──────────────┬─────────────┬──────────────┬────────┐
│ Backlog  │ Ready        │ In Progress │ In Review    │ Done   │
├──────────┼──────────────┼─────────────┼──────────────┼────────┤
│ P1-01    │ P0-03        │ P0-01 🤖    │              │        │
│ P1-02    │ P0-04        │ P0-02 🤖    │              │        │
│ P2-01    │              │             │              │        │
│ ...      │              │             │              │        │
└──────────┴──────────────┴─────────────┴──────────────┴────────┘
```

Move issues through columns. Label with agent name when assigned.

## Agent-Specific Instructions

### For aider

```bash
# 1. Pull latest main
git checkout main && git pull

# 2. Create branch
git checkout -b P0-01-error-type

# 3. Run aider with the task file as context
aider --read tasks/P0-01-error-type.md contracts/src/lib.rs contracts/Cargo.toml

# 4. In aider, say:
# "Read the task file P0-01-error-type.md and implement everything it says.
#  Create contracts/src/error.rs, modify contracts/Cargo.toml and contracts/src/lib.rs.
#  Then run cargo test to verify."

# 5. After aider is done:
cargo test --workspace
git push -u origin P0-01-error-type
gh pr create --title "P0-01: Add AethelError to contracts" --body "$(cat tasks/P0-01-error-type.md)"
```

### For Claude Code / Claude

```bash
# 1. Open the task file
cat tasks/P0-01-error-type.md

# 2. Claude reads the task and implements it
# Claude creates files, runs tests, commits

# 3. Push and create PR
git push -u origin P0-01-error-type
gh pr create --fill
```

### For GLM 5.1 / Composer 2.0 / SWP 1.5

```
Prompt template:

"You are working on a Rust project. Read the following task specification
and implement EXACTLY what it says. Do not deviate. Do not add extra features.
Create the files listed, write the code shown, and ensure all tests pass.

Task specification:
[paste content of P0-XX-taskname.md]

After implementing, run: cargo test --workspace
Report the test results."
```

### For Cursor

1. Open the AETHEL/AETHEL workspace
2. Open the task file (e.g., `tasks/P0-01-error-type.md`)
3. Select all text, press Cmd+K
4. Say: "Implement this task specification exactly as written"
5. Review and apply changes
6. Run `cargo test` in terminal

### For human developers

1. Read the task file
2. Implement it
3. Run `cargo test --workspace`
4. Commit with the message template in the task file
5. Push and create PR

## PR Template

When creating a PR, use this template:

```markdown
## Task: P0-XX: [Title]

### Changes
- [List of files created/modified]

### Tests
- [X] All new tests pass
- [X] All existing tests still pass
- [X] cargo clippy clean
- [X] cargo fmt clean

### Executed by
- Agent: [aider/Claude/GLM/Composer/SWP/human]
- Time: [how long it took]
```

## Review Process

1. **CI must pass** (cargo test, clippy, fmt)
2. **Any agent or human can review** — not just the author
3. **Review checklist:**
   - [ ] Does the code match the task specification?
   - [ ] Are all tests from the task file present?
   - [ ] Does it modify only the files listed in the task?
   - [ ] Is the commit message correct?
4. **Merge: squash-merge into main**
5. **Delete the branch after merge**

## Handling Merge Conflicts

Most conflicts will be in `contracts/src/lib.rs` (multiple tasks adding `pub mod X;`).

Resolution: **Always keep all `pub mod` lines.** They are independent declarations.

```bash
# When a PR has conflicts:
git checkout P0-03-claim-transitions
git rebase main
# Resolve: keep all pub mod lines
git add contracts/src/lib.rs
git rebase --continue
git push --force-with-lease
```

## Labels

| Label | Meaning |
|-------|---------|
| `P0` | Phase 0: Foundation |
| `P1` | Phase 1: Capability |
| `P2` | Phase 2: FIMAS |
| `P3` | Phase 3: Persistence |
| `agent-ready` | Task can be picked up by any agent |
| `in-progress` | Being worked on |
| `needs-review` | PR exists, needs review |
| `blocked` | Depends on another task |

## Dependency Graph

```
P0-01 ──┐
P0-02 ──┤
P0-03 ──┼── P1-01 ── P1-02 ── P1-03 ── P1-04 ──┐
P0-04 ──┘                                        ├── P2-01 ──┐
                                                  │           ├── P2-03
                                                  ├── P2-02 ──┘
                                                  │
                                                  └── P3-01 ── P3-02
```

P0-01 through P0-04 can ALL run in parallel.
P1-01 through P1-04 must run sequentially.
P2-01 and P2-02 can run in parallel (after P1).
P3-01 and P3-02 can run after P1.
