# Autolight Agent Instructions

Start every implementation pass from `docs/NOW.md`.

Do not begin by scanning old local notes or stale PR transcripts. `docs/NOW.md` is the active source of truth unless it says it is blocked or stale.

## Routing Order

1. Read `docs/NOW.md`.
2. Verify only the active batch and its immediate prerequisites in code.
3. Use `docs/ROADMAP.md` only if `docs/NOW.md` is complete, blocked, or stale.
4. Use `docs/PROCESS.md` for batch and handoff rules.

## Current Direction

All product work targets the Rust/CXX-Qt app. Qt Quick/QML remains the UI layer; Rust owns the runtime, project model, job execution, timeline model, and controller surfaces.

## Batch Rules

- Work one batch at a time.
- Keep target paths narrow.
- Update `docs/NOW.md` with status, verification, and handoff notes in the same pass as code changes.
- Do not create another long implementation transcript.
- If a plan needs more than one screen to understand, split it into smaller batches.

## Done Criteria

A batch is done only when:

- Code/docs for that batch are updated.
- Listed verification commands have been run or the blocker is recorded.
- `docs/NOW.md` says what changed and what the next batch is.
- The final handoff is short enough for the next agent to act on without rereading historical plans.
