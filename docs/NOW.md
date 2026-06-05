# Autolight NOW

Updated: 2026-06-05

## Active Batch: PR 15 Review-Bot Follow-Through

**Status:** complete

**Goal:** Pull current PR #15 bot feedback, fix actionable findings, analyze diffray risk areas, push, then reply to relevant review threads.

## Scope

- Fetched current PR #15 review threads, top-level bot comments, and check status from GitHub.
- Fixed the unresolved Codex thread by clipping marker hit tests to the same visible timeline lane geometry used for rendering.
- Addressed Greptile's AppRuntime mutability concern by restoring readonly public native state mirrors backed by refreshable internal state.
- Analyzed diffray's scene graph, counter notification, waveform budget, QML refresh, benchmark, and constants suggestions.
- Left non-defect diffray suggestions as evidence-backed non-actions: current tests cover the risky paths, and benchmark/user-facing policy docs are not required for this review fix.

## Target Paths

- `UI/AppRuntime.qml`
- `crates/autolight-qt/src/app_controller/tests.rs`
- `crates/autolight-qt/src/timeline_scene/scene_frame_builder.h`
- `crates/autolight-qt/src/timeline_scene/scene_frame_builder.cpp`
- `crates/autolight-qt/src/timeline_scene/timeline_scene_item.cpp`
- `docs/NOW.md`

## Verification

Passed:

```bash
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked native_timeline_marker_hit_tests_are_clipped_to_visible_lane
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_app_runtime_keeps_public_native_state_mirrors_readonly
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_app_runtime
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked native_timeline
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked waveform_max_bytes_param
cargo test -p autolight-analysis --locked waveform_level_counts
cargo test -p autolight-analysis --locked waveform_payload_build
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-app --locked embedded_qml_bundle_contains_runtime_and_components
QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_scene_item_exposes_native_timing_counters_for_manual_profiling
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked waveform_projection
cargo fmt --all -- --check
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
git diff --check
```

## Handoff

Next: commit, push `codex/native-timeline-risk-hardening`, refresh PR #15 bot state, and reply to any threads that remain actionable.
