# Autolight NOW

Updated: 2026-06-05

## Active Batch: PR 13 Review-Bot Follow-Through Refresh

**Status:** complete

**Goal:** Pull current PR #13 commits, fetch unresolved and new bot feedback including stale/outside-diff/duplicate threads, fix valid findings, push, then reply where needed.

## Scope

- Fast-forwarded `codex/rust-runtime-port` from `befc331` to `6be9f26`.
- Refreshed PR #13 review threads, flat review comments, and bot status comments from GitHub.
- Found five actionable unresolved current bot threads plus one stale unresolved DeepSource thread before the first follow-up push.
- Fixed CodeRabbit's timing-sensitive async worker tests by adding a test-only ready-worker factory and replacing sleep/poll assumptions with deterministic worker results.
- Fixed Codex's generated-marker audio-parent finding by allowing complete generated `markers.v1` tracks to reuse their online source-audio context for audio-input transforms.
- Fixed Codex's generated-audio playback finding by preferring the selected track's valid `audio`/`stem` cache artifact before falling back to source audio.
- Fixed Codex's cache recovery finding by restoring cache-stale tracks and dependents when all cache refs validate again, and marking the controller dirty when Check Cache changes project state.
- Fixed Codex's runnerless rerun finding by gating selected-track rerun on the Rust job registry.
- DeepSource's unresolved `create_dir` comment is stale: current `main.rs` uses `DirBuilder::create(&root)` for exclusive temp-root creation and `create_dir_all` only for nested asset parents.
- After the first push, refreshed the newest review-thread page and found three new CodeRabbit comments.
- Fixed CodeRabbit's README Qt-install wording by replacing machine-specific wording with a Homebrew example plus generic Qt-distribution guidance.
- Fixed CodeRabbit's injected-controller startup finding by gating default-project bootstrap to the self-owned `AppRuntime`; injected `appController` instances still get viewport sizing without being reset.
- CodeRabbit's `TimelineView.qml` native dependency comment is not actionable for this branch: current product direction is Rust/CXX-Qt only, and the QML/native timeline scene is intentionally covered by Rust-only surface tests.

## Target Paths

- `README.md`
- `UI/Main.qml`
- `crates/autolight-core/src/cache.rs`
- `crates/autolight-core/src/transforms.rs`
- `crates/autolight-jobs/src/queue.rs`
- `crates/autolight-qt/src/app_controller/job_worker.rs`
- `crates/autolight-qt/src/app_controller/mod.rs`
- `crates/autolight-qt/src/app_controller/playback_controller.rs`
- `crates/autolight-qt/src/app_controller/tests.rs`
- `docs/NOW.md`

## Verification

Passed:

```bash
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked
cargo test -p autolight-jobs --locked jobs_refresh_cache_validity
cargo test -p autolight-core --locked transforms_audio_parent_compatibility_accepts_source_or_audio_artifact_context
cargo fmt --all -- --check
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke
git diff --check
```

## Handoff

PR #13 follow-through has no remaining local code/docs work for the latest fetched bot feedback. After the final follow-up is pushed and review-thread replies are posted, the next batch is only to monitor fresh bot reruns or human review; do not start new implementation work unless a fresh actionable thread appears.
