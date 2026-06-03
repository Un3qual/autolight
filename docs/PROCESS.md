# Autolight Process

This repo uses a small active dispatcher instead of long live plans.

## Operating Model

- `AGENTS.md` is the repo-level prompt.
- `docs/NOW.md` is the only active implementation dispatcher.
- `docs/ROADMAP.md` is the ordered queue.
- `docs/superpowers/specs/` contains behavior and architecture references.
- `docs/superpowers/plans/` contains historical plans or background migration plans, not the active execution queue.
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
- Keep specs focused on durable behavior and architecture decisions.
- Keep implementation plans short enough to execute without reading historical code snippets.
- Do not paste full source files into plans.
- Do not mark dozens of historical checkboxes as a primary progress signal.
- Historical Superpowers plans can remain for provenance, but should not drive new work unless explicitly referenced by NOW.

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

For Rust batches, prefer:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --locked
QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke
git diff --check
```

For Python reference-app checks, use:

```bash
uv run python -m unittest discover -s tests -v
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
```

Python checks are reference/parity checks after the Rust direction is locked. They do not justify new product work in Python.
