# Autolight Retained Waveform Renderer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` to implement this plan task-by-task. All implementation and review subagents must run as `gpt-5.5` with `xhigh` reasoning. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current QML `Canvas` waveform and analysis rendering path with a Rust-owned, zoom-aware waveform projection and a retained Qt Quick scene-graph item so timeline scroll, zoom, and playback follow do not repaint and serialize full waveform data on every viewport change.

**Architecture:** Keep waveform math, level selection, artifact parsing, cache validation, and viewport projection in Rust. Keep QML as a thin layout/input layer. Use Qt Quick retained scene-graph rendering for dense per-pixel waveform geometry; QML overlays remain responsible for markers, selection borders, and the playhead until they become proven bottlenecks.

**Tech Stack:** Rust/CXX-Qt 0.8.1, Qt Quick `QQuickItem`, Qt scene graph `QSGGeometryNode`, a small C++ helper compiled by `cxx-qt-build`, existing QML timeline components, `serde_json` compatibility tests, offscreen smoke checks.

---

## Research Baseline

- Qt Quick's scene graph is explicitly designed to retain primitives between frames, batch state, and render on a dedicated render thread on many platforms. Custom visual items add primitives through `QQuickItem::updatePaintNode()` and should use `QSG*` classes only during that call: <https://doc.qt.io/qt-6/qtquick-visualcanvas-scenegraph.html>
- Qt's `Canvas.Image` path is not a good fit for this workload. The docs say large canvases, frequent updates, and animation should generally be avoided because accelerated rendering requires texture uploads on each update: <https://doc.qt.io/qt-6/qml-qtquick-canvas.html>
- CXX-Qt supports `#[base = ...]` inheritance and `#[cxx_override]` method generation for Qt base-class overrides; this branch already uses CXX-Qt 0.8.1 and `cxx-qt-build` supports `.cpp_file(...)` for moc/compiled C++ helpers: <https://kdab.github.io/cxx-qt/book/concepts/inheritance.html>
- Audacity models waveform display as width-sized arrays of min/max/rms values for `t0` and `pixelsPerSecond`, which matches a viewport projection boundary instead of full artifact serialization: <https://doxy.audacityteam.org/_waveform_cache_8h_source.html>
- Sonic Visualiser documents the correct display behavior at extreme zoom: peak/mean columns per pixel while zoomed out, switching to individual samples/connected points when close enough: <https://sonicvisualiser.org/doc/reference/4.5.2/en/>
- wavesurfer.js recommends pre-decoded peaks for large files and streaming use cases, supporting the same idea that durable peak artifacts should feed the renderer instead of repeatedly decoding or moving raw audio through UI state: <https://wavesurfer.xyz/docs/>

## Rust Best-Practices Contract

- Rust owns the data model and viewport projection. QML must not select waveform LOD levels, iterate waveform buckets, or compute visible bucket ranges after this plan lands.
- The hot path must use borrowed data and bounded output. A renderer request may produce at most one peak/rms column per visible device pixel, plus a bounded sample-line output when zoomed past one sample per pixel.
- `timelineRowsJson` may continue to carry lightweight row metadata for now, but it must stop carrying `waveformLevels` sample arrays. Rows should carry a small `waveformRef` object pointing to a validated artifact/cache entry.
- Keep compatibility for current version 1/2 waveform payloads. New artifacts may use a version 3 peak pyramid, but old projects and demo payloads must still render.
- Put pure Rust rendering logic in testable modules without Qt types. The CXX-Qt/QSG item should be an adapter from QML properties to Rust renderer output and scene-graph geometry.
- Avoid `unwrap()`/`expect()` in production rendering, artifact loading, and cache validation paths. Invalid JSON, missing cache files, stale digests, non-finite viewport values, or unsupported artifact versions must render an empty frame and expose a diagnosable error state where the controller already does that.
- Do not add broad async/thread abstractions in this pass. Cache parsing may be memoized, but all long-running waveform generation remains in the existing job path.
- Treat Clippy as a gate. Do not add broad `allow` attributes; use local `#[expect]` only with a concrete reason if unavoidable.

## Subagent Workflow

Before implementation, dispatch three parallel audit subagents, all `gpt-5.5 xhigh`:

- `Waveform Data Audit`: inspect `crates/autolight-analysis/src/waveform.rs`, transform output payloads, cache entries, demo payloads, and existing waveform tests.
- `Qt Scene Graph Audit`: inspect `crates/autolight-qt/build.rs`, the generated bridge patterns in `app_controller/mod.rs`, CXX-Qt 0.8.1 inheritance/override support, and the current QML module registration.
- `Timeline QML Audit`: inspect `UI/components/TimelineLane.qml`, `WaveformStrip.qml`, `AnalysisStrip.qml`, `TimelineView.qml`, and `AppRuntime.qml` for props that must be replaced or kept.

For each implementation task below:

1. Run one implementation subagent with only that task and the relevant audit notes.
2. Run one spec-compliance review subagent.
3. Run one code-quality/performance review subagent.
4. Apply fixes from both reviews before starting the next task.

Tasks 2 and 3 can be developed in parallel only if they work in separate worktrees and merge through Task 4. Otherwise execute tasks serially.

## Target Files

- Modify: `docs/NOW.md`
- Modify: `crates/autolight-analysis/src/waveform.rs`
- Create: `crates/autolight-qt/src/timeline_renderer/mod.rs`
- Create: `crates/autolight-qt/src/timeline_renderer/waveform.rs`
- Create: `crates/autolight-qt/src/timeline_renderer/cache.rs`
- Create: `crates/autolight-qt/src/timeline_renderer/item.rs`
- Create: `crates/autolight-qt/src/timeline_renderer/scene_graph.h`
- Create: `crates/autolight-qt/src/timeline_renderer/scene_graph.cpp`
- Modify: `crates/autolight-qt/src/lib.rs`
- Modify: `crates/autolight-qt/src/timeline_model.rs`
- Modify: `crates/autolight-qt/src/app_controller/mod.rs`
- Modify: `crates/autolight-qt/src/app_controller/timeline_controller.rs`
- Modify: `crates/autolight-qt/src/app_controller/jobs.rs`
- Modify: `crates/autolight-qt/src/app_controller/tests.rs`
- Modify: `crates/autolight-qt/build.rs`
- Modify: `UI/components/TimelineLane.qml`
- Modify: `UI/components/TimelineView.qml`
- Delete or retire: `UI/components/WaveformStrip.qml`
- Delete or retire: `UI/components/AnalysisStrip.qml` only after analysis strips have an equivalent renderer.

## Task 1: Promote The Renderer Batch

**Files:**
- Modify: `docs/NOW.md`

- [ ] Replace the current active batch with "Retained Waveform Renderer".
- [ ] Record the target files above, trimmed if audits prove a smaller set.
- [ ] Record the implementation contract: Rust/CXX-Qt runtime only, Python remains reference-only, no `timelineRowsJson` embedded waveform levels, no QML Canvas for waveform rendering.
- [ ] Record the verification commands from the bottom of this plan.
- [ ] Add a handoff note that the first technical checkpoint is a minimal scene-graph item proof, not waveform business logic.

## Task 2: Prove The Qt Scene-Graph Bridge

**Files:**
- Create: `crates/autolight-qt/src/timeline_renderer/item.rs`
- Create: `crates/autolight-qt/src/timeline_renderer/scene_graph.h`
- Create: `crates/autolight-qt/src/timeline_renderer/scene_graph.cpp`
- Modify: `crates/autolight-qt/src/timeline_renderer/mod.rs`
- Modify: `crates/autolight-qt/src/lib.rs`
- Modify: `crates/autolight-qt/build.rs`
- Modify: `crates/autolight-qt/src/app_controller/tests.rs`

- [ ] Add a CXX-Qt `TimelineWaveformItem` qobject that inherits `QQuickItem` using `#[base = QQuickItem]` and is registered as a QML element in `Autolight.Qt`.
- [ ] Override `updatePaintNode` with `#[cxx_override]` and route node construction to a tiny C++ helper. Keep `QSG*` allocation and mutation inside the helper so Rust does not need broad Qt scene-graph bindings.
- [ ] Add a C++ helper API in `scene_graph.h/.cpp` that can build/update a root node with two `QSGGeometryNode` children: one for peak spans and one for rms spans. It should accept plain column data already projected by Rust.
- [ ] Set `QQuickItem::ItemHasContents` during initialization.
- [ ] Update `build.rs` to include `.qt_module("Quick")`, `.files(["src/app_controller/mod.rs", "src/timeline_renderer/item.rs"])`, and `.cpp_file(...)` entries for the helper header/source.
- [ ] Add a compile/smoke test assertion that `TimelineWaveformItem` is registered and that `WaveformStrip.qml` is not required for the app to load.
- [ ] If CXX-Qt 0.8.1 cannot override `QQuickItem::updatePaintNode` directly, complete the same task with a manual C++ `QQuickItem` subclass registered through `.cpp_file(...)` and moc, while keeping Rust projection in `timeline_renderer/waveform.rs`. Do not fall back to QML `Canvas`.

## Task 3: Introduce A Rust Waveform Projection Model

**Files:**
- Modify: `crates/autolight-analysis/src/waveform.rs`
- Create: `crates/autolight-qt/src/timeline_renderer/waveform.rs`
- Modify: `crates/autolight-qt/src/app_controller/tests.rs`

- [ ] Add or formalize structs for durable peak data:

```rust
pub struct WaveformPeakColumn {
    pub min: f32,
    pub max: f32,
    pub rms: f32,
    pub count: u32,
}

pub struct WaveformPeakLevel {
    pub samples_per_column: u32,
    pub columns: Vec<WaveformPeakColumn>,
}

pub struct WaveformPeakPyramid {
    pub sample_rate: u32,
    pub frame_count: u64,
    pub levels: Vec<WaveformPeakLevel>,
}
```

- [ ] Keep current `WaveformPayload` version 2 deserialization working. Convert v1/v2 payloads into the new in-memory pyramid on load.
- [ ] Add a version 3 JSON payload only if it materially simplifies cache storage; otherwise keep the public artifact JSON stable and use the new pyramid as the internal representation.
- [ ] Add `WaveformRenderRequest` with `scroll_seconds`, `visible_seconds`, `pixels_per_second`, `width_pixels`, `height_pixels`, `left_padding_pixels`, and device pixel ratio.
- [ ] Add `WaveformRenderFrame` with either:
  - `PeakColumns(Vec<WaveformColumnGeometry>)`, one output column per visible pixel at most; or
  - `SamplePolyline(Vec<WaveformSamplePoint>)`, used when `samples_per_pixel < 1.0`.
- [ ] Implement LOD selection using `samples_per_pixel = sample_rate / pixels_per_second`. Choose the finest level whose `samples_per_column <= samples_per_pixel`, then aggregate adjacent columns into output pixels if needed.
- [ ] Preserve spikes and impulses. The aggregation rule for an output pixel must use min-of-min and max-of-max across all source columns touching that pixel, and rms must combine `sum_squares/count` rather than averaging rms values.
- [ ] Add unit tests:
  - `waveform_projection_chooses_peak_columns_when_zoomed_out`
  - `waveform_projection_switches_to_sample_polyline_when_zoomed_in`
  - `waveform_projection_preserves_single_sample_impulse_when_zoomed_out`
  - `waveform_projection_combines_rms_from_energy_not_average`
  - `waveform_projection_output_is_bounded_by_visible_width`

## Task 4: Replace Embedded Row Waveforms With Waveform References

**Files:**
- Modify: `crates/autolight-qt/src/timeline_model.rs`
- Modify: `crates/autolight-qt/src/app_controller/tests.rs`
- Modify: `UI/components/TimelineView.qml`
- Modify: `UI/components/TrackRow.qml`
- Modify: `UI/components/TimelineLane.qml`

- [ ] Replace `TimelineRow.waveform_levels` with a lightweight `TimelineWaveformRef`:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineWaveformRef {
    pub track_id: String,
    pub cache_ref: String,
    pub artifact_kind: String,
    pub duration_seconds: f64,
    pub sample_rate: u32,
}
```

- [ ] Serialize `waveformRef: null` for rows without a valid complete waveform artifact.
- [ ] Remove `waveformLevels` from `timelineRowsJson` after adding transitional tests. Do not keep both fields long-term.
- [ ] Keep row duration available as metadata, but do not derive painting data in QML from row JSON.
- [ ] Update QML property plumbing so `TimelineLane` receives `waveformRef` and passes only identity/viewport props to `TimelineWaveformItem`.
- [ ] Add tests:
  - `timeline_rows_emit_waveform_ref_without_embedded_levels`
  - `timeline_rows_omit_waveform_ref_for_invalid_or_stale_cache`
  - `qml_timeline_uses_timeline_waveform_item_not_waveform_strip_canvas`

## Task 5: Add A Controller Waveform Cache Boundary

**Files:**
- Create: `crates/autolight-qt/src/timeline_renderer/cache.rs`
- Modify: `crates/autolight-qt/src/app_controller/mod.rs`
- Modify: `crates/autolight-qt/src/app_controller/timeline_controller.rs`
- Modify: `crates/autolight-qt/src/app_controller/jobs.rs`
- Modify: `crates/autolight-qt/src/app_controller/tests.rs`

- [ ] Add a cache owned by `AppControllerState` or a child timeline renderer state:

```rust
pub struct WaveformArtifactCache {
    entries: BTreeMap<String, CachedWaveformArtifact>,
}

pub struct CachedWaveformArtifact {
    pub cache_ref: String,
    pub payload_digest: String,
    pub pyramid: WaveformPeakPyramid,
}
```

- [ ] Resolve `cache_ref` through existing project cache entries and path validation helpers. Never trust a QML-provided file path.
- [ ] Invalidate cached parsed artifacts when cache entry path, validation status, dependency digest, or payload digest changes.
- [ ] Add an invokable/query path for `TimelineWaveformItem` to request a projected frame by `track_id`, `cache_ref`, and viewport values. Return empty geometry for invalid/missing artifacts.
- [ ] Keep projection side-effect free: requesting waveform geometry must not mutate project state, edit history, selected track, dirty state, or job state.
- [ ] Add tests:
  - `controller_waveform_cache_reuses_parsed_artifact_for_viewport_changes`
  - `controller_waveform_cache_invalidates_when_payload_digest_changes`
  - `controller_waveform_render_request_rejects_unknown_cache_ref`
  - `controller_waveform_render_request_does_not_mark_project_dirty`

## Task 6: Wire The Retained Item Into Timeline QML

**Files:**
- Modify: `UI/components/TimelineLane.qml`
- Modify: `UI/components/TimelineView.qml`
- Modify: `UI/components/TrackRow.qml`
- Modify: `UI/Main.qml`
- Delete or retire: `UI/components/WaveformStrip.qml`
- Modify: `crates/autolight-qt/src/app_controller/tests.rs`

- [ ] Replace the `WaveformStrip` instance with `TimelineWaveformItem`.
- [ ] Bind item properties to `appController`, `trackId`, `waveformRef.cacheRef`, `waveformRef.durationSeconds`, `timelineScrollSeconds`, `timelineVisibleSeconds`, `timelinePixelsPerSecond`, `timelineLeftPadding`, and lane height/width.
- [ ] Make item updates explicit: when viewport props change, QML changes properties; the item schedules `update()` and reuses scene-graph nodes where possible.
- [ ] Keep marker blocks above waveform geometry with current z ordering.
- [ ] Keep selected-row border/stripe and playhead visibility unchanged.
- [ ] Delete `WaveformStrip.qml` only after tests prove no QML imports or bundle code reference it. If deletion is too noisy for this batch, leave it unused and add a follow-up deletion step in the same PR before final verification.
- [ ] Add QML structure tests that fail if waveform rendering goes back to `Canvas`.

## Task 7: Move Analysis Strips Off Canvas

**Files:**
- Create or extend: `crates/autolight-qt/src/timeline_renderer/waveform.rs`
- Modify: `crates/autolight-qt/src/timeline_model.rs`
- Modify: `UI/components/TimelineLane.qml`
- Delete or retire: `UI/components/AnalysisStrip.qml`
- Modify: `crates/autolight-qt/src/app_controller/tests.rs`

- [ ] Decide whether energy/harmonic strips are rendered by `TimelineWaveformItem` as extra geometry bands or by a sibling `TimelineAnalysisItem`. Prefer a sibling item only if it keeps the Rust projection types simpler.
- [ ] Stop emitting dense visible analysis samples on every timeline row refresh if an artifact ref can represent the analysis output.
- [ ] Project analysis bands in Rust with the same viewport request and bounded visible-width output.
- [ ] Use scene-graph rectangles or vertex colors, not QML `Canvas`, for energy and harmonic-color bands.
- [ ] Add tests:
  - `timeline_rows_emit_analysis_refs_without_visible_canvas_samples`
  - `qml_timeline_uses_retained_analysis_renderer_not_canvas`
  - `analysis_projection_output_is_bounded_by_visible_width`

## Task 8: Preserve Smooth Interaction Semantics

**Files:**
- Modify: `crates/autolight-qt/src/app_controller/timeline_controller.rs`
- Modify: `UI/AppRuntime.qml`
- Modify: `UI/components/TimelineNavigationSurface.qml`
- Modify: `UI/components/TimelineLane.qml`
- Modify: `crates/autolight-qt/src/app_controller/tests.rs`

- [ ] Ensure scroll, pinch zoom, ruler drag, playhead drag, and playback follow still call the existing Rust viewport APIs from the native navigation batch.
- [ ] Confirm viewport changes do not reload `timelineRowsJson` unless row metadata actually changed.
- [ ] Keep playback position ticks on the lightweight sync path.
- [ ] Add a regression test that `timelineRowsJson` is not parsed or rebuilt for repeated playback-position updates.
- [ ] Add a regression test that viewport-only scroll/zoom changes update renderer item props without refreshing row models.

## Task 9: Verification And Manual Feel Pass

**Commands:**

```bash
cargo fmt --all -- --check
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-analysis --locked
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D clippy::perf
QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke
git diff --check
```

**Manual macOS pass:**

- [ ] Open the Rust app with `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo run -p autolight-app`.
- [ ] Load the demo and confirm waveform renders for the full song at fit, mid, and max zoom.
- [ ] Two-finger horizontal scroll should stay smooth while waveform geometry remains stable.
- [ ] Pinch zoom should update detail without visible LOD popping or canvas repaint flicker.
- [ ] During playback follow at high zoom, the playhead should remain visible and timeline motion should be smooth.
- [ ] Dragging the playhead/ruler should scrub without row reload jank.
- [ ] Rerun the waveform transform and confirm the retained renderer updates after cache invalidation.

## Done Criteria

- `WaveformStrip.qml` is deleted or unused, and no active timeline waveform path uses QML `Canvas`.
- `AnalysisStrip.qml` is deleted or unused, or analysis strips have an explicit retained-renderer replacement in the same PR.
- `timelineRowsJson` no longer serializes waveform sample/level arrays.
- Rust tests prove LOD selection, impulse preservation, RMS aggregation, output bounds, cache invalidation, and no dirty-state mutation.
- QML structure tests prove the timeline uses retained renderer items.
- Full automated verification passes, or any blocker is recorded in `docs/NOW.md` with exact command/output.
- `docs/NOW.md` contains a short completion update and the next batch handoff.
