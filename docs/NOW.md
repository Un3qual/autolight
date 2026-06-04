# Autolight NOW

Updated: 2026-06-04

## Active Batch: None

**Status:** complete

**Goal:** The Rust/CXX-Qt runtime cutover is complete for the current roadmap. No unblocked Rust-port batch remains in `docs/ROADMAP.md`.

## Current State

The Rust/CXX-Qt app is now the primary runtime path. The Python/PySide app remains checked in as the reference implementation and parity baseline.

Default run path:

```bash
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo run -p autolight-app
```

Reference Python run path:

```bash
uv run python main.py
```

## Completion Update

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
