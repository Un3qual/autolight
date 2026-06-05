# Autolight Process

This repo uses a small active dispatcher instead of long live plans.

## Operating Model

- `AGENTS.md` is the repo-level prompt.
- `docs/NOW.md` is the only active implementation dispatcher.
- `docs/ROADMAP.md` is the ordered queue.
- `docs/templates/HANDOFF.md` is the handoff format for end-of-batch updates.

## Batch Shape

A good batch fits on one screen and has:

- one concrete goal;
- narrow target paths;
- immediate prerequisites only;
- a short implementation contract;
- exact verification commands;
- a handoff section.

If a batch needs multiple unrelated target areas, split it before implementation.

## Planning Rules

- Prefer `docs/NOW.md` updates over new long plans.
- Keep implementation plans short enough to execute without reading historical code snippets.
- Do not paste full source files into plans.

## Handoff Rules

Every implementation pass should end with a short handoff in `docs/NOW.md`:

- status: `ready`, `in_progress`, `complete`, or `blocked`;
- changes made;
- verification commands and results;
- blockers, if any;
- next recommended batch.

If work is blocked, name the exact blocker and the smallest next action. Avoid broad statements like "needs investigation."

## Promotion Rules

When `docs/NOW.md` is complete:

1. Pick the next `pending` item from `docs/ROADMAP.md`.
2. Rewrite `docs/NOW.md` around only that batch.
3. Leave the completed batch summary in the handoff notes or move it into a short completed section.

When `docs/NOW.md` is stale:

1. Verify the stale fact in code or docs.
2. Update NOW to match the current repo.
3. Continue with the smallest still-valid batch.

## Verification Policy

Use the narrowest command that proves the batch first. Run broader checks only at cutover points or when the batch touches shared contracts.

Prefer:

```bash
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo fmt --all -- --check
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked
QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke
git diff --check
```
