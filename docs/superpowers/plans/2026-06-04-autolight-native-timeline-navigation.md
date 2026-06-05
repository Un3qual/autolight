# Autolight Native Timeline Navigation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` to implement this plan task-by-task. All implementation and review subagents must run as `gpt-5.5` with `xhigh` reasoning. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Rust/CXX-Qt Autolight timeline feel like a native audio/video editor timeline: direct trackpad pan, pinch zoom, playhead dragging, wider smooth zoom, and non-janky playback follow.

**Architecture:** Treat the timeline surface, not the sliders, as the primary navigation control. QML owns low-latency pointer, wheel, pinch, and drag events; Rust owns viewport policy, clamping, follow mode, persistence, and model refresh scheduling. Playback position rendering must stay cheap and independent from heavyweight model/window refreshes.

**Tech Stack:** Rust/CXX-Qt controller slots and tests, Qt Quick/QML `WheelHandler`, `PinchHandler`, `DragHandler` or scoped `MouseArea`, existing QML timeline components, offscreen smoke checks.

**Execution Note:** This plan is for the current Rust port branch/worktree that contains `crates/`. If executing from a stale planning worktree without `crates/`, switch to or create a worktree for `codex/rust-runtime-port`, read `docs/NOW.md`, then promote Task 1 below into `docs/NOW.md` before editing.

---

## Rust Best-Practices Contract

Implementation must follow the repo's Rust/CXX-Qt direction, with QML as a thin view/input layer and Rust as the source of truth for timeline behavior.

- Keep timeline math in Rust. QML may collect raw event data (`pixelDelta`, `angleDelta`, pinch scale, pointer x, lane width), but Rust must convert it into scroll, zoom, scrub, and follow-state updates.
- Split pure policy from bridge code. Add or reuse a focused Rust viewport-policy module, preferably `crates/autolight-qt/src/app_controller/timeline_viewport.rs`, and keep CXX-Qt slots in `timeline_controller.rs` as thin adapters.
- Use typed internal values instead of loose `f64` plumbing. CXX-Qt/QML slots can accept primitive `f64`, but the first Rust boundary should normalize into small `Copy` structs/enums such as `TimelinePixels`, `TimelineSeconds`, `PixelsPerSecond`, `TimelineViewport`, `TimelineInput`, and `TimelineFollowMode`.
- Validate all bridge inputs. Non-finite, negative, or impossible values from QML must be clamped or rejected without `panic!`, `unwrap()`, or `expect()` in production code.
- Use Rust enums for state. `TimelineFollowMode` must be an enum internally, not stringly typed state. If QML needs strings or ints, convert at the bridge boundary only.
- Prefer borrowing and slices. Avoid cloning track, marker, waveform, or model vectors in scroll/playback hot paths. Use `&[T]`, iterators, and borrowed model data where the code only reads.
- Avoid heap allocation and dynamic dispatch in hot paths. Do not introduce `Box<dyn Trait>`, `Arc<Mutex<_>>`, channels, or async work for viewport math unless the code already requires it and the performance audit justifies it.
- Use `Result` only for genuinely fallible operations. Pure clamp/zoom/scroll calculations should return concrete values. Bridge calls that can fail because of app state should return `Result` internally and map errors to the controller's existing user-visible error mechanism.
- Make tests living documentation. Unit tests should target the pure Rust viewport policy with descriptive names and one behavior per test where practical.
- Treat Clippy as a gate, not a suggestion. Fix warnings rather than adding `#[allow]`; use local `#[expect(clippy::...)]` only with a concrete reason.
- Add comments only for non-obvious platform/input behavior, for example macOS trackpad `pixelDelta` semantics or a deliberate follow-mode tradeoff. Do not add comments that restate code.

---

## Research Baseline

- Final Cut Pro supports two-finger pinch zoom, two-finger swipe scroll, two-finger double-tap zoom-to-fit, and continuous timeline scrolling with the playhead centered during playback.
- Premiere supports dragging the playhead in the time ruler, pinch-to-zoom, two-finger horizontal scrolling, and a combined horizontal zoom/scroll bar.
- Logic Pro anchors trackpad zoom on the pointer, except near the playhead where it anchors on the playhead.
- Audacity is the audio-specific reference for pinned playhead playback and deep horizontal zoom.
- Qt Quick has the needed primitives: `WheelHandler` for mouse/touchpad wheel events, `WheelEvent.pixelDelta` for high-resolution trackpad scrolling, `PinchHandler` for multiplicative zoom deltas, and `Flickable`/input handlers for native-feeling scroll behavior.

---

## Subagent Workflow

Before implementation, dispatch three parallel audit subagents, all `gpt-5.5 xhigh`:

- `QML Input Audit`: inspect `UI/Main.qml`, `UI/components/TimelineView.qml`, `TimelineRuler.qml`, `TimelineLane.qml`, `TrackRow.qml`, and marker drag components for event conflicts.
- `Rust Viewport Audit`: inspect Rust timeline controller/state/tests for scroll, zoom, follow, visible seconds, and persistence APIs.
- `Performance Audit`: inspect waveform/model refresh paths and playback position update paths for expensive recomputation during playback.

For each implementation task below, use the normal two-stage subagent loop:

1. Dispatch one implementation subagent, `gpt-5.5 xhigh`, with only that task and the audit notes it needs.
2. Dispatch one spec-compliance review subagent, `gpt-5.5 xhigh`.
3. Dispatch one code-quality review subagent, `gpt-5.5 xhigh`.
4. Do not start the next implementation task until both reviews are clean.

Do not dispatch implementation subagents in parallel. These tasks touch the same QML and controller state.

---

## Target Files

Expected Rust-port targets:

- Modify: `docs/NOW.md`
- Modify/Create: `UI/components/TimelineNavigationSurface.qml`
- Modify: `UI/components/TimelineView.qml`
- Modify: `UI/components/TimelineRuler.qml`
- Modify: `UI/components/TimelineLane.qml`
- Modify: `UI/components/TrackRow.qml`
- Modify: `UI/Main.qml`
- Create/Modify: `crates/autolight-qt/src/app_controller/timeline_viewport.rs`
- Modify: `crates/autolight-qt/src/app_controller/timeline_controller.rs`
- Modify: `crates/autolight-qt/src/app_controller/playback_controller.rs`
- Modify: `crates/autolight-qt/src/app_controller/tests.rs`
- Modify: `crates/autolight-qt/src/timeline_model.rs`

If the Rust branch has split these paths differently, preserve the existing controller hierarchy and keep the same responsibilities.

---

### Task 1: Promote Native Timeline Navigation Batch

**Files:**
- Modify: `docs/NOW.md`

- [ ] Rewrite `docs/NOW.md` around only this first implementation batch: native QML timeline input plus Rust viewport API.
- [ ] Record exact target paths from the live Rust worktree.
- [ ] Record verification commands:

```bash
cargo fmt --all -- --check
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke
git diff --check
```

Commit after the batch is implemented, not after this docs-only promotion unless the repo convention requires separate dispatch commits.

---

### Task 2: Add Rust Viewport APIs For Native Input

**Files:**
- Create/Modify: `crates/autolight-qt/src/app_controller/timeline_viewport.rs`
- Modify: `crates/autolight-qt/src/app_controller/timeline_controller.rs`
- Modify: `crates/autolight-qt/src/app_controller/tests.rs`

- [ ] Add a pure Rust viewport-policy module if one does not already exist. It should not depend on QML, CXX-Qt generated types, QObject state, or model objects.
- [ ] Define small internal value types or an equivalent existing local pattern for finite timeline quantities. Keep QML-facing methods primitive only at the FFI boundary.
- [ ] Add tests for horizontal scroll by pixel delta: positive and negative deltas convert through `timeline_pixels_per_second`, clamp to `0..max_scroll`, and update visible waveform/model state only through the existing viewport refresh path.
- [ ] Add tests for zoom-by-factor around an anchor time: the anchor remains at the same screen x after zoom, with clamping at min/max.
- [ ] Add tests for pointer-vs-playhead anchor selection: if the pointer x is within a small threshold of the playhead x, the playhead is used as anchor; otherwise the pointer time is used.
- [ ] Add tests for user-navigation state: manual scroll or pinch temporarily suppresses playback follow.
- [ ] Implement pure Rust policy functions first, then expose thin controller slots:

```rust
fn scroll_by_pixels(viewport: TimelineViewport, delta: TimelinePixels) -> TimelineViewport;
fn zoom_by_factor(
    viewport: TimelineViewport,
    factor: f64,
    anchor: TimelineZoomAnchor,
) -> TimelineViewport;
fn scrub_at_x(viewport: TimelineViewport, x: TimelinePixels) -> TimelineSeconds;

// CXX-Qt/QML adapter methods remain primitive at the boundary:
scroll_timeline_by_pixels(pixel_delta_x: f64);
zoom_timeline_by_factor(factor: f64, anchor_x: f64, lane_width: f64);
begin_timeline_user_navigation();
end_timeline_user_navigation();
scrub_timeline_at_x(x: f64, lane_width: f64);
```

- [ ] Sanitize non-finite QML input before calling pure policy. Invalid input must not panic.
- [ ] Keep existing `set_timeline_zoom` and `set_timeline_scroll_seconds` as compatibility wrappers for sliders and persistence.

---

### Task 3: Add QML Timeline Navigation Surface

**Files:**
- Create/Modify: `UI/components/TimelineNavigationSurface.qml`
- Modify: `UI/components/TimelineView.qml`
- Modify: `UI/components/TimelineLane.qml`
- Modify: `UI/components/TimelineRuler.qml`
- Modify: `UI/Main.qml`

- [ ] Add a surface item over the timeline lane/ruler area, excluding track labels.
- [ ] Add `WheelHandler` with `acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad`.
- [ ] Use `WheelEvent.pixelDelta.x` for high-resolution horizontal panning when available; fall back to `angleDelta.x` or shifted vertical wheel for mouse hardware.
- [ ] Use modifier-wheel zoom for mouse users, mapping delta to a multiplicative factor rather than a fixed slider step.
- [ ] Add `PinchHandler { target: null }` and call `zoom_timeline_by_factor(delta, centroid.x, laneWidth)` on scale changes.
- [ ] Do not duplicate Rust viewport math in JavaScript. QML may compute local coordinates and event deltas only.
- [ ] Preserve marker drag/resize priority. Marker interactions must not accidentally pan or scrub the viewport.
- [ ] Keep vertical track scrolling on the existing `ListView`; horizontal gestures over the lane should pan time, vertical gestures should continue to scroll tracks.

---

### Task 4: Make Playhead And Ruler Scrubbable

**Files:**
- Modify: `UI/components/TimelineRuler.qml`
- Modify: `UI/components/TimelineLane.qml`
- Modify: `crates/autolight-qt/src/app_controller/timeline_controller.rs`
- Modify: `crates/autolight-qt/src/app_controller/tests.rs`

- [ ] Add tests that dragging the ruler/playhead calls one seek path and clamps to project duration.
- [ ] Add tests that scrubbing does not create edit-history entries and does not mark the project dirty.
- [ ] Make ruler click seek.
- [ ] Make ruler drag scrub continuously.
- [ ] Make the visible playhead handle draggable even when no marker/track item is selected.
- [ ] During scrubbing, pause playback follow until drag release, then resume according to follow mode.
- [ ] Keep scrub calculations in the Rust viewport-policy module and route QML drag events through the same controller path as ruler clicks.

---

### Task 5: Expand And Log-Scale Zoom

**Files:**
- Modify: `crates/autolight-qt/src/app_controller/timeline_controller.rs`
- Modify: `crates/autolight-qt/src/app_controller/tests.rs`
- Modify: `UI/Main.qml`
- Modify: `UI/components/TimelineRuler.qml`

- [ ] Replace the fixed `24..240 px/s` mental model with dynamic limits:
  - minimum: fit project duration into available lane width, with a small lower bound for empty/demo projects;
  - maximum: high enough for audio cue work, initially `8000 px/s` unless performance audit proves a lower bound is required.
- [ ] Represent min/max zoom and current zoom with the same Rust value type or invariant-checking helper so invalid zoom cannot spread across controller code.
- [ ] Convert the visible zoom slider to logarithmic mapping if the slider remains visible.
- [ ] Add zoom-to-fit action, preferably two-finger double-tap if Qt exposes it cleanly and a button/shortcut fallback.
- [ ] Ensure tick marks remain readable across the full range.
- [ ] Ensure waveform LOD selection still chooses more detail when zooming in.

---

### Task 6: Rework Playback Follow Modes

**Files:**
- Modify: `crates/autolight-qt/src/app_controller/timeline_controller.rs`
- Modify: `crates/autolight-qt/src/app_controller/playback_controller.rs`
- Modify: `crates/autolight-qt/src/app_controller/tests.rs`
- Modify: `UI/components/TimelineLane.qml`
- Modify: `UI/Main.qml`

- [ ] Add follow modes:
  - `off`: never auto-scroll, but show an offscreen playhead indicator;
  - `band`: scroll only when the playhead enters an edge band;
  - `center`: keep playhead centered while playing after it reaches center.
- [ ] Implement follow modes as a Rust enum and exhaustively match state transitions. Do not use ad hoc strings or booleans for internal follow state.
- [ ] Default to `center` for playback if it feels stable in smoke/manual verification; otherwise default to `band` and expose `center`.
- [ ] Suspend follow during manual user navigation and resume after a short quiet period.
- [ ] Keep playhead rendering visible and cheap even when timeline model refresh is throttled.
- [ ] Add tests for follow suppression, center mode target scroll, and offscreen indicator state.

---

### Task 7: Smooth Rendering And Refresh Scheduling

**Files:**
- Modify: `UI/components/WaveformStrip.qml`
- Modify: `UI/components/AnalysisStrip.qml`
- Modify: `UI/components/TimelineLane.qml`
- Modify: `crates/autolight-qt/src/timeline_model.rs`
- Modify: relevant Rust/QML tests

- [ ] Avoid rebuilding marker/waveform delegates on every playback tick.
- [ ] Use cheap x-position updates or a translated content layer for playhead and timeline content motion.
- [ ] Debounce waveform visible-window refresh during pinch/scroll, then commit a high-detail refresh when the gesture ends.
- [ ] Keep current waveform LOD behavior but prevent visible popping by preserving previous samples until replacement data is ready.
- [ ] Add or update tests proving playback position changes do not force timeline model rebuilds.
- [ ] Avoid `.clone()` of waveform sample arrays, marker spans, or track lists in playback-tick paths. If ownership is unavoidable, document why in code or isolate it outside the hot path.
- [ ] Use release-mode profiling only if the performance audit finds unresolved jank after the structural fixes; do not add speculative caching layers.

---

### Task 8: Verification And Handoff

**Files:**
- Modify: `docs/NOW.md`
- Modify: `README.md` only if user-facing controls changed enough to need run/use notes

- [ ] Run focused tests after each task.
- [ ] Run final verification:

```bash
cargo fmt --all -- --check
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D clippy::perf
QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke
git diff --check
```

- [ ] Do one manual macOS trackpad pass:
  - two-finger horizontal swipe pans timeline;
  - pinch zooms smoothly around pointer/playhead;
  - ruler drag scrubs;
  - playing in follow mode keeps playhead visible without choppy jumps;
  - manual scrolling during playback does not fight the user.
- [ ] Update `docs/NOW.md` with changes, verification, blockers if any, and next batch.

---

## Acceptance Criteria

- Horizontal two-finger trackpad scrolling works over the timeline lane/ruler.
- Pinch zoom works and uses an intuitive anchor.
- Zoom range supports both whole-song overview and detailed audio cue work.
- The playhead can be dragged/scrubbed from the ruler or visible handle.
- Playback follow keeps the playhead visible and does not scroll choppily.
- Manual navigation during playback temporarily wins over auto-follow.
- Timeline model and waveform refreshes are not tied to every playback tick.
- Viewport policy is testable Rust code independent of QML/CXX-Qt generated types.
- Internal follow/viewport state uses Rust types and enums instead of stringly typed or JavaScript-owned policy.
- Final Rust verification passes with `cargo fmt`, workspace tests, `clippy -D warnings`, and `clippy -D clippy::perf`.
