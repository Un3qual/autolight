# Autolight NOW

Updated: 2026-06-04

## Active Batch: Rust Port PR Bot Follow-through

**Status:** complete

**Goal:** Pull new and unresolved PR #13 code-review-bot comments, including duplicate and outside-diff threads, fix valid Rust/QML issues, push the branch, refresh review state, and reply where needed.

## Batch Plan

1. Refresh PR #13 review-thread state with thread-aware GitHub GraphQL, not flat comments only.
2. Triage current unresolved bot comments plus duplicate/outside-diff review summaries.
3. Fix valid runtime issues: pending async polling, stale async merge, reset cancellation, Save As cache artifacts, generated-parent audio artifacts, stale snap guides, and small Rust nits.
4. Leave already-fixed stale QML comments and duplicate Windows-save feedback with evidence.
5. Run workspace tests, clippy, formatting, smoke, and `git diff --check`.
6. Push and refresh PR thread state again, then reply in-thread where needed.

## Completion Update

- 2026-06-04 follow-up: The pushed `263134a` commit resolved GitHub-visible review threads, but the current DeepSource status still reported Rust failed with no GitHub annotations for its outside-diff findings.
- Changes made: audited the current Rust code for the recurring DeepSource `RS-W1079` empty-`new()` pattern and replaced the remaining local `JobRegistry::new()` / `EditHistory::new()` call sites with `Default`-based construction.
- Verification:
  - `cargo fmt --all -- --check`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked`: passed, 84 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: passed.
- 2026-06-04: Addressed the latest PR #13 bot-review findings from CodeRabbit, Codex, and the pasted DeepSource/Diffray Rust report.
- Root cause: the post-push async/cache hardening still left edge cases in artifact job preflight, worker-panic terminalization, progress polling dirty-state handling, demo cache materialization, Windows save replacement, cache revalidation, QML temp extraction/path guards, WAV frame validation, and small Rust idioms flagged by DeepSource.
- Changes made: artifact-producing jobs now require a project/demo cache directory before detaching a worker; progress polling marks non-history dirty without clearing undo/redo; worker join failures terminalize the run/track; demo cache refs are real hash-backed artifacts; Windows save replacement uses replace-existing semantics without deleting the old file first; cache validity refresh can restore recovered entries; QML asset extraction uses a unique directory and exclusive file creation; WAV inspection rejects partial frames; QML timeline guards handle missing/non-array models; runnable transform IDs are shared between the model and job registry; waveform LOD selection is viewport-based; UNC file URLs and relink hints are sanitized; DeepSource clone/map/test-path nits are fixed.
- Comments triaged without code changes: Diffray's broader summary risks around deeper controller thinning, parity audits, telemetry, malformed project recovery, and large-timeline performance are roadmap-scale follow-ups, not new blocking defects for this PR cleanup.
- Next batch: none for this PR cleanup after the pushed commit is refreshed on GitHub and fixed bot threads are replied/resolved.
- Verification:
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-jobs --offline`: passed after offline lock metadata update, 25 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked`: first failed while updating stale cache-refresh test assumptions; passed with 84 tests after validating real missing/recovered artifacts and adding artifact-preflight/progress-history regressions.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-app --locked`: passed, 5 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`: passed, including 22 `autolight-analysis`, 5 `autolight-app`, 44 `autolight-core`, 25 `autolight-jobs`, and 84 `autolight-qt` tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: passed.
  - `cargo fmt --all -- --check`: passed.
  - `git diff --check`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`: passed and printed `Rust smoke loaded UI/Main.qml with Autolight.Qt AppController`; Qt emitted non-fatal host audio/font warnings.

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
