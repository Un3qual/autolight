# Autolight NOW

Updated: 2026-06-05

## Active Batch: PR 14 Review-Bot Follow-Through Refresh

**Status:** complete

**Goal:** Pull current PR #14 commits, fetch unresolved/new bot feedback including stale duplicates, fix actionable native timeline findings, push, then reply to threads that need responses.

## Scope

- Pulled `codex/native-timeline-navigation`; PR #14 was already current at `e02807d`.
- Fetched PR #14 review threads, top-level bot comments, stale/outdated duplicate threads, and review summaries from GitHub.
- Found three new unresolved Codex bot threads.
- Fixed the cache-only waveform finding by allowing the native scene snapshot to build waveform previews from validated cache-backed `waveformRef` payloads when inline `waveform_payload` provenance is absent, without marking the project dirty or reintroducing QML waveform rendering.
- Fixed the scrolled-row clipping finding by adding row-viewport and row-lane clip helpers that keep row backgrounds, labels, tree chrome, waveform bodies, analysis previews, and marker bodies below the ruler while leaving intentional ruler/playhead drawing paths intact.
- Analyzed the QML mirror snapshot finding with a 5.5 xhigh subagent and verified it was not valid as stated: marker selection, marker move/resize, and track expansion already call `reloadModels()` or `reloadTimelineSceneSnapshot()`. Strengthened the source contract test for those mutation wrappers instead of changing production QML.
- Used two 5.5 xhigh subagents for independent review-thread analysis: one for cache-backed waveforms and one for QML snapshot refresh.

## Target Paths

- `crates/autolight-qt/src/app_controller/tests.rs`
- `crates/autolight-qt/src/timeline_scene/scene_frame_builder.h`
- `crates/autolight-qt/src/timeline_scene/scene_frame_builder.cpp`
- `crates/autolight-qt/src/timeline_scene/model.rs`
- `crates/autolight-qt/src/timeline_scene/mod.rs`
- `crates/autolight-qt/src/app_controller/timeline_controller.rs`
- `docs/NOW.md`

## Verification

Passed:

```bash
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_scene_snapshot_uses_cache_backed_waveform_ref_without_inline_payload
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked native_timeline_scene_clips_scrolled_rows_below_ruler
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_app_runtime_refreshes_native_timeline_scene_snapshot_after_model_changes
cargo fmt --all -- --check
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke
git diff --check
```

## Handoff

PR #14 follow-through is complete for the latest fetched bot feedback. The branch was pushed, all three new Codex bot threads were replied to and resolved, and the post-reply thread refresh reported zero unresolved review threads.

Next: monitor PR #14 for any new bot feedback on the latest head. Current external status caveat: diffray failed with an infrastructure-style "Review task failed. Please try again" message and no annotations; DeepSource Rust/Python passed, CodeRabbit skipped review, and cubic is neutral due the plan review limit.
