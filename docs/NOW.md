# Autolight NOW

Updated: 2026-06-03

## Active Batch: Rust CXX-Qt Smoke Spike

**Status:** ready

**Goal:** Prove that a Rust binary can load the existing Qt Quick/QML shell through CXX-Qt and expose a minimal `AppController`-like object to QML.

**Why this batch:** The Rust port is locked to CXX-Qt. Nothing else should be ported until the Qt/Rust bridge can launch the current shell in offscreen smoke mode.

## Target Paths

- `Cargo.toml`
- `crates/autolight-qt/Cargo.toml`
- `crates/autolight-qt/build.rs`
- `crates/autolight-qt/src/lib.rs`
- `crates/autolight-qt/src/app_controller.rs`
- `crates/autolight-app/Cargo.toml`
- `crates/autolight-app/src/main.rs`
- `Cargo.lock`
- `UI/Main.qml` only if a minimal import or context adapter is required
- `README.md` only for new Rust smoke/run commands if the spike works

## Reference Docs

- Active architecture: `docs/superpowers/specs/2026-06-03-autolight-rust-cxx-qt-port-design.md`
- Background plan: `docs/superpowers/plans/2026-06-03-autolight-rust-cxx-qt-port.md`, Task 1 only
- Process rules: `docs/PROCESS.md`

Do not read older Python implementation plans unless the spike needs to understand an exact QML property or startup behavior.

## Implementation Contract

Build the smallest useful spike:

- A Cargo workspace exists.
- `autolight-app` starts a Qt application.
- `autolight-qt` registers or exposes a minimal Rust-backed controller.
- The controller exposes `projectName`, `lastError`, and `newProject()`.
- Any other controller properties, child objects, or models read during `UI/Main.qml` startup are stubbed with inert values so the existing QML shell can load without runtime binding errors.
- The Rust binary supports `--smoke`.
- Offscreen smoke proves the QML root loads and can observe at least one Rust controller value.
- `Cargo.lock` is created and committed before locked Cargo verification is required.

Do not port project schema, jobs, transforms, timeline models, or analysis in this batch.

## Verification

Run the commands that exist after the spike is implemented:

```bash
cargo fmt --all -- --check
cargo generate-lockfile
cargo test --workspace --locked
QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke
git diff --check
```

If CXX-Qt dependencies cannot be fetched because of sandboxed network access, rerun the dependency command with escalation and record the result in the handoff.

## Completion Update

When this batch is done, update this file:

- Set `Status` to `complete`.
- Add the exact commands run and whether they passed.
- Add the next active batch from `docs/ROADMAP.md`.

## Handoff Notes

- Current app entrypoint is Python `main.py`; Rust entrypoint does not exist yet.
- Existing QML root is `UI/Main.qml`.
- Python/PySide remains the reference app until Rust parity is reached.
