# Autolight NOW

Updated: 2026-06-05

## Active Batch: PR 14 Review-Bot Follow-Through

**Status:** complete

**Goal:** Pull current PR #14 commits, fetch unresolved/new bot feedback including stale duplicates, fix actionable native timeline findings, push, then reply to threads that need responses.

## Scope

- Pulled `codex/native-timeline-navigation` to PR #14 head `59a2cda`, including the merged PR #15 native timeline risk-hardening commits and Python runtime removal.
- Fetched PR #14 review threads, top-level bot comments, stale/outdated duplicate threads, and review summaries from GitHub.
- Fixed the still-actionable Codex review findings: fractional vertical track scrolling now applies sub-row offsets to row placement and hit testing; editable markers expose native body-drag and right-handle resize signals wired to the existing Rust marker edit commands; marker labels survive native snapshot parsing and render inside sufficiently wide cue blocks; ruler header/track-label clicks no longer seek; visible track ranges refresh when height or track count changes even if the clamped scroll value is unchanged; and the scroll slider now marks user navigation so playback follow does not immediately snap it back.
- Verified the stale marker lane-clipping duplicate was already fixed by `timelineLaneClippedRect` and the existing `native_timeline_marker_hit_tests_are_clipped_to_visible_lane` regression.
- One requested 5.5 xhigh QML subagent could not start because that model was at capacity; the QML comments were analyzed and fixed locally instead of using a lower model.

## Target Paths

- `UI/Main.qml`
- `UI/components/TimelineView.qml`
- `crates/autolight-qt/src/app_controller/tests.rs`
- `crates/autolight-qt/src/timeline_scene/scene_frame_builder.h`
- `crates/autolight-qt/src/timeline_scene/scene_frame_builder.cpp`
- `crates/autolight-qt/src/timeline_scene/scene_snapshot_parser.h`
- `crates/autolight-qt/src/timeline_scene/scene_snapshot_parser.cpp`
- `crates/autolight-qt/src/timeline_scene/timeline_scene_item.h`
- `crates/autolight-qt/src/timeline_scene/timeline_scene_item.cpp`
- `docs/NOW.md`

## Verification

Passed:

```bash
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked native_timeline_scene_
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_native_timeline_refreshes_visible_range_when_scroll_value_is_unchanged
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_main_marks_scroll_slider_changes_as_user_navigation
cargo fmt --all -- --check
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke
git diff --check
```

## Handoff

PR #14 follow-through is complete for the fetched bot feedback. The branch was pushed, all seven unresolved Codex bot threads were replied to and resolved, and the final thread refresh reported zero unresolved review threads.

Next: monitor PR #14 for any new bot feedback on the latest head. Current external status caveat: diffray's check failed with an infrastructure-style "Review task failed. Please try again" message and no annotations; DeepSource Rust/Python passed and CodeRabbit skipped review.
