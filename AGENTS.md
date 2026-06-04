# Autolight Agent Instructions

Start every implementation pass from `docs/NOW.md`.

Do not begin by scanning `docs/superpowers/plans/` or old specs. Those files are historical reference material unless `docs/NOW.md` or `docs/ROADMAP.md` points to one by name.

## Routing Order

1. Read `docs/NOW.md`.
2. Verify only the active batch and its immediate prerequisites in code.
3. Use `docs/ROADMAP.md` only if `docs/NOW.md` is complete, blocked, or stale.
4. Use `docs/PROCESS.md` for batch and handoff rules.
5. Read historical Superpowers plans/specs only for parity details called out by the active batch.

## Current Direction

All forward product work targets the Rust/CXX-Qt app. The Python/PySide app is the reference implementation and parity baseline.

Python changes are allowed only when they preserve the reference app, add parity fixtures/tests, or unblock Rust migration.

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
