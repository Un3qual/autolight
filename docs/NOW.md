# Autolight NOW

Updated: 2026-06-05

## Active Batch: Native Timeline Scene

**Status:** complete

**Goal:** Replace the reactive QML timeline lane/ruler/rendering stack with one native CXX-Qt timeline scene item so playback follow, scroll, and zoom are transform-only on the hot path and waveform/analysis detail rebuilds never block UI motion.

## Planned Follow-Up Batch: Native Timeline Risk Hardening

**Status:** in progress, Task 2 complete

**Goal:** Close the remaining diffray risk areas that are not already fixed: native scene profiling, scene-item file decomposition, explicit waveform memory budgeting, legacy/reference path fencing, and the real-window macOS gesture/playback gate.

**Plan:** `docs/superpowers/plans/2026-06-05-autolight-timeline-risk-hardening.md`

**Task 1:** Completed 2026-06-05. Added the native scene snapshot lifecycle regression
`native_timeline_viewport_changes_do_not_reparse_scene_snapshot` and the manual macOS
hardening gate at `docs/manual-testing/native-timeline-risk-hardening.md`.

**Task 1 Verification:** `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked native_timeline_viewport_changes_do_not_reparse_scene_snapshot` passed with 1 test.

**Task 2:** Completed 2026-06-05. Added QML-readable native scene timing counters
on `TimelineSceneItem` for snapshot parse count, worst snapshot parse time, worst scene graph
update time, and text texture creation count. Counter updates use `QElapsedTimer`, preserve the
snapshot parse hot-path boundary from Task 1, and count text textures only on new texture creation.

**Task 2 Verification:** The focused regression
`timeline_scene_item_exposes_native_timing_counters_for_manual_profiling` first failed on the missing
counter `Q_PROPERTY` contract, then passed after implementation. Final focused checks passed:
`QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_scene_item_exposes_native_timing_counters_for_manual_profiling`;
`QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_scene_item_draws_ruler_markers_selection_and_playhead_without_qml_repeaters`;
`cargo fmt --all -- --check`; and `git diff --check`.

**Task 2 Threading Follow-Up:** Completed 2026-06-05. The render-thread-updated counters now use
atomic storage and queue `scenePerfCountersChanged()` back to the QObject thread instead of emitting
from `updatePaintNode`.

**Next Task:** Split `timeline_scene_item.cpp` by responsibility.

## Batch Plan

1. Add failing architecture regressions proving the Rust runtime no longer instantiates QML timeline lanes/ruler/navigation/marker repeaters or JSON geometry render paths.
2. Add pure Rust timeline scene snapshot types that carry static track, marker, selection, and artifact-ref data without waveform/analysis geometry.
3. Add a native `TimelineSceneItem` registered in `Autolight.Qt` and compiled through the Rust app build.
4. Replace the active Rust `TimelineView.qml` path with `TimelineSceneItem`; QML keeps only layout/chrome and high-level toolbar controls.
5. Move ruler, playhead, markers, selected-track styling, and timeline gestures into the native item.
6. Replace waveform/analysis JSON geometry calls with double-buffered native tile data prepared off the UI/render hot path.
7. Retire old QML timeline primitives from the Rust bundle, add performance counters, run automated checks, and finish with a real-window playback/zoom/trackpad gate.

## Target Paths

- `docs/NOW.md`
- `crates/autolight-qt/build.rs`
- `crates/autolight-qt/src/lib.rs`
- `crates/autolight-qt/src/timeline_model.rs`
- `crates/autolight-qt/src/app_controller/mod.rs`
- `crates/autolight-qt/src/app_controller/timeline_controller.rs`
- `crates/autolight-qt/src/app_controller/timeline_viewport.rs`
- `crates/autolight-qt/src/app_controller/tests.rs`
- `crates/autolight-qt/src/timeline_scene/`
- `crates/autolight-qt/src/timeline_renderer/waveform.rs`
- `crates/autolight-qt/src/timeline_renderer/cache.rs`
- `UI/Main.qml`
- `UI/components/TimelineView.qml`
- `UI/components/TimelineLane.qml`
- `UI/components/TimelineRuler.qml`
- `UI/components/TimelineNavigationSurface.qml`
- `UI/components/MarkerBlock.qml`
- `crates/autolight-app/src/main.rs`
- `tests/test_app_controller.py`

## Implementation Contract

- Forward runtime work targets the Rust/CXX-Qt app only; Python remains reference-only.
- Do not tune QML animation, tile width, JSON caps, or repeater shapes as a substitute for replacing the timeline surface.
- No JSON geometry payloads in playback-follow, scroll, or zoom hot paths.
- No QML `Repeater` for moving timeline contents, ruler ticks, playhead, markers, waveform columns, or analysis strips in the active Rust path.
- Playback follow and scroll must be transform-only per frame.
- Zoom drag must update a transient native viewport immediately; expensive waveform/analysis detail swaps must be async or deferred until prepared.
- Rust owns scene snapshots, viewport policy, cache validation, and waveform/analysis projection; Qt scene graph owns retained geometry and prepared tile swaps.
- Keep pure Rust logic testable without Qt types and avoid production `unwrap()`/`expect()` in rendering/cache paths.
- Detailed implementation plan: `docs/superpowers/plans/2026-06-05-autolight-native-timeline-scene.md`.

## Verification

Run focused tests while implementing, then run:

```bash
cargo fmt --all -- --check
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D clippy::perf
QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke
git diff --check
```

Manual macOS playback/trackpad pass after automated checks:

- demo project playback follow in `Band` and `Center` modes has no visible freezes;
- zoom slider can move while playing without stalls;
- two-finger horizontal swipe pans timeline smoothly;
- pinch zooms around pointer/playhead smoothly;
- ruler drag scrubs;
- playhead stays visible and waveform detail may refine after zoom without blocking motion.

## Completion Update

- 2026-06-05 PR #14 review-bot follow-up: Pulled the unresolved/new bot feedback for the stacked native timeline PR, including duplicate/outside-range threads from DeepSource, Codex, CodeAnt, Greptile, CodeRabbit, cubic, and diffray summaries. Fixed the actionable Rust/QML issues: DeepSource boolean-assert and `map_or` nits; stale tile fallback promotion; native vertical row scrolling plus visible-track-range refresh after controller/model binding changes; lane-local native scrub math; marker click selection; continuous ruler drag scrub; lane-click seek; guarded Rust-only timeline controls for Python reference mode; legacy lane scrub forwarding; and native scene rendering for analysis previews instead of carrying analysis refs that could never draw. Greptile's device-pixel-ratio note on `renderTimelineWaveform` was analyzed as a legacy/reference-path concern rather than active native-scene hot-path code; the active Rust timeline now uses `TimelineSceneItem` and no longer calls the old QML waveform geometry invokable. Diffray's risk areas remain mostly manual/perf gates already called out below: real-window macOS gesture feel, physical playback-follow smoothness, long-session memory, and extreme-zoom artifacts.
- Verification: targeted regressions passed: `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_native_timeline_keeps_vertical_scroll_and_visible_track_range_current`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_timeline_native_scrub_omits_label_width_and_reference_controls_are_guarded`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_timeline_`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_scene_`; and `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_tiles_prepare_next_zoom_bucket_without_replacing_active_tile`. Final checks passed: `cargo fmt --all -- --check`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked` with 150 tests; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D clippy::perf`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`; `QT_QPA_PLATFORM=offscreen uv run python main.py --smoke`; and `git diff --check`. The smokes emitted only the known host audio/font warnings; the first Python smoke attempt was sandbox-blocked on uv cache access and passed when rerun with cache access.
- Next batch: after the PR refresh, resolve or reply to fixed bot threads. If the refreshed bots surface no new actionable code issues, the remaining work is the already-known real-window macOS trackpad/playback feel gate rather than more QML geometry churn.

- 2026-06-05 PR #14 second bot-refresh follow-up: CodeRabbit surfaced another batch after the first push. Fixed the valid issues: restored timeline viewport state now preserves raw scroll until duration is known, clamps through `TimelineViewport` when duration is available, clears transient user-navigation suppression, and refreshes playhead offscreen state; zero `waveform_payload.sample_rate` now falls back to the source audio rate; legacy analysis geometry now paints through a bounded Canvas renderer; legacy navigation quiet timers stop/gate themselves when pinch or scrub gestures take over; `TimelineGeometryItem.emptyReason` has a dedicated notify signal and is updated before `geometryJsonChanged`; pending native tiles merge into the active tile map instead of replacing unrelated active keys; and the review nits for analysis-width and native wheel constants are named. The still-unfixed Greptile DPR thread remains intentionally not code-changed for the same reason as above: it points at the old `renderTimelineWaveform` invokable, not the active native scene item.
- Verification: focused regressions passed: `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked controller_restore_timeline_view_clamps_scroll_and_clears_navigation_state`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_rows_treat_zero_waveform_sample_rate_as_missing`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_tiles_prepare_next_zoom_bucket_without_replacing_active_tile`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_timeline_native_scrub_omits_label_width_and_reference_controls_are_guarded`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_geometry_item_empty_reason_has_dedicated_notify_signal`; and `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_scene_item_draws_ruler_markers_selection_and_playhead_without_qml_repeaters`. Final checks passed: `cargo fmt --all -- --check`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked` with 153 tests; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D clippy::perf`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`; `QT_QPA_PLATFORM=offscreen uv run python main.py --smoke`; and `git diff --check`. The smokes emitted only the known host audio/font warnings.
- 2026-06-05 hidden DeepSource follow-up: The DeepSource dashboard exposed two non-inline Rust `RS-W1070` findings on `TimelineTileBuffer` where `last_good_tiles` was assigned from `active_tiles.clone()`. Switched both assignments to `clone_from(&self.active_tiles)` so the existing buffer allocation can be reused and the analyzer no longer flags assignment-from-clone. Verification after this hidden fix passed: `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_tiles_prepare_next_zoom_bucket_without_replacing_active_tile`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked` with 153 tests; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D clippy::perf`; and `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`. The smoke emitted only the known host audio/font warnings.

- 2026-06-05 native timeline interaction follow-up: Fixed three regressions in the new native timeline scene. Root causes: `TimelineSceneItem` inverted the sign from the old QML navigation handler for horizontal trackpad deltas; the Rust scene snapshot carried `depth`/`expanded` but not `hasChildren`, and the native parser/renderer ignored tree metadata entirely; native wheel/zoom requests marked `timelineUserNavigationActive` through the controller but never ended that gesture, so playback follow stayed suppressed after touching the timeline. The native scene now uses natural horizontal/shift-scroll deltas, the scene snapshot includes `hasChildren`, the native item draws tree indentation guides and disclosure boxes with expansion toggles, and `TimelineView.qml` wraps native scroll/zoom gestures in a 220 ms quiet-period timer that ends user navigation so follow mode can resume.
- Verification: new regressions first failed, then passed: `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked native_timeline_trackpad_scroll_uses_natural_horizontal_direction`, `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_native_timeline_gestures_resume_follow_after_quiet_period`, `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_scene_snapshot_preserves_track_tree_metadata_for_native_rendering`, and `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_scene_item_draws_track_tree_indentation_and_disclosure`. Follow-up checks passed: `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_timeline_`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_scene_`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked controller_native_`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked follow`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked` with 148 tests; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`; and `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`. The smoke emitted only the known host audio/font warnings. A real-window macOS trackpad/playback pass is still needed to confirm physical gesture feel.

- 2026-06-05 import-track visibility follow-up: Fixed the remaining case where the track list/timeline scene stayed empty even after importing audio. Root cause: `AppRuntime.qml` parsed and replaced `trackRows` after model-changing native calls, but the native `TimelineSceneItem` consumed `timelineSceneSnapshotJson`, which was still exposed as a readonly passthrough to the native controller. In the live QML object graph that left the scene item on its initial empty snapshot after import/demo/model mutations. `AppRuntime.qml` now owns a mutable scene snapshot mirror, refreshes it during `reloadModels()`, and refreshes it after `select_track()` so selected-row styling updates with the native scene too. Added a controller regression proving imported audio appears in the native scene snapshot as the selected source track, plus a QML contract regression proving model reloads and selection refresh the scene snapshot mirror.
- Verification: `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked controller_import_audio_updates_native_timeline_scene_snapshot`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_app_runtime`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`; `cargo fmt --all`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked` with 144 tests; `cargo fmt --all -- --check`; and `git diff --check` all passed. The smoke emitted only the known host audio/font warnings.

- 2026-06-05 runtime-startup follow-up: Fixed the actual empty-timeline state shown in the screenshot. The visible `TRACKS` header, `Autolight Rust Smoke` window title, and `0:00 / 0:00` playback duration showed that the live QML runtime was still on the default empty smoke project, not that track rows were merely hidden by paint colors. Root cause: the Rust runtime demo load lived inside dynamically created `AppRuntime.qml`'s `Component.onCompleted`, while `Main.qml` owns the created runtime object and controller binding. Startup is now explicit in `Main.qml`: after the Rust runtime is owned and exposed as `root.controller`, `initializeRustRuntime()` calls `start_default_project()` and then refreshes the timeline viewport width. `AppRuntime.qml` now exposes `start_default_project()` and no longer relies on its own completion hook for default content.
- Verification: added `qml_main_initializes_rust_runtime_demo_after_runtime_is_owned`, which first failed on the missing explicit startup path and then passed. Final checks passed: `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_main_initializes_rust_runtime_demo_after_runtime_is_owned`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_`; `cargo fmt --all -- --check`; `git diff --check`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked` with 142 tests; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`; and `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`. The smoke still emitted only the known host audio/font warnings.

- 2026-06-05 row-visibility follow-up: Fixed the native timeline lane area reading as blank after the label column became visible. Root cause: rows with no visible waveform or marker content only had very dark full-lane fills and subtle separators, so the timeline body looked like one empty panel even though the scene snapshot had tracks. `TimelineSceneItem` now draws explicit per-row lane chrome: a gutter, brighter row panel, selected-row lane fill, top/bottom/left lane borders, a center guide, and per-row time grid fragments behind waveform/marker content.
- Verification: added `timeline_scene_item_draws_visible_lane_rows_even_without_waveform_or_markers`, which first failed on the missing native row-lane chrome and then passed. Final checks passed: `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_scene_item_draws_visible_lane_rows_even_without_waveform_or_markers`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_timeline_`; `cargo fmt --all -- --check`; `git diff --check`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked` with 141 tests; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`; and `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`. The smoke still emitted only the known host audio/font warnings.

- 2026-06-05 track-visibility follow-up: Fixed the native timeline scene regression where rows were present but visually unreadable because the new `TimelineSceneItem` had replaced the old QML delegates without drawing a track label column. The native scene now parses track names, types, and result state from the Rust snapshot, draws a fixed label/ruler column with selected-track emphasis, and offsets waveform, marker, playhead, scrub, and wheel-zoom math into the timeline lane instead of the full item width.
- Verification: added `timeline_scene_item_draws_track_labels_and_offsets_timeline_content`, which first failed on the missing native label-column contract and then passed. Final checks passed: `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_scene_item_draws_track_labels_and_offsets_timeline_content`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_timeline_`; `cargo fmt --all -- --check`; `git diff --check`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked` with 140 tests; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`; and `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`. The smoke still emitted only the known host audio/font warnings.

- 2026-06-05 native scene batch: Replaced the active Rust timeline surface with a native CXX-Qt `TimelineSceneItem`. Rust now builds a static scene snapshot for tracks, markers, selection, artifact refs, and bounded waveform preview data; viewport scroll/zoom/playback changes keep that snapshot stable and move through native item properties. The native item draws ruler ticks, row backgrounds, selected-track styling, markers, waveform previews, and the playhead, and handles track clicks, ruler scrubbing, wheel/trackpad horizontal pan, and modifier-wheel zoom. The Rust app bundle no longer embeds the old QML timeline lane/ruler/navigation/marker primitives for the active path.
- Implementation cleanup: Added pure Rust `timeline_scene` model, tile-buffer, and perf-counter modules; registered and compiled `timeline_scene_item.h/.cpp` through the CXX-Qt build; updated `Main.qml`/`TimelineView.qml` to load the native scene item for Rust and a legacy loader for the Python reference path; kept legacy Python QML loadable by removing the Rust module dependency from `TimelineLane.qml`, adding local inert waveform/analysis placeholder items, and guarding legacy ruler bindings until a controller is injected. Fixed the one clippy structural issue found during verification by moving the `BTreeMap` import before the test module in `timeline_scene/tiles.rs`.
- Verification: red architecture tests first failed on the old QML timeline path, then passed after the native scene implementation. Final checks passed: `cargo fmt --all -- --check`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked` with 139 tests; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D clippy::perf`; `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`; `uv run --with pytest python -m pytest tests/test_app_controller.py tests/test_waveform_summary.py` with 173 tests; `QT_QPA_PLATFORM=offscreen uv run python main.py --smoke`; and `git diff --check`. The Rust and Python smokes emitted only host audio/font warnings.
- Manual note: the real-window macOS feel gate was not run in this environment. The next batch should be a physical playback/trackpad pass for two-finger horizontal pan, pinch zoom, ruler drag scrub, zoom-slider movement while playing, and playhead-follow smoothness; any remaining issues should be fixed against the native scene item rather than by reviving QML geometry paths.

- 2026-06-05 follow-up 2: Reduced the remaining playback-follow stalls after manual testing showed longer freezes at each stutter. Root cause: the previous overscanned tile fix reduced how often geometry regenerated, but each boundary update now serialized and parsed a much larger waveform/analysis JSON payload. The follow animation also used short ease-out jumps toward coarse media-player position ticks, which could move quickly and then pause between ticks. `AppRuntime.qml` now uses `SmoothedAnimation` with playback-speed velocity for follow scroll, and the Rust waveform/analysis projection paths cap wide retained-tile output to 2,048 geometry columns while striding across the full tile width instead of emitting one JSON rect per pixel. This keeps boundary updates bounded without rendering only the left side of the tile.
- Verification: new regressions first failed on the old code, then passed after the fix: `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_app_runtime_smooths_playback_follow_scroll_only_during_follow`, `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked waveform_projection_caps_wide_tiles_and_spans_full_width`, and `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked analysis_projection_caps_wide_tiles_and_spans_full_width`. `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_timeline_` passed. `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_renderer::` passed. `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked` passed with 128 tests. `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked` passed. `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` passed. `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D clippy::perf` passed. `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke` passed and loaded `UI/Main.qml` after non-fatal host audio/font warnings.
- Manual note: this still needs a real-window playback pass. If zoomed-in follow remains visibly bad after this bounded-geometry fix, the remaining architectural issue is that tile generation is still synchronous at all; the next step should be a double-buffered or asynchronous scene-graph tile cache instead of more QML binding tuning.

- 2026-06-05 follow-up: Fixed zoomed-in playback-follow choppiness after the retained renderer landed. Root cause: `TimelineLane.qml` still keyed `renderTimelineWaveform` and `renderTimelineAnalysis` directly off live `timelineScrollSeconds`, so the smooth follow animation forced Rust geometry JSON generation and C++ JSON parsing on every scroll frame. The lane now renders a three-viewport-wide retained tile keyed by a quantized tile start and translates `TimelineWaveformItem` / `TimelineAnalysisItem` with a cheap `x` offset while playback follow animates. Geometry is regenerated only when the scroll crosses a tile boundary, not on every animation tick.
- Verification: `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_timeline_scroll_translates_retained_renderer_without_per_frame_geometry_regeneration` first failed on the old direct-scroll render bindings, then passed after the tile translation fix. `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_timeline_` passed. `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked` passed with 126 tests. `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-app --locked` passed with 5 tests. `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked` passed. `cargo fmt --all -- --check` passed. `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` passed. `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D clippy::perf` passed. `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke` passed and loaded `UI/Main.qml` after non-fatal host audio/font warnings. `git diff --check` passed.
- Manual note: this environment still cannot prove real macOS trackpad/playback feel. The expected visible change is that zoomed-in follow no longer does per-frame render-query work; a real-window pass should check that the playhead-follow scroll now feels smooth while playing.

- 2026-06-05: Completed the retained waveform renderer batch. Added a Qt Quick scene-graph item pair (`TimelineWaveformItem` / `TimelineAnalysisItem`) registered through the Rust/CXX-Qt app build, replaced the active QML `Canvas` waveform/analysis path with retained items, and removed `WaveformStrip.qml` / `AnalysisStrip.qml` from the Rust app bundle. Timeline rows now emit cache-backed waveform and analysis refs instead of serializing waveform levels or visible analysis samples. Rust now owns waveform payload parsing, peak/RMS LOD projection, artifact digest validation, viewport-bounded geometry, and a controller-owned retained waveform cache.
- Review fixes: 5.5 xhigh subagents caught cache provenance digest bypass, unbounded waveform cache lifetime, high-zoom point rendering from envelope payloads, top-level visible-analysis array handling, harmonic-color per-sample color loss, and stale Python QML assertions for deleted Canvas files. Fixed all of those: render cache population reads validated artifact bytes; project replacement/cache validation prunes the retained cache; high zoom keeps peak/RMS envelope columns; visible energy/harmonic arrays render through the retained analysis path; harmonic color emits grouped color bands with left-edge context; Python QML structure tests now assert the retained renderer.
- Verification: `cargo fmt --all -- --check` passed. `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-analysis --locked` passed, 22 tests. `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-app --locked` passed, 5 tests. `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked` passed, 125 tests. `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked` passed. `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` passed. `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D clippy::perf` passed. `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke` passed and loaded `UI/Main.qml` after non-fatal host audio/font warnings. `uv run --with pytest python -m pytest` on the 9 changed QML structure tests passed. `git diff --check` passed.
- Manual note: the automated smoke verifies QML loading and scene-graph type registration, but this environment did not run a real macOS trackpad pass. Next batch should be a real-window feel pass for two-finger horizontal pan, pinch zoom, ruler scrub, playback follow smoothness, and visual density.

- 2026-06-05: Researched the remaining timeline choppiness and raised waveform detail. Root cause: the timeline still treats horizontal motion as data-dependent repaint work. Playback follow, pan, and zoom mutate `timelineScrollSeconds`; every visible `WaveformStrip` and analysis strip listens to that value and repaints a QML `Canvas`. Qt's own rendering guidance says frequent large Canvas updates are expensive because updates become texture uploads, while the scene graph is designed to retain geometry and transform it between frames. The current QML Canvas approach also allocated JS objects during every paint pass, which can contribute to GC interruptions during scroll/animation. Short-term mitigation: `WaveformStrip.qml` and `AnalysisStrip.qml` now use `Canvas.Threaded` and draw visible spans directly without per-frame allocation arrays. Waveform generation now defaults to a 4,096-bucket base and can build up to 32,768-bucket LODs instead of the previous 512/4,096 limits.
- Verification: `cargo test -p autolight-analysis --locked waveform_level_counts_are_bounded_by_maximum_and_frame_count` and `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked waveform_bucket_param` first failed on the old 4,096 cap and 512 default, then passed after the LOD bump. `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_` passed and guards threaded/no-allocation Canvas paint paths. `cargo test -p autolight-analysis --locked` passed with 22 tests. `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked` passed with 106 tests. `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy -p autolight-analysis -p autolight-qt --all-targets --all-features --locked -- -D warnings` passed. `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke` passed after non-fatal host audio/font warnings. `cargo fmt --all -- --check` and `git diff --check` passed.
- Architecture handoff: the real fix is a retained scene-graph timeline renderer, likely a Rust/C++ `QQuickItem` with `QSGGeometryNode`/texture-backed waveform layers and a model boundary that does not serialize full waveform levels through `timelineRowsJson` on track refresh. The current fix improves fidelity and reduces QML paint cost but does not eliminate Canvas texture upload work during scroll/follow.

- 2026-06-04: Removed the Qt 6 deprecated implicit signal-parameter warning from `AppRuntime.qml`. Root cause: `MediaPlayer.onPositionChanged` used a block handler that referenced the injected `position` parameter; changed it to `onPositionChanged: function(position)` and added a QML regression assertion for the formal handler shape.
- Verification: `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_app_runtime_uses_controller_models_and_actions` first failed on the old handler shape, then passed after the fix. `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke` passed and no longer emitted the `AppRuntime.qml:59` deprecated parameter warning; it still emitted the non-fatal host audio/font warnings.

- 2026-06-04: Hotfixed choppy playback-follow scrolling at high zoom. Root cause: native follow mode correctly updated the viewport target on each `MediaPlayer.position` tick, but QML mirrored that target directly into `timelineScrollSeconds`, so each coarse playback tick became a visible timeline jump when `pixels_per_second` was high. Added a guarded `Behavior on timelineScrollSeconds` in `AppRuntime.qml` so only active playback follow animates the mirrored scroll value; manual timeline navigation stays immediate because the behavior is disabled while `timelineUserNavigationActive` is true or follow mode is off.
- Verification: `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_app_runtime_smooths_playback_follow_scroll_only_during_follow` first failed on the missing smooth-follow runtime property, then failed again on the missing `QtQuick` import after smoke exposed `Behavior is not a type`; passed after adding the guarded scroll behavior and import. `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke` passed and loaded `UI/Main.qml` after non-fatal host audio/font warnings.

- 2026-06-04: Hotfixed Qt Multimedia seeking after manual runtime testing hit `AppRuntime.qml:246: TypeError: Property 'seek' of object QQuickMediaPlayer is not a function`. Root cause: Qt 6.11's `MediaPlayer` exposes writable `position` through `setPosition`, not a QML `seek()` method. Added `AppRuntime.seekMediaPlayerToSeconds()` and routed stop, explicit playback seek, and timeline scrub through `mediaPlayer.position = positionMs`; added a QML regression assertion that `mediaPlayer.seek(` is not used. Verified against `/opt/homebrew/Cellar/qtmultimedia/6.11.1/share/qt/qml/QtMultimedia/plugins.qmltypes`.
- Verification: `cargo fmt --all -- --check` passed; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_app_runtime_uses_controller_models_and_actions` passed; `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke` passed and loaded `UI/Main.qml` after non-fatal host audio/font warnings; `git diff --check` passed; `rg -n "mediaPlayer\\.seek|\\.seek\\(" UI/AppRuntime.qml UI crates/autolight-qt/src/app_controller/tests.rs` found only the negative test assertion.

- 2026-06-04: Completed timeline UI polish pass. Replaced the timeline zoom/follow/scroll strip with a compact dark editor control band using Qt Quick Basic controls so custom styling works on macOS; restyled playback transport and scrubber controls with the same visual language; improved track rows with custom disclosure controls, type/state badges, clearer metadata hierarchy, selected-row emphasis, and styled progress bars; improved the timeline ruler with a TIME label area, major/minor tick marks, a slimmer playhead stem/cap, and a subtle lane playhead halo.
- Verification: `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_timeline_polish_uses_editor_controls_badges_and_tick_marks` passed; `cargo fmt --all -- --check` passed; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked` passed with 103 tests; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked` passed; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` passed; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D clippy::perf` passed; `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke` passed and loaded `UI/Main.qml` with the Rust `AppController` after non-fatal host audio/font warnings; `git diff --check` passed.
- Manual note: this pass was verified through QML structure tests and offscreen smoke. It still needs an eyeball pass in the real app window for spacing, density, and trackpad feel.
- Next batch: continue UI polish on the top project/transform bars and marker inspector, then do a real-window manual pass.

- 2026-06-04: Promoted the native timeline navigation batch from `docs/superpowers/plans/2026-06-04-autolight-native-timeline-navigation.md`.
- 2026-06-04: Completed native Rust/CXX-Qt timeline navigation. Added a pure Rust viewport policy module for typed/clamped scroll, zoom anchors, scrub conversion, dynamic zoom bounds, follow modes, and playhead offscreen state. Added QML timeline navigation surfaces for horizontal wheel/trackpad pan, modifier-wheel zoom, pinch zoom, ruler click/drag scrub, and a draggable ruler playhead handle while keeping marker blocks above navigation surfaces. Reworked playback ticks to use a lightweight `syncPlaybackPosition` path instead of row/model reloads, added explicit `off`/`band`/`center` follow modes with manual-navigation suppression, widened/log-scaled zoom with fit support, constrained the ruler to the timeline rows instead of the inspector column, and kept waveform/analysis paint loops bounded to visible ranges. Embedded the new QML component in the Rust app bundle.
- Verification: `cargo fmt --all -- --check` passed; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked` passed with 102 tests; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked` passed; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` passed; `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D clippy::perf` passed; `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke` passed and loaded `UI/Main.qml` with the Rust `AppController` after non-fatal host audio/font warnings; `git diff --check` passed.
- Review: 5.5 xhigh subagent final review first caught wheel follow suppression, lane-click follow parity, stale playhead offscreen state after manual viewport changes, and missing embedded QML bundle coverage. Those were fixed and the same 5.5 xhigh reviewer approved the re-review with no blockers.
- Manual note: the automated smoke verifies QML loading, but a physical macOS trackpad gesture pass still needs to be done in the real app window for two-finger horizontal pan, pinch zoom, ruler drag scrub, and playback follow feel.
- Next batch: manual app feel pass and any polish from real trackpad testing; otherwise continue PR review-bot follow-through for the Rust port branch.

## Current State

The Rust/CXX-Qt app is now the primary runtime path. The Python/PySide app remains checked in as the reference implementation and parity baseline.

Older completed batches are retained below as completion history for this PR branch.

Default run path:

```bash
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo run -p autolight-app
```

Reference Python run path:

```bash
uv run python main.py
```

## Completion History

- 2026-06-04: Addressed the Rust-port review quality cleanup batch for playback viewport mirroring, marker undo payload size, timeline projection cost, cooperative job cancellation, and low-level Rust perf nits.
- Root cause: playback position ticks updated Rust playback state without refreshing QML viewport mirrors; Qt marker edits used full `ProjectDocument` undo snapshots even though core has marker/dependent snapshot commands; timeline row projection rebuilt marker/job/cache lookups repeatedly and still emitted a dead legacy `waveformSamples` JSON role; job cancellation only represented pending cancellation, not a token a running transform could observe; the WAV reader and fixed-interval runner still had clippy/perf anti-patterns.
- Changes made: `UI/AppRuntime.qml` now calls `reloadViewportState()` after media-player position ticks without rebuilding full models; marker add/update/bulk/move/resize/delete in the Rust Qt controller now record `MarkerSnapshotCommand` entries with affected-marker IDs plus recursive `DependentTrackSnapshot`s so undo avoids cloning/restoring the whole project; timeline projection now builds an indexed context for tracks, markers, latest jobs, cache entries, and audio assets, visible-track calculations use ordered projected track IDs without full row payloads, and the unused `waveformSamples` row field is no longer serialized; `LocalJobQueue` now exposes cloneable cancellation tokens that running transforms can observe cooperatively; empty WAV data chunks no longer call `read_exact` on a zero-length buffer and fixed-interval markers are generated with an index loop.
- Next batch: a true nonblocking transform worker path can be designed later around CXX-Qt async/thread ownership and progress delivery. This pass intentionally keeps the current synchronous controller ABI while making the queue cancellation primitive real and testable.
- Verification:
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked controller_marker_undo_preserves_unrelated_state_and_dependent_track_snapshot`: first failed because full-project undo restored an unrelated project rename; passed after marker/dependent snapshots.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_app_runtime_uses_controller_models_and_actions`: first failed because playback ticks did not refresh viewport mirrors; passed after adding `reloadViewportState()`.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_rows_json_omits_unused_legacy_waveform_samples_field`: first failed because `waveformSamples` was still emitted; passed after removing the field.
  - `cargo test -p autolight-jobs --locked jobs_running_transform_observes_external_cancellation_token`: first failed because `LocalJobQueue::cancellation_token` did not exist; passed after adding cooperative cancellation tokens.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked`: passed, 70 tests.
  - `cargo test -p autolight-jobs --locked`: passed, 22 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy -p autolight-qt --all-targets --all-features --locked -- -D warnings`: passed.
  - `cargo clippy -p autolight-jobs --all-targets --all-features --locked -- -D warnings`: passed.
  - `cargo fmt --all -- --check`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`: passed, including 22 `autolight-analysis` tests, 2 `autolight-app` tests, 44 `autolight-core` tests, 22 `autolight-jobs` tests, and 70 `autolight-qt` tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`: passed and printed `Rust smoke loaded UI/Main.qml with Autolight.Qt AppController`; Qt emitted non-fatal host audio/font warnings.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -W clippy::perf -W clippy::nursery`: ran as advisory; remaining warnings are broader style suggestions, while the targeted zero-byte read and floating-loop warnings were addressed.
  - `git diff --check`: passed.

- 2026-06-04: Deepened the Rust Qt controller split into an internal root/child controller hierarchy.
- Root cause: the first split removed helper/test weight from the generated bridge file, but `AppControllerState` still owned timeline viewport behavior and playback behavior as a flat set of methods and fields. Splitting those directly into QML-visible QObjects would churn the CXX-Qt ABI and QObject lifetime model, so the safer next step is internal child state with the existing flat qproperties kept as bridge mirrors.
- Changes made: added `TimelineControllerState` in `crates/autolight-qt/src/app_controller/timeline_controller.rs` for duration, zoom, scroll, visible-window, visible-track filtering, timeline refresh, snap filtering, and viewport persistence; added `PlaybackControllerState` in `crates/autolight-qt/src/app_controller/playback_controller.rs` for source path, position, duration, play/pause/stop/seek, volume, load/unload, and playback errors; kept `AppController` as the single QML-facing root with explicit `sync_timeline_bridge_state` and `sync_playback_bridge_state` mirror updates for CXX-Qt qproperties. The bridge shell is now 2,135 lines, with playback and timeline behavior moved into focused modules.
- Next batch: none. A later pass can split more root-owned domains, likely project I/O/open-save/import, editable marker commands, and job orchestration, but this pass intentionally avoids exposing multiple QObject controllers until the QML surface has a concrete need for them.
- Verification:
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked controller_runtime_refactor_has_internal_controller_hierarchy`: first failed because `timeline_controller.rs` did not exist; passed after adding timeline/playback child controller modules and root ownership fields.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked controller_`: first failed because restored timeline scroll was clamped before project duration was known; passed after preserving restored scroll until `refresh_view_state` computes duration.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_timeline_supports_wheel_scroll_and_anchor_zoom_without_model_reload`: passed.
  - `cargo fmt --all`: ran.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked`: passed, 68 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`: passed, including 22 `autolight-analysis` tests, 2 `autolight-app` tests, 44 `autolight-core` tests, 21 `autolight-jobs` tests, and 68 `autolight-qt` tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`: passed; Qt emitted host audio/font warnings, then loaded `UI/Main.qml` with the Rust `AppController`.
  - `cargo fmt --all -- --check`: passed.
  - `git diff --check`: passed.
  - `rg -n "RustAdapter|rustAdapter|createRustAdapter|rustAdapterSource" UI crates/autolight-qt/build.rs crates/autolight-qt/src --glob '!crates/autolight-qt/src/app_controller/tests.rs'`: passed, no matches.

- 2026-06-04: Split the Rust Qt controller bridge and renamed the QML runtime wrapper away from Rust-specific transitional naming.
- Root cause: `crates/autolight-qt/src/app_controller.rs` had grown past 5,100 lines and mixed the generated CXX-Qt bridge, runtime state, job runners, WAV parsing, project/cache/path helpers, marker display helpers, and tests in one compilation unit. That hurt code quality and made routine edits touch the same large file as the generated bridge. `UI/RustAdapter.qml` was also a stale transition-era name now that the Rust/CXX-Qt runtime is the forward app path.
- Changes made: moved the CXX-Qt bridge shell to `crates/autolight-qt/src/app_controller/mod.rs`, updated `crates/autolight-qt/build.rs` to generate from that file, and extracted helpers into `audio.rs`, `jobs.rs`, `markers.rs`, `project_io.rs`, `project_state.rs`, and `tests.rs`. Renamed `UI/RustAdapter.qml` to `UI/AppRuntime.qml`, renamed `createRustAdapter`/`rustAdapter` to `createAppRuntime`/`appRuntime`, and renamed the wrapped native object handle from `rustController` to `nativeController` while preserving the exported `AppController` QML type.
- Next batch: none. A deeper follow-up could move more `_state` methods into focused `impl AppControllerState` modules, but this pass keeps the QML ABI and generated bridge shell stable while removing the biggest helper/test weight from the bridge file.
- Verification:
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked controller_runtime_refactor_keeps_bridge_small_and_domain_modules_split`: first failed because `app_controller/mod.rs` did not exist; passed after the split.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_app_runtime_uses_controller_models_and_actions`: first failed because `UI/AppRuntime.qml` did not exist; passed after the rename and QML/test updates.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_timeline_supports_wheel_scroll_and_anchor_zoom_without_model_reload`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked controller_run_waveform_summary_completes_with_visible_waveform`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked`: passed, 67 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`: passed, including 22 `autolight-analysis` tests, 2 `autolight-app` tests, 44 `autolight-core` tests, 21 `autolight-jobs` tests, and 67 `autolight-qt` tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`: passed; Qt emitted host audio/font warnings, then loaded `UI/Main.qml` with the Rust `AppController`.
  - `cargo fmt --all -- --check`: passed.
  - `git diff --check`: passed.
  - `test -f UI/AppRuntime.qml && test ! -e UI/RustAdapter.qml`: passed.

- 2026-06-04: Improved Rust waveform quality across zoom levels with smooth LOD transitions.
- Root cause: the waveform artifact pipeline already produced versioned LOD levels, and the analysis crate already had zoom-aware level-selection logic, but Rust timeline rows flattened `waveform_payload` to the finest level before QML saw it. Zoom changes intentionally avoid row-model reloads for smooth viewport motion, so the canvas kept repainting the same detail level at every zoom. That made zoomed-out waveforms dense/noisy, zoomed-in waveforms blocky, and level changes unavailable during live zoom.
- Changes made: added camelCase `waveformLevels` to timeline rows while keeping `visibleWaveformSamples` as the legacy fallback; normalized payload levels into `{bucketCount, samples}` rows from coarse to fine; threaded `waveformLevels` through `TimelineView`, `TrackRow`, and `TimelineLane`; changed `WaveformStrip` to choose the target bucket count from `duration * pixelsPerSecond / 8`, compute bucket width from explicit `bucketCount`, repaint on level/duration/viewport changes, and crossfade adjacent LOD levels during zoom instead of popping or rebuilding rows.
- Next batch: none. If the next manual pass still finds visual artifacts, inspect actual generated payload level counts and consider raising `DEFAULT_WAVEFORM_BUCKETS` or adding a release-mode canvas performance pass; the current QML path can now consume all existing LOD levels without breaking the no-model-reload zoom behavior.
- Verification:
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_rows_expose_all_waveform_lod_levels_for_zoom_painting`: first failed because `waveformLevels` was absent from row JSON; passed after exposing normalized payload levels.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_waveform_strip_selects_and_blends_lod_levels_during_zoom`: first failed because QML did not pass levels to the canvas or select/blend LODs; passed after the QML threading and renderer changes.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_rows_expose_waveform_bucket_count`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_rows_prefer_full_waveform_payload_over_stale_visible_slice`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_rows_normalize_legacy_waveform_and_energy_payloads`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_timeline_supports_wheel_scroll_and_anchor_zoom_without_model_reload`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked`: passed, 66 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`: passed, including 22 `autolight-analysis` tests, 2 `autolight-app` tests, 44 `autolight-core` tests, 21 `autolight-jobs` tests, and 66 `autolight-qt` tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`: passed; Qt emitted host audio/font warnings, then loaded `UI/Main.qml` with the Rust `AppController`.
  - `cargo fmt --all -- --check`: passed.
  - `git diff --check`: passed.

- 2026-06-04: Fixed follow-up Rust waveform rendering coverage and shape issues from manual testing.
- Root cause: timeline rows were still deriving drawable waveform data from the legacy `visible_waveform` slice, which can be stale or viewport-sized, even when a full persisted `waveform_payload` is available. The QML waveform canvas also rendered each bucket as a one-pixel vertical stroke, so a full-song bucket payload looked sparse and visually distorted.
- Changes made: added versioned full-payload waveform data to the Rust demo, made timeline duration/sample/bucket extraction prefer `waveform_payload` levels before falling back to legacy visible-waveform fields, and changed `WaveformStrip` to render clipped bucket-width filled peak/RMS spans instead of isolated vertical strokes.
- Next batch: none. If another real imported file still truncates, inspect the saved waveform artifact payload and cache ref for that file; the timeline now consumes the full payload when it is present and valid.
- Verification:
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_rows_prefer_full_waveform_payload_over_stale_visible_slice`: first failed with the timeline row duration stuck at the stale one-second visible slice; passed after preferring `waveform_payload`.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_waveform_strip_renders_contiguous_bucket_spans`: first failed because `WaveformStrip` did not compute bucket spans or call `fillRect`; passed after the canvas renderer change.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_rows_normalize_legacy_waveform_and_energy_payloads`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked controller_run_waveform_summary_completes_with_visible_waveform`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked`: passed, 64 tests.
  - `cargo fmt --all -- --check`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`: passed, including 22 `autolight-analysis` tests, 2 `autolight-app` tests, 44 `autolight-core` tests, 21 `autolight-jobs` tests, and 64 `autolight-qt` tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`: passed; Qt emitted host audio/font warnings, then loaded `UI/Main.qml` with the Rust `AppController`.
  - `git diff --check`: passed.

- 2026-06-04: Fixed follow-up Rust timeline zoom/scroll and playhead visibility issues from manual testing.
- Root cause: after the smooth timeline pass removed full model reloads for viewport-only changes, the adapter still exposed timeline viewport values as direct wrapper bindings and did not explicitly mirror them after imperative zoom/scroll invocations; sliders and wheel handlers could call Rust successfully while the QML surface kept using stale wrapper values. The lane playhead was also gated on `playback.sourcePath`, so it stayed hidden until audio was loaded instead of showing on a populated timeline.
- Changes made: added `RustAdapter.reloadViewportState()` and writable viewport mirror properties for pixels-per-second, scroll seconds, visible seconds, and duration; `set_timeline_zoom`, `set_timeline_scroll_seconds`, and `set_timeline_visible_seconds` now update those mirrors without rebuilding track rows; the lane playhead is visible whenever the timeline has duration and the computed playhead x is in view.
- Next batch: none. Zoom/scroll now update the ruler, lanes, waveform/cue positioning, and sliders through mirrored viewport state while keeping the no-model-rebuild behavior.
- Verification:
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_timeline_supports_wheel_scroll_and_anchor_zoom_without_model_reload`: first failed on missing `reloadViewportState`; passed after adding viewport mirroring and the playhead visibility change.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_rust_adapter_uses_controller_models_and_actions`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked`: passed, 62 tests.
  - `cargo fmt --all -- --check`: passed after running `cargo fmt --all`.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`: passed; Qt emitted host audio/font warnings, then loaded `UI/Main.qml` with the Rust `AppController`.
  - `git diff --check`: passed.

- 2026-06-04: Fixed follow-up Rust timeline model/selection issues from manual testing.
- Root cause: the Rust adapter was copying parsed timeline rows through a QML `ListModel`, which is a poor fit for nested JS arrays such as `markerSpans` and `visibleWaveformSamples`; the inspector bypassed that path, so cue markers could appear there while disappearing from the timeline. Selection action state also depended on direct wrapper bindings after `select_track`, so the selected-row affordance and rerun buttons could lag the Rust controller state.
- Changes made: changed the timeline model bridge to keep parsed rows as plain JS objects and made `TimelineView` read row data directly by index, preserving marker and waveform arrays; mirrored selected-track id/action flags explicitly in `RustAdapter.reloadSelectionModels()` and refreshed rows on selection so selected-track borders/stripe and rerun eligibility update immediately; aligned the Rust demo with the Python reference by creating a temporary silent WAV for the demo source and using the demo temp directory for unsaved demo waveform artifact reruns, with unique temp dirs under parallel tests and cleanup via a guard.
- Next batch: none. Built-in demo waveform reruns now have real audio backing; imported/saved projects continue to use their project directory for artifacts.
- Verification:
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_rust_adapter_uses_controller_models_and_actions`: first failed on missing `trackRows`; passed after switching away from `trackModel.append(rows[i])`.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked controller_demo_waveform_can_be_selected_and_rerun`: first failed because the demo source was offline; passed after adding temp demo WAV/artifact backing.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_track_rows_show_track_selection_and_allow_lane_selection`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_timeline_supports_wheel_scroll_and_anchor_zoom_without_model_reload`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked`: passed, 62 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`: passed, including 62 `autolight-qt` tests.
  - `cargo fmt --all -- --check`: passed after running `cargo fmt --all`.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`: passed; Qt emitted host audio/font warnings, then loaded `UI/Main.qml` with the Rust `AppController`.
  - `git diff --check`: passed.

- 2026-06-04: Fixed follow-up Rust timeline track selection regression.
- Root cause: the smoother timeline pass added a high-z full-cover `MouseArea` in `TimelineView.qml` for wheel events; that sat above row delegates and could intercept clicks before `TrackRow`/`TimelineLane` selection handlers saw them.
- Changes made: replaced the full-cover wheel `MouseArea` with a `WheelHandler` on the `ListView`, preserving horizontal wheel/trackpad scroll and Ctrl/Meta-wheel anchor zoom without overlaying track click targets; tightened the QML regression so wheel handling cannot reintroduce a click-blocking `acceptedButtons: Qt.NoButton`/`z: 100` overlay.
- Next batch: none. If any click target still feels wrong in manual testing, inspect the specific child target next, but the global event-eater has been removed.
- Verification:
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_timeline_supports_wheel_scroll_and_anchor_zoom_without_model_reload`: first failed on the new `WheelHandler` expectation; passed after replacing the overlay `MouseArea`.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_track_rows_show_track_selection_and_allow_lane_selection`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`: passed; Qt emitted host audio/font warnings, then loaded `UI/Main.qml` with the Rust `AppController`.
  - `cargo fmt --all -- --check`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked`: passed, 61 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy -p autolight-qt --all-targets --all-features --locked -- -D warnings`: passed.
  - `git diff --check`: passed.

- 2026-06-04: Addressed four Rust-port usability issues from manual testing.
- Changes made: strengthened selected-track affordance with an accent stripe, selected background, and thicker selected borders across the track label and lane while preserving source/audio track selection logic; confirmed source/audio tracks were already selectable and covered the visibility issue instead of changing selection semantics; kept the import dialog WAV-focused because the Rust importer currently validates/decodes WAV content only, so advertising MP3/M4A/FLAC would route users to unsupported imports; added a real Rust `waveform.summary` runner for imported WAV sources, runtime-only `audio_path` resolution, cache-backed waveform artifact persistence, and visible waveform provenance for timeline rendering; made timeline scroll/zoom smoother by avoiding full QML model reloads for viewport-only changes and adding horizontal wheel/trackpad scrolling plus Ctrl/Meta-wheel anchor zoom.
- Next batch: none. Future non-WAV import support should start with a Rust decoder/prober change before broadening the file picker; deeper timeline smoothness can move toward a true horizontal Flickable/Qt model if needed after this incremental pass.
- Verification:
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_track_rows_show_track_selection_and_allow_lane_selection`: first failed because only `border.color` changed on selection; passed after adding explicit row selection affordances.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_timeline_supports_wheel_scroll_and_anchor_zoom_without_model_reload`: first failed because no wheel signals/handlers existed and viewport setters reloaded models; passed after the QML/adapter changes.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked controller_run_waveform_summary_completes_with_visible_waveform`: first failed with the waveform track in `Failed` state under the unsupported Rust transform runner; passed after adding runtime audio-path resolution and the WAV waveform runner.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_rust_adapter_uses_controller_models_and_actions`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked controller_run_unimplemented_builtin_transform_fails_without_empty_completion`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked controller_import_audio`: passed, 5 tests.
  - `cargo test -p autolight-analysis --locked waveform`: passed, 13 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked`: passed, 61 tests.
  - `cargo fmt --all -- --check`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`: passed, 150 tests across unit and doctest targets.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: passed after folding two new test `project_path` assignments into their default initializers.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`: passed; Qt emitted host audio/font warnings, then loaded `UI/Main.qml` with the Rust `AppController`.
  - `git diff --check`: passed.

- 2026-06-04: Addressed three new unresolved Codex bot review threads on PR #13 after commit `6dbd91f` and analyzed diffray's summary risk areas.
- Changes made: clamped snapped single-marker drags at the timeline start before Rust marker movement validation; restored the inspector's no-selection `Apply To Track` behavior by expanding the Rust controller bulk update to all selected-track markers while keeping the lower-level core bulk API empty-selection-safe; verified the cache-artifact overwrite comment does not need a code change because the queue writes same-directory temp artifacts and Rust `fs::rename` already replaces existing files on supported platforms. Diffray's risk list was triaged as either already mitigated by current code/tests or future work for the async/heavy-transform batch, with no PR-blocking code change recommended.
- Next batch: none. Push this final review cleanup, refresh GitHub review threads/checks, reply to the two fixed Codex threads and the cache-artifact evidence thread, and resolve the current threads if no new bot findings appear.
- Verification:
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked controller_bulk_update_without_marker_selection_updates_track_markers`: first failed with `left: 0 right: 2`; passed after expanding empty UI selection to selected-track marker IDs.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked controller_snapped_single_marker_move_clamps_at_timeline_start`: first failed because `move_selected_markers_state` returned `false`; passed after clamping no-candidate snapped times to `0.0`.
  - `cargo fmt --all`: ran.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked`: passed, 58 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`: passed, including 22 `autolight-analysis` tests, 2 `autolight-app` tests, 44 `autolight-core` tests, 21 `autolight-jobs` tests, and 58 `autolight-qt` tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: passed.
  - `cargo fmt --all -- --check`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`: passed and printed `Rust smoke loaded UI/Main.qml with Autolight.Qt AppController`; Qt printed non-fatal audio-device and missing `Sans Serif` font alias warnings.
  - `git diff --check`: passed.

- 2026-06-04: Addressed the follow-up DeepSource Rust dashboard report for PR #13 at commit range `f5342af...c9c936b`.
- Changes made: replaced remaining empty `new()` calls in test setup with `EditHistory::default()` and `TransformRegistry::default()` where `Default` is equivalent and already preserves the intended empty state.
- Next batch: none. Push this final DeepSource cleanup and refresh PR checks/review threads.
- Verification:
  - `rg -n 'EditHistory::new\(\)|TransformRegistry::new\(\)' crates/autolight-core/src/history.rs crates/autolight-core/src/transforms.rs`: passed, no matches.
  - `cargo test -p autolight-core --locked`: passed, 44 tests.
  - `cargo fmt --all -- --check`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: passed.
  - `git diff --check`: passed.

- 2026-06-04: Addressed four new unresolved Codex bot review threads that surfaced after the DeepSource cleanup push `2f0a7ed`.
- Changes made: decoded Windows `file:///C:/...` file-dialog URLs to local paths before Rust import/open/save path handling; built QML playback file URLs from native Windows paths by normalizing backslashes and preserving drive-letter colons; avoided rebuilding `trackModel` during `select_track` so marker press handling does not lose delegates mid-gesture; rejected opened project markers with non-finite or negative timestamps/durations during graph validation.
- Next batch: none. Push this follow-up, refresh GitHub review threads, reply/resolve the four current Codex bot comments, and report any newly surfaced bot comments or external check failures.
- Verification:
  - `cargo test -p autolight-core --locked graph_validate_rejects_invalid_marker_extents`: first failed because invalid marker extents were accepted; passed after the graph validation fix.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked controller_decodes_windows_file_urls_to_local_paths`: first failed with `/C:/Users/me/My Song.wav`; passed after stripping the URL-only drive slash.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_rust_adapter_uses_controller_models_and_actions`: first failed because Windows-safe playback URL handling and no-rebuild `select_track` were absent; passed after the QML adapter fix.
  - `cargo fmt --all`: ran after `cargo fmt --all -- --check` found formatting drift in the new Rust helper/import.
  - `cargo fmt --all -- --check`: passed after formatting.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`: passed, including 22 `autolight-analysis` tests, 2 `autolight-app` tests, 44 `autolight-core` tests, 21 `autolight-jobs` tests, and 56 `autolight-qt` tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`: passed and printed `Rust smoke loaded UI/Main.qml with Autolight.Qt AppController`; Qt printed non-fatal audio-device and missing `Sans Serif` font alias warnings.
  - `QT_QPA_PLATFORM=offscreen uv run python main.py --smoke`: first failed inside the sandbox because `uv` could not access `/Users/admin/.cache/uv`; rerun outside the sandbox passed. Qt multimedia channel warnings were non-fatal.
  - `git diff --check`: passed.

- 2026-06-04: Addressed the DeepSource Rust dashboard report for PR #13 at commit range `f5342af...e812b2a`.
- Changes made: applied the safe `map_or`/`map_or_else`, `Default::default`, `clone_from`, and boolean assertion cleanups across the reported Rust files; refactored the Rust WAV inspector into focused header/chunk/format/metadata helpers while preserving hashed reads, odd-byte padding, supported encoding checks, and existing importer error strings; rejected the naive `f64::clamp` rewrite as non-mechanical for `NaN` scroll origins and added a regression before preserving `NaN` as a zero scroll origin.
- Next batch: none. Push this DeepSource cleanup and refresh external PR checks; DeepSource/diffray dashboard state may lag GitHub-visible review threads.
- Verification:
  - `cargo test -p autolight-analysis --locked waveform_visible_samples_treats_nan_scroll_origin_as_zero`: first failed with a full-window `[0.0, ..., 9.0]` result under the naive `clamp` rewrite; passed after the explicit `NaN` guard.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked controller_import_audio`: passed, 5 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_rows`: passed, 7 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked controller_open_project_relinks_audio_from_project_directory`: passed.
  - `cargo test -p autolight-analysis --locked`: passed, 22 tests.
  - `cargo test -p autolight-core --locked`: passed, 43 tests.
  - `cargo test -p autolight-jobs --locked`: passed, 21 tests.
  - `cargo fmt --all -- --check`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`: passed, including 22 `autolight-analysis` tests, 2 `autolight-app` tests, 43 `autolight-core` tests, 21 `autolight-jobs` tests, and 55 `autolight-qt` tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`: passed and printed `Rust smoke loaded UI/Main.qml with Autolight.Qt AppController`; Qt printed non-fatal audio-device and missing `Sans Serif` font alias warnings.
  - `QT_QPA_PLATFORM=offscreen uv run python main.py --smoke`: first failed inside the sandbox because `uv` could not access `/Users/admin/.cache/uv`; rerun outside the sandbox passed. Qt multimedia channel warnings were non-fatal.
  - `git diff --check`: passed.

- 2026-06-04: Addressed four new unresolved Codex bot review threads on PR #13 after commit `41183f7`.
- Changes made: forwarded QML `MediaPlayer` position ticks to the Rust controller so playback-follow keeps the playhead visible during normal playback; rejected present-but-nonnumeric `markers.fixed_interval` `duration` and `interval` params instead of silently defaulting them; marked opened projects dirty only when load-time audio/job/cache refreshes actually mutate the loaded project document; kept the existing Rust `std::fs::rename` save path unchanged after verifying the Windows replace-existing claim was incorrect.
- Next batch: none. Push this follow-up, refresh GitHub review threads, reply/resolve the four current Codex bot comments, and report any newly surfaced bot comments or external check failures.
- Verification:
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked controller_fixed_interval_rejects_nonnumeric_params`: first failed with the track completing; passed after the fix.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked controller_open_project_keeps_persisted`: first failed with reopened stale projects dirty; passed after the fix.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_rust_adapter_uses_controller_models_and_actions`: first failed because `onPositionChanged` was absent; passed after the fix.
  - `cargo fmt --all`: ran.
  - `cargo fmt --all -- --check`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked`: passed, 55 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`: passed, including 21 `autolight-analysis` tests, 2 `autolight-app` tests, 43 `autolight-core` tests, 21 `autolight-jobs` tests, and 55 `autolight-qt` tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`: passed and printed `Rust smoke loaded UI/Main.qml with Autolight.Qt AppController`; Qt printed non-fatal audio-device and missing `Sans Serif` font alias warnings.
  - `QT_QPA_PLATFORM=offscreen uv run python main.py --smoke`: passed outside the sandbox because `uv` needs access to `/Users/admin/.cache/uv`. Qt multimedia channel warnings were non-fatal.
  - `git diff --check`: passed.

- 2026-06-04: Addressed 11 new unresolved bot review threads on PR #13 after commit `6c2da0d`.
- Changes made: filtered beat-grid fixture times to the project duration before applying `max_markers`; fixed waveform visible-window stop calculation for negative scroll origins; reported missing source audio asset IDs instead of track IDs during graph validation; prevented marker snapshot restore from recreating orphan markers for deleted tracks; made empty marker bulk-update selections no-op; synced the project-save parent directory after same-directory atomic replacement and kept Rust `fs::rename` replace-existing semantics; persisted submit-time job transform versions and stopped reloading mutable track params for submitted runs; blocked derive-editable from stale or failed source tracks; made Rust adapter creation fail fast instead of exposing a null controller; logged Rust adapter JSON parse failures; split the QML model reload helper for maintainability.
- Next batch: none. Push this follow-up, refresh GitHub review threads, reply/resolve the 11 addressed bot comments, and report any newly surfaced bot comments or external check failures.
- Verification:
  - `cargo fmt --all`: ran.
  - `cargo fmt --all -- --check`: passed.
  - `cargo test -p autolight-analysis --locked music`: passed, 9 tests.
  - `cargo test -p autolight-analysis --locked waveform`: passed, 12 tests.
  - `cargo test -p autolight-core --locked graph`: passed, 10 tests.
  - `cargo test -p autolight-core --locked history`: passed, 6 tests.
  - `cargo test -p autolight-core --locked markers`: passed, 12 tests.
  - `cargo test -p autolight-core --locked project`: passed, 12 tests.
  - `cargo test -p autolight-jobs --locked jobs`: passed, 21 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked controller_rejects_deriving_editable_track_from_stale_marker_track`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_rust_adapter_uses_controller_models_and_actions`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked`: passed, 52 tests.
  - `cargo test -p autolight-analysis --locked`: passed, 21 tests.
  - `cargo test -p autolight-core --locked`: passed, 43 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`: passed, including 21 `autolight-analysis` tests, 2 `autolight-app` tests, 43 `autolight-core` tests, 21 `autolight-jobs` tests, and 52 `autolight-qt` tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`: passed and printed `Rust smoke loaded UI/Main.qml with Autolight.Qt AppController`; Qt printed non-fatal audio-device and missing `Sans Serif` font alias warnings.
  - `QT_QPA_PLATFORM=offscreen uv run python main.py --smoke`: passed outside the sandbox because `uv` needs access to `/Users/admin/.cache/uv`. Qt multimedia channel warnings were non-fatal.
  - `git diff --check`: passed.

- 2026-06-04: Addressed six new unresolved Codex bot review threads on PR #13 after commit `489acff`.
- Changes made: removed the orphan active demo job and stopped marking the nonexistent demo WAV online; preserved undo/redo stacks after project saves while still resetting history on load/open/new/demo; kept the Rust playhead visible by scrolling the viewport on seek/nudge; included marker end times in Rust timeline duration; accepted PCM, IEEE-float, and WAVE_FORMAT_EXTENSIBLE WAV metadata in the Rust importer while still rejecting unknown encodings; added focused Rust controller/timeline regressions for each review finding.
- Next batch: none. Push this follow-up, refresh GitHub review threads, reply/resolve the six addressed Codex bot comments, and report any newly surfaced bot comments or external check failures.
- Verification:
  - `cargo fmt --all`: applied rustfmt changes.
  - `cargo fmt --all -- --check`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked`: passed, 51 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`: passed, including 20 `autolight-analysis` tests, 2 `autolight-app` tests, 39 `autolight-core` tests, 19 `autolight-jobs` tests, and 51 `autolight-qt` tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: passed.
  - `git diff --check`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`: passed and printed `Rust smoke loaded UI/Main.qml with Autolight.Qt AppController`; Qt printed non-fatal audio-device and missing `Sans Serif` font alias warnings.
  - `QT_QPA_PLATFORM=offscreen uv run python main.py --smoke`: passed outside the sandbox because `uv` needs access to `/Users/admin/.cache/uv`. Qt multimedia channel warnings were non-fatal.

- 2026-06-04: Addressed the latest unresolved Codex bot review threads on PR #13 after commit `32d9bd3`.
- Changes made: stopped rebuilding the Rust QML `trackModel` for viewport-only visible-row updates; relinked missing source WAVs from the opened project directory when metadata/fingerprint match; persisted produced artifact payload files before recording cache refs; made project saves use a same-directory temp file and atomic replace; validated project graph invariants during load/save; normalized Rust demo and legacy fixture waveform/energy/harmonic payloads into drawable QML sample fields; preserved stale pending generated tracks before starting queued jobs; corrected the Rust README workflow so it no longer advertises unsupported analysis transforms or background execution; removed explicit temp-file `drop` calls flagged by DeepSource by relying on lexical file scopes before rename.
- Next batch: none. Push this follow-up, refresh GitHub review threads, reply/resolve the addressed bot comments, and report any external check failures that remain outside GitHub-visible inline comments.
- Verification:
  - `cargo test -p autolight-core --locked project`: passed, 11 tests.
  - `cargo test -p autolight-jobs --locked jobs_preserve_stale_pending_track_before_runner_starts`: passed.
  - `cargo test -p autolight-jobs --locked jobs`: passed, 19 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked controller`: passed, 36 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_rows`: passed, 7 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_rust_adapter_uses_controller_models_and_actions`: passed.
  - `cargo fmt --all -- --check`: passed.
  - `git diff --check`: passed.
  - `rg -n "readonly property string rustAdapterSource|Qt\.createQmlObject\(" UI`: passed, no matches.
  - `rg -n "set_timeline_visible_track_range\(firstRow, rowCount\).*reloadModels" UI/RustAdapter.qml`: passed, no matches.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`: passed, including 20 `autolight-analysis` tests, 2 `autolight-app` tests, 39 `autolight-core` tests, 19 `autolight-jobs` tests, and 44 `autolight-qt` tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`: passed and printed `Rust smoke loaded UI/Main.qml with Autolight.Qt AppController`; Qt printed non-fatal audio-device and existing missing `Sans Serif` font alias warnings.
  - `QT_QPA_PLATFORM=offscreen uv run python main.py --smoke`: passed outside the sandbox because `uv` needs access to `/Users/admin/.cache/uv`. Qt multimedia channel warnings were non-fatal.

- 2026-06-04: Cleaned up the Rust QML adapter after review follow-through.
- Changes made: removed the embedded `rustAdapterSource` QML string from `UI/Main.qml`, moved the Rust/CXX-Qt adapter bridge into `UI/RustAdapter.qml`, kept the main UI on a file-backed synchronous adapter component, and extended the QML structure test to prevent regressions to string-generated adapter code.
- Next batch: none. PR review-bot refresh remains the next external gate after this push.
- Verification:
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_rust_adapter_uses_controller_models_and_actions`: passed.
  - `cargo fmt --all`: applied rustfmt changes.
  - `cargo fmt --all -- --check`: passed.
  - `git diff --check`: passed.
  - `rg -n "readonly property string rustAdapterSource|Qt\.createQmlObject\(" UI`: passed, no matches.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked`: passed, 39 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`: passed, including 20 `autolight-analysis` tests, 2 `autolight-app` tests, 37 `autolight-core` tests, 17 `autolight-jobs` tests, and 39 `autolight-qt` tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`: passed and printed `Rust smoke loaded UI/Main.qml with Autolight.Qt AppController`; Qt printed non-fatal audio-device and existing missing `Sans Serif` font alias warnings.
  - `QT_QPA_PLATFORM=offscreen uv run python main.py --smoke`: first failed inside the sandbox because `uv` could not access `/Users/admin/.cache/uv`; rerun outside the sandbox passed. Qt multimedia channel warnings were non-fatal.

- 2026-06-04: Addressed a fresh review-bot follow-through pass on PR #13 after commit `97c50c8`.
- Changes made: fixed job terminal failure/cancel descendant staleness, rejected negative produced marker timestamps, blocked job submission when inputs are not complete, refreshed/marked dirty when `run_next` errors after mutating project state, encoded local playback paths with URL-safe path segments, bounded Rust `markers.fixed_interval` generation to the Python reference marker cap, disabled reruns for incomplete inputs, invalidated snapshot undo after structural non-history project mutations, finalized persisted active jobs on open, validated cache artifact files during open, restored source/dependent tracks when offline audio comes back online, tolerated JSON roundtrip noise in audio duration comparisons, cleaned up DeepSource-flagged empty `JobRegistry::new()` test initializers, and fixed outside-diff Rust clippy findings that were not exposed as GitHub inline comments.
- Next batch: none. Final post-push GitHub refresh after replies/resolutions showed no unresolved bot review threads.
- Verification:
  - `cargo fmt --all`: applied rustfmt changes.
  - `cargo fmt --all -- --check`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: passed.
  - `cargo test -p autolight-jobs --locked jobs`: passed, 17 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked`: passed, 39 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`: passed, including 20 `autolight-analysis` tests, 2 `autolight-app` tests, 37 `autolight-core` tests, 17 `autolight-jobs` tests, and 39 `autolight-qt` tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`: passed and printed `Rust smoke loaded UI/Main.qml with Autolight.Qt AppController`; Qt printed non-fatal audio-device and existing missing `Sans Serif` font alias warnings.
  - `QT_QPA_PLATFORM=offscreen uv run python main.py --smoke`: first failed inside the sandbox because `uv` could not access `/Users/admin/.cache/uv`; rerun outside the sandbox passed. Qt multimedia channel warnings were non-fatal.
  - `git diff --check`: passed.
  - GitHub review-thread refresh after replies/resolutions: passed, `unresolved_bot_total: 0`.

- 2026-06-04: Addressed review-bot follow-through on PR #13.
- Changes made: fixed Rust-port review findings for persisted ID seeding, negative marker timestamps, job-run parameter persistence, job-run failure finalization, full cache digest IDs, graph child-state summary precomputation, checked waveform legacy integer fields, zero-count waveform buckets, beat-grid tempo estimation, boundary-only energy peaks, visible-row snapping, WAV import validation/fingerprinting, timeline artifact validation, QML visible track range wiring, WAV-only import filtering, unsupported transform failure handling, loaded-project audio status refresh, cache artifact file validation, QtMultimedia playback transport, root-load checks, and Qt exit-status propagation.
- Next batch: none. All current review-bot findings have code/test coverage or will be replied to with the pushed fix commit.
- Verification:
  - `cargo fmt --all`: applied rustfmt changes.
  - `cargo test -p autolight-core --locked graph`: passed, 8 tests.
  - `cargo test -p autolight-core --locked markers`: passed, 11 tests.
  - `cargo test -p autolight-core --locked cache`: passed, 6 tests.
  - `cargo test -p autolight-core --locked project`: passed, 9 tests.
  - `cargo test -p autolight-analysis --locked waveform`: passed, 11 tests.
  - `cargo test -p autolight-analysis --locked music`: passed, 9 tests.
  - `cargo test -p autolight-jobs --locked jobs`: passed, 13 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked`: passed, 32 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-app --locked`: passed, 2 tests.
  - `cargo fmt --all -- --check`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`: passed, including 20 `autolight-analysis` tests, 2 `autolight-app` tests, 37 `autolight-core` tests, 13 `autolight-jobs` tests, and 32 `autolight-qt` tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`: passed and printed `Rust smoke loaded UI/Main.qml with Autolight.Qt AppController`; Qt printed non-fatal audio-device and existing missing `Sans Serif` font alias warnings.
  - `QT_QPA_PLATFORM=offscreen uv run python main.py --smoke`: first failed inside the sandbox because `uv` could not access `/Users/admin/.cache/uv`; rerun outside the sandbox passed. Qt multimedia channel warnings were non-fatal.

- 2026-06-04: Completed `Rust Runtime Cutover`.
- Changes made: updated README to present the Rust/CXX-Qt binary as the primary app and Python/PySide as the reference app; kept the Rust and Python smoke/test commands explicit; folded in the runtime-cutover blocker for timeline viewport and snap parity by moving timeline zoom/scroll/visible-seconds state into the Rust controller, persisting zoom/scroll in `.autolight` UI state, routing `snap_timeline_time` through Rust, applying snap to single-marker moves, and guarding QML timeline list bindings against transient null values during Rust adapter reloads.
- Next batch: none. All `docs/ROADMAP.md` ready-queue items are complete.
- Verification:
  - `cargo fmt --all`: applied rustfmt changes.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked`: passed, 23 tests.
  - `cargo fmt --all -- --check`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`: passed, including 15 `autolight-analysis` tests, 35 `autolight-core` tests, 10 `autolight-jobs` tests, and 23 `autolight-qt` tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`: passed and printed `Rust smoke loaded UI/Main.qml with Autolight.Qt AppController`; the only warning was Qt's existing missing `Sans Serif` font alias warning.
  - `QT_QPA_PLATFORM=offscreen uv run python main.py --smoke`: first failed inside the sandbox because `uv` could not access `/Users/admin/.cache/uv`; rerun outside the sandbox passed. Qt multimedia channel warnings were non-fatal.
  - `git diff --check`: passed.

## Previous Batch

- 2026-06-04: Completed `Rust File And Playback Controller Actions`.
- Changes made: added Rust controller qproperties for project path, selected-track playability, playback source/position/duration/playing/error/volume state; wired Rust qinvokables for open/save/import, selected-track playback, loaded playback, pause/stop/seek/nudge, and volume; added minimal WAV probing and deterministic fingerprints for Rust audio import; connected the Rust QML adapter to file dialogs and playback controls while preserving the Python `appController` path; added focused Rust tests for audio import/playability, save/open roundtrip, playback state transitions, and QML adapter wiring.
- Verification:
  - `cargo fmt --all`: applied rustfmt changes.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked`: passed, 21 tests.
  - `cargo fmt --all -- --check`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`: passed, including 15 `autolight-analysis` tests, 35 `autolight-core` tests, 10 `autolight-jobs` tests, and 21 `autolight-qt` tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`: passed and printed `Rust smoke loaded UI/Main.qml with Autolight.Qt AppController`.
  - `QT_QPA_PLATFORM=offscreen uv run python main.py --smoke`: first failed inside the sandbox because `uv` could not access `/Users/admin/.cache/uv`; rerun outside the sandbox passed. Qt multimedia channel warnings were non-fatal.
  - `git diff --check`: passed.
