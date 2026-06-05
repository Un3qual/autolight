# Autolight Native Timeline Scene Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. All implementation and review subagents must run as `gpt-5.5` with `xhigh` reasoning. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the reactive QML timeline lane/ruler/rendering stack with one native CXX-Qt timeline scene item that keeps playback follow, scroll, and zoom smooth by making per-frame viewport changes transform-only and moving expensive tile/detail rebuilds off the hot path.

**Architecture:** QML keeps app chrome, toolbar controls, inspectors, and layout. A native `TimelineSceneItem` owns timeline rendering, gestures, ruler ticks, playhead, markers, waveform/analysis layers, and transient viewport interaction state. Rust owns scene snapshots, viewport policy, waveform/analysis projection, cache validation, and tile scheduling; Qt scene graph owns retained geometry and swaps prepared tiles only when they are ready.

**Tech Stack:** Rust/CXX-Qt 0.8.1, Qt Quick `QQuickItem`, Qt scene graph `QSGGeometryNode`, small C++ scene-graph adapter helpers compiled through `cxx-qt-build`, Rust worker/cache modules, existing `TimelineViewport` policy, existing project/cache/job models.

---

## Root Cause Summary

Manual testing showed the timeline remains choppy while playing, following the playhead, and dragging the zoom slider. The current architecture still has these hot-path costs:

- `timelineScrollSeconds` and `timelinePixelsPerSecond` update QML bindings across ruler ticks, markers, playhead, lanes, slider labels, and renderer geometry.
- Waveform/analysis detail still crosses `QML -> Rust qinvokable -> QString JSON -> C++ QJsonDocument -> QSGGeometry` when geometry changes.
- The zoom slider calls into Rust continuously and then QML refreshes viewport mirrors continuously.
- Ruler ticks and markers are individual QML items positioned through bindings instead of retained native scene primitives.

The next implementation must stop tuning those bindings and replace the timeline surface.

## Non-Negotiable Architecture Rules

- No JSON geometry payloads in the playback-follow, scroll, or zoom hot path.
- No QML `Repeater` for moving timeline contents, ruler ticks, playhead, markers, waveform columns, or analysis strips.
- Playback follow and scroll must be transform-only per frame.
- Zoom drag must update a transient native viewport immediately; expensive waveform/analysis detail swaps must be asynchronous or deferred until prepared.
- Rust must keep pure, testable viewport and projection logic. C++/Qt must only adapt prepared Rust draw data into `QSGNode`/`QSGGeometry`.
- QML may receive summary properties for labels and controls, but it must not render timeline primitives after cutover.
- Keep Python reference behavior untouched except tests that explicitly assert Rust/QML structure.

## Target Files

- Modify: `docs/NOW.md`
- Modify: `crates/autolight-qt/build.rs`
- Modify: `crates/autolight-qt/src/lib.rs`
- Modify: `crates/autolight-qt/src/app_controller/mod.rs`
- Modify: `crates/autolight-qt/src/app_controller/tests.rs`
- Modify: `crates/autolight-qt/src/app_controller/timeline_controller.rs`
- Modify: `crates/autolight-qt/src/app_controller/timeline_viewport.rs`
- Modify: `crates/autolight-qt/src/timeline_model.rs`
- Create: `crates/autolight-qt/src/timeline_scene/mod.rs`
- Create: `crates/autolight-qt/src/timeline_scene/model.rs`
- Create: `crates/autolight-qt/src/timeline_scene/viewport.rs`
- Create: `crates/autolight-qt/src/timeline_scene/tiles.rs`
- Create: `crates/autolight-qt/src/timeline_scene/item.rs`
- Create: `crates/autolight-qt/src/timeline_scene/scene_graph.h`
- Create: `crates/autolight-qt/src/timeline_scene/scene_graph.cpp`
- Modify: `crates/autolight-qt/src/timeline_renderer/waveform.rs`
- Modify: `crates/autolight-qt/src/timeline_renderer/cache.rs`
- Modify: `UI/Main.qml`
- Modify: `UI/components/TimelineView.qml`
- Retire from active Rust path: `UI/components/TimelineLane.qml`
- Retire from active Rust path: `UI/components/TimelineRuler.qml`
- Retire from active Rust path: `UI/components/TimelineNavigationSurface.qml`
- Retire from active Rust path: `UI/components/MarkerBlock.qml`

## Task 1: Lock The Active Batch And Red Tests

**Files:**
- Modify: `docs/NOW.md`
- Modify: `crates/autolight-qt/src/app_controller/tests.rs`

- [ ] **Step 1: Promote the active batch in `docs/NOW.md`**

Set the active batch to `Native Timeline Scene` with status `ready`. The goal should explicitly say that QML timeline primitives are being replaced by one native scene item because the retained waveform renderer still stutters under playback follow and zoom.

- [ ] **Step 2: Add a QML structure regression that fails on the current code**

Add a test in `crates/autolight-qt/src/app_controller/tests.rs` named:

```rust
#[test]
fn qml_timeline_uses_native_scene_item_instead_of_reactive_lanes() {
    let timeline_view_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../UI/components/TimelineView.qml"),
    )
    .unwrap();
    let main_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../UI/Main.qml"),
    )
    .unwrap();

    assert!(timeline_view_qml.contains("TimelineSceneItem"));
    assert!(timeline_view_qml.contains("sceneSnapshotJson:"));
    assert!(timeline_view_qml.contains("viewportScrollSeconds:"));
    assert!(timeline_view_qml.contains("viewportPixelsPerSecond:"));
    assert!(!timeline_view_qml.contains("delegate: TrackRow"));
    assert!(!timeline_view_qml.contains("TimelineLane"));
    assert!(!timeline_view_qml.contains("TimelineRuler"));
    assert!(!main_qml.contains("TimelineRuler"));
}
```

- [ ] **Step 3: Add a renderer-boundary regression that fails on the current code**

Add a test in `crates/autolight-qt/src/app_controller/tests.rs` named:

```rust
#[test]
fn qml_timeline_scene_hot_path_has_no_geometry_json_or_qml_marker_repeaters() {
    let timeline_view_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../UI/components/TimelineView.qml"),
    )
    .unwrap();
    let lane_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../UI/components/TimelineLane.qml"),
    )
    .unwrap_or_default();
    let scene_header = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/timeline_scene/scene_graph.h"),
    )
    .unwrap();

    assert!(!timeline_view_qml.contains("renderTimelineWaveform("));
    assert!(!timeline_view_qml.contains("renderTimelineAnalysis("));
    assert!(!timeline_view_qml.contains("geometryJson"));
    assert!(!lane_qml.contains("geometryJson"));
    assert!(!lane_qml.contains("Repeater {\n        model: root.listOrEmpty(markerSpans)"));
    assert!(!scene_header.contains("Q_PROPERTY(QString geometryJson"));
}
```

- [ ] **Step 4: Verify both tests fail for the expected reason**

Run:

```bash
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_timeline_uses_native_scene_item_instead_of_reactive_lanes
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_timeline_scene_hot_path_has_no_geometry_json_or_qml_marker_repeaters
```

Expected: both fail because `TimelineSceneItem` and `src/timeline_scene/scene_graph.h` do not exist yet.

## Task 2: Add Pure Rust Timeline Scene Snapshot Types

**Files:**
- Create: `crates/autolight-qt/src/timeline_scene/mod.rs`
- Create: `crates/autolight-qt/src/timeline_scene/model.rs`
- Modify: `crates/autolight-qt/src/lib.rs`
- Modify: `crates/autolight-qt/src/timeline_model.rs`
- Modify: `crates/autolight-qt/src/app_controller/tests.rs`

- [ ] **Step 1: Create the module skeleton**

Create `crates/autolight-qt/src/timeline_scene/mod.rs`:

```rust
pub mod model;
pub mod tiles;
pub mod viewport;
```

Update `crates/autolight-qt/src/lib.rs`:

```rust
pub mod timeline_scene;
```

- [ ] **Step 2: Define the scene snapshot structs**

Create `crates/autolight-qt/src/timeline_scene/model.rs` with these public data types:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSceneSnapshot {
    pub tracks: Vec<TimelineSceneTrack>,
    pub duration_seconds: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSceneTrack {
    pub track_id: String,
    pub name: String,
    pub track_type: String,
    pub result_state: String,
    pub depth: usize,
    pub selected: bool,
    pub expanded: bool,
    pub markers: Vec<TimelineSceneMarker>,
    pub waveform_ref: Option<TimelineSceneArtifactRef>,
    pub analysis_refs: Vec<TimelineSceneArtifactRef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSceneMarker {
    pub marker_id: String,
    pub timestamp: f64,
    pub duration: f64,
    pub label: String,
    pub color: String,
    pub selected: bool,
    pub editable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSceneArtifactRef {
    pub track_id: String,
    pub cache_ref: String,
    pub artifact_kind: String,
    pub duration_seconds: f64,
}
```

- [ ] **Step 3: Build snapshots from existing projected rows**

Add a conversion function in `timeline_scene/model.rs`:

```rust
pub fn scene_snapshot_from_rows(
    rows: &[crate::timeline_model::TimelineRow],
    duration_seconds: f64,
) -> TimelineSceneSnapshot {
    TimelineSceneSnapshot {
        duration_seconds,
        tracks: rows
            .iter()
            .map(TimelineSceneTrack::from)
            .collect(),
    }
}
```

Implement `From<&TimelineRow>` for `TimelineSceneTrack` by copying row metadata, marker spans, waveform refs, and analysis refs. If the existing `TimelineRow` fields are private, add narrow accessors instead of making the entire row public.

- [ ] **Step 4: Add a snapshot serialization test**

Add:

```rust
#[test]
fn timeline_scene_snapshot_contains_static_tracks_without_waveform_geometry() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();

    let snapshot_json = state.timeline_scene_snapshot_json_state();
    let parsed: serde_json::Value = serde_json::from_str(&snapshot_json).unwrap();

    assert!(parsed["tracks"].as_array().unwrap().len() >= 3);
    assert!(parsed["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|track| track["waveformRef"].is_object()));
    assert!(!snapshot_json.contains("waveformLevels"));
    assert!(!snapshot_json.contains("\"rects\""));
    assert!(!snapshot_json.contains("\"bands\""));
}
```

- [ ] **Step 5: Run the test and make it pass**

Run:

```bash
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_scene_snapshot_contains_static_tracks_without_waveform_geometry
```

Expected: pass after adding `timeline_scene_snapshot_json_state()`.

## Task 3: Add Native TimelineSceneItem Skeleton

**Files:**
- Create: `crates/autolight-qt/src/timeline_scene/item.rs`
- Create: `crates/autolight-qt/src/timeline_scene/scene_graph.h`
- Create: `crates/autolight-qt/src/timeline_scene/scene_graph.cpp`
- Modify: `crates/autolight-qt/build.rs`
- Modify: `crates/autolight-qt/src/app_controller/tests.rs`

- [ ] **Step 1: Add the native item properties**

Create a native `TimelineSceneItem` with these QML-facing properties:

```cpp
Q_PROPERTY(QString sceneSnapshotJson READ sceneSnapshotJson WRITE setSceneSnapshotJson NOTIFY sceneSnapshotJsonChanged)
Q_PROPERTY(double viewportScrollSeconds READ viewportScrollSeconds WRITE setViewportScrollSeconds NOTIFY viewportChanged)
Q_PROPERTY(double viewportPixelsPerSecond READ viewportPixelsPerSecond WRITE setViewportPixelsPerSecond NOTIFY viewportChanged)
Q_PROPERTY(double viewportVisibleSeconds READ viewportVisibleSeconds WRITE setViewportVisibleSeconds NOTIFY viewportChanged)
Q_PROPERTY(double playbackPositionSeconds READ playbackPositionSeconds WRITE setPlaybackPositionSeconds NOTIFY viewportChanged)
Q_PROPERTY(int selectedTrackIndex READ selectedTrackIndex NOTIFY sceneSnapshotJsonChanged)
```

Do not add `geometryJson`.

- [ ] **Step 2: Implement a no-op scene graph root**

In `scene_graph.cpp`, implement `updatePaintNode` so an empty scene returns a root node and a non-empty scene draws only:

- lane backgrounds;
- selected track outline;
- playhead line.

This proves the item can render without QML lane primitives.

- [ ] **Step 3: Wire the build**

Update `crates/autolight-qt/build.rs` to compile the new item/helper files. Keep the existing retained waveform helper until later tasks remove its active use.

- [ ] **Step 4: Add a QML registration smoke assertion**

Extend the existing embedded bundle or QML structure tests to assert:

```rust
assert!(timeline_view_qml.contains("TimelineSceneItem"));
assert!(scene_header.contains("class TimelineSceneItem"));
assert!(!scene_header.contains("geometryJson"));
```

- [ ] **Step 5: Run focused checks**

```bash
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_timeline_uses_native_scene_item_instead_of_reactive_lanes
QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke
```

Expected: test and smoke pass once QML is switched in Task 4.

## Task 4: Replace TimelineView QML With The Native Scene Item

**Files:**
- Modify: `UI/components/TimelineView.qml`
- Modify: `UI/Main.qml`
- Modify: `crates/autolight-qt/src/app_controller/mod.rs`
- Modify: `crates/autolight-qt/src/app_controller/tests.rs`

- [ ] **Step 1: Expose scene snapshot JSON from the controller**

Add a read-only qproperty:

```rust
#[qproperty(QString, timeline_scene_snapshot_json, cxx_name = "timelineSceneSnapshotJson")]
timeline_scene_snapshot_json: QString,
```

Refresh it only when project rows, selection, marker edits, expansion, or cache refs change. Do not refresh it for scroll, playback position, zoom, or visible seconds.

- [ ] **Step 2: Replace the ListView delegate stack**

Rewrite `UI/components/TimelineView.qml` so the active content is:

```qml
import QtQuick
import Autolight.Qt 1.0

Item {
    id: timelineRoot
    property var appController
    signal trackSelected(string trackId)
    signal seekRequested(real x)

    TimelineSceneItem {
        id: scene
        anchors.fill: parent
        sceneSnapshotJson: timelineRoot.appController.timelineSceneSnapshotJson
        viewportScrollSeconds: timelineRoot.appController.timelineScrollSeconds
        viewportPixelsPerSecond: timelineRoot.appController.timelinePixelsPerSecond
        viewportVisibleSeconds: timelineRoot.appController.timelineVisibleSeconds
        playbackPositionSeconds: timelineRoot.appController.playback.positionSeconds
        onTrackClicked: function(trackId) { timelineRoot.trackSelected(trackId) }
        onScrubRequested: function(seconds) { timelineRoot.appController.seek_playback(seconds) }
    }
}
```

Adapt exact signal names to the native item implementation.

- [ ] **Step 3: Remove `TimelineRuler` from `UI/Main.qml`**

Remove the QML ruler row. The native item must draw the ruler and playhead itself.

- [ ] **Step 4: Keep toolbar sliders but avoid continuous model reloads**

The zoom slider may keep calling `set_timeline_zoom_for_lane_width`, but `reloadViewportState()` must update only viewport qproperties. It must not rebuild `timelineSceneSnapshotJson`.

- [ ] **Step 5: Run structure tests**

```bash
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_timeline_uses_native_scene_item_instead_of_reactive_lanes
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_timeline_scene_hot_path_has_no_geometry_json_or_qml_marker_repeaters
```

Expected: both pass.

## Task 5: Move Ruler, Markers, Selection, And Playhead Into Native Geometry

**Files:**
- Modify: `crates/autolight-qt/src/timeline_scene/model.rs`
- Modify: `crates/autolight-qt/src/timeline_scene/scene_graph.h`
- Modify: `crates/autolight-qt/src/timeline_scene/scene_graph.cpp`
- Modify: `crates/autolight-qt/src/app_controller/tests.rs`

- [ ] **Step 1: Add deterministic layout constants**

Use one Rust/C++ shared contract:

```rust
pub const TIMELINE_LABEL_WIDTH: f64 = 280.0;
pub const TIMELINE_RULER_HEIGHT: f64 = 32.0;
pub const TIMELINE_ROW_HEIGHT: f64 = 76.0;
pub const TIMELINE_LEFT_PADDING: f64 = 24.0;
```

Mirror these in C++ only if CXX sharing is not practical. Do not duplicate divergent values in QML.

- [ ] **Step 2: Draw the ruler in `TimelineSceneItem`**

Ruler ticks must be generated in native code from viewport values. Generate at most one major/minor tick per 8 screen pixels.

- [ ] **Step 3: Draw markers in `TimelineSceneItem`**

Use scene snapshot markers. Clamp marker rectangles to the visible viewport before adding geometry. Selected markers get a border or overlay geometry.

- [ ] **Step 4: Draw selected track state natively**

The selected track outline and label-side stripe must come from native scene state, not a QML `TrackRow` delegate.

- [ ] **Step 5: Draw playhead natively**

The playhead line/cap is based on `playbackPositionSeconds` and current viewport. During playback, updating this property must not rebuild waveform/analysis tiles.

- [ ] **Step 6: Add tests**

Add tests named:

```rust
timeline_scene_snapshot_preserves_marker_selection_for_native_rendering
timeline_scene_item_draws_ruler_markers_selection_and_playhead_without_qml_repeaters
```

The second test can be a structure test that asserts retired QML files are not referenced and native files contain the expected layer names.

## Task 6: Add Native Gesture And Transient Viewport Handling

**Files:**
- Create: `crates/autolight-qt/src/timeline_scene/viewport.rs`
- Modify: `crates/autolight-qt/src/app_controller/timeline_viewport.rs`
- Modify: `crates/autolight-qt/src/timeline_scene/scene_graph.h`
- Modify: `crates/autolight-qt/src/timeline_scene/scene_graph.cpp`
- Modify: `UI/components/TimelineView.qml`
- Modify: `crates/autolight-qt/src/app_controller/tests.rs`

- [ ] **Step 1: Move gesture handling into the native item**

The native item handles:

- horizontal wheel/trackpad pan;
- shift-wheel pan fallback;
- modifier-wheel zoom;
- pinch zoom;
- ruler click/drag scrub;
- playhead handle drag.

QML should no longer instantiate `TimelineNavigationSurface`.

- [ ] **Step 2: Add transient viewport state**

The native item keeps a transient viewport during active gesture/slider/playback-follow frames. It emits committed viewport changes to the controller after:

- wheel quiet timer expires;
- pinch ends;
- mouse drag releases;
- zoom slider releases, if the slider is moved to native item ownership later.

- [ ] **Step 3: Keep Rust viewport policy authoritative**

Reuse `timeline_viewport.rs` for clamp/zoom/follow math. If the native item is C++, call a narrow Rust/CXX bridge for viewport policy, or port the pure math into `timeline_scene/viewport.rs` with identical tests. Do not leave a separate QML implementation.

- [ ] **Step 4: Add tests**

Add tests:

```rust
timeline_scene_viewport_scroll_is_transform_only_until_commit
timeline_scene_viewport_zoom_preserves_anchor_without_scene_snapshot_reload
qml_no_longer_instantiates_timeline_navigation_surface
```

## Task 7: Replace Waveform/Analysis JSON Geometry With Double-Buffered Tiles

**Files:**
- Create: `crates/autolight-qt/src/timeline_scene/tiles.rs`
- Modify: `crates/autolight-qt/src/timeline_renderer/waveform.rs`
- Modify: `crates/autolight-qt/src/timeline_renderer/cache.rs`
- Modify: `crates/autolight-qt/src/timeline_scene/scene_graph.h`
- Modify: `crates/autolight-qt/src/timeline_scene/scene_graph.cpp`
- Modify: `crates/autolight-qt/src/app_controller/mod.rs`
- Modify: `crates/autolight-qt/src/app_controller/tests.rs`

- [ ] **Step 1: Define tile keys**

Create:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimelineTileKey {
    pub track_row: usize,
    pub layer: TimelineTileLayer,
    pub zoom_bucket: i32,
    pub start_bucket: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TimelineTileLayer {
    Waveform,
    Energy,
    HarmonicColor,
}
```

- [ ] **Step 2: Define prepared tile geometry as Rust structs, not JSON**

Use:

```rust
pub struct PreparedTimelineTile {
    pub key: TimelineTileKey,
    pub origin_seconds: f64,
    pub width_seconds: f64,
    pub bands: Vec<PreparedTimelineBand>,
}

pub struct PreparedTimelineBand {
    pub color_rgba: [f32; 4],
    pub rects: Vec<PreparedTimelineRect>,
}

pub struct PreparedTimelineRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
```

If CXX cannot move these structs directly, use a compact binary `QByteArray` with a documented header and fixed-width little-endian records. Do not use JSON.

- [ ] **Step 3: Add double buffering**

The scene item keeps:

- `active_tiles`: currently displayed tiles;
- `pending_tiles`: tiles being prepared for the next zoom/scroll bucket;
- `last_good_tiles`: fallback while pending tiles build.

Viewport motion transforms `active_tiles`. It never blocks waiting for `pending_tiles`.

- [ ] **Step 4: Build tile preparation off the UI hot path**

Use a Rust worker queue or a Qt queued connection. Tile prep may be synchronous in tests, but app code must not do waveform/analysis projection from `updatePaintNode`.

- [ ] **Step 5: Add tile cache tests**

Add tests:

```rust
timeline_tiles_reuse_active_tile_during_scroll_within_tile
timeline_tiles_prepare_next_zoom_bucket_without_replacing_active_tile
timeline_tile_payload_is_not_json
timeline_tile_projection_output_is_bounded_by_visible_device_pixels
```

## Task 8: Remove Old Active Timeline Components

**Files:**
- Modify or delete: `UI/components/TimelineLane.qml`
- Modify or delete: `UI/components/TimelineRuler.qml`
- Modify or delete: `UI/components/TimelineNavigationSurface.qml`
- Modify or delete: `UI/components/MarkerBlock.qml`
- Modify: `crates/autolight-app/src/main.rs`
- Modify: `tests/test_app_controller.py`
- Modify: `crates/autolight-qt/src/app_controller/tests.rs`

- [ ] **Step 1: Remove retired files from the embedded Rust app bundle**

The active Rust app should no longer bundle timeline lane/ruler/navigation/marker QML if the native scene owns those primitives.

- [ ] **Step 2: Keep files only if Python reference still imports them**

If the Python reference path still needs a retired QML component, keep it in the repo but guard Rust structure tests so the Rust path does not instantiate it.

- [ ] **Step 3: Update Python structure tests**

Python tests must assert reference compatibility only. They must not require the Rust app to keep old QML timeline primitives.

- [ ] **Step 4: Add bundle assertions**

Add a Rust app test asserting the embedded QML bundle contains `TimelineSceneItem` usage and does not require deleted components for the Rust path.

## Task 9: Performance Instrumentation And Real-Window Gate

**Files:**
- Create: `crates/autolight-qt/src/timeline_scene/perf.rs`
- Modify: `crates/autolight-qt/src/timeline_scene/scene_graph.cpp`
- Modify: `docs/NOW.md`
- Modify: `crates/autolight-qt/src/app_controller/tests.rs`

- [ ] **Step 1: Add lightweight performance counters**

Track these counters behind a debug/test-accessible API:

- scene snapshot parses;
- tile prepare count;
- active tile swaps;
- `updatePaintNode` calls;
- worst tile prepare milliseconds;
- worst scene-graph update milliseconds.

- [ ] **Step 2: Add a test-only counter snapshot**

Expose an invokable or test helper that returns:

```json
{
  "sceneSnapshotParses": 1,
  "tilePrepares": 0,
  "tileSwaps": 0,
  "paintUpdates": 12
}
```

- [ ] **Step 3: Add a synthetic playback-follow test**

Add a Rust unit test that advances playback position across 300 frames and asserts:

- no scene snapshot rebuilds;
- no synchronous tile preparation on the paint path;
- viewport updates do not mutate `timelineSceneSnapshotJson`.

- [ ] **Step 4: Run the real-window manual gate**

Run:

```bash
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo run -p autolight-app
```

Manual checks:

- load demo project;
- zoom to high zoom;
- play with follow mode `Band`;
- play with follow mode `Center`;
- drag zoom slider while playing;
- two-finger horizontal scroll with trackpad;
- pinch zoom around playhead;
- drag playhead in ruler.

Expected: no visible freezes, no delayed multi-frame stalls, playhead remains visible, waveform detail may refine after zoom without blocking motion.

## Final Verification

Run:

```bash
cargo fmt --all -- --check
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D clippy::perf
QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke
git diff --check
```

Then run the real-window manual gate in Task 9.

## Execution Notes

- Do not attempt to keep the old QML timeline path alive for the Rust runtime except as a temporary fallback while a task is in progress.
- Do not tune `SmoothedAnimation`, tile width, JSON caps, or QML `Repeater` shapes as a substitute for this plan.
- If CXX-Qt inheritance blocks the native item, use a manual C++ `QQuickItem` subclass plus Rust-owned scene/tile state behind narrow FFI. The hot path rule remains unchanged: no JSON geometry and no QML timeline primitive repeaters.
- The first implementation checkpoint is Task 4: a native item drawing ruler/playhead/markers without waveform detail. The second checkpoint is Task 7: waveform/analysis tiles swap without blocking scroll/playback.
