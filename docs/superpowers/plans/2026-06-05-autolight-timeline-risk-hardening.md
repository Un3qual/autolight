# Autolight Timeline Risk Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the remaining diffray risk areas that are not already fixed by adding measurement, lifecycle hardening, source-file decomposition, memory budgeting, and a real-window macOS gesture gate for the native Rust/CXX-Qt timeline.

**Architecture:** Keep the active Rust runtime on `TimelineSceneItem`; do not revive QML Canvas/repeater paths or optimize retiring legacy JSON geometry paths. The main changes are measurable C++ scene-graph counters exposed through the Rust controller, a focused split of `timeline_scene_item.cpp` into parse/frame/node/input responsibilities, explicit waveform memory budgets, and documented gates for Python reference removal.

**Tech Stack:** Rust 2021, CXX-Qt, Qt Quick scene graph, QML, Cargo tests, clippy, offscreen smoke, real-window macOS manual verification.

---

## Files

- Modify: `docs/NOW.md` to point to this follow-up batch and record completion/verification.
- Modify: `crates/autolight-qt/build.rs` to compile any new C++ scene files.
- Modify: `crates/autolight-qt/src/timeline_scene/timeline_scene_item.cpp` to remove duplicated responsibilities after extraction.
- Modify: `crates/autolight-qt/src/timeline_scene/timeline_scene_item.h` only if new private helpers require declarations.
- Create: `crates/autolight-qt/src/timeline_scene/scene_snapshot_parser.h`
- Create: `crates/autolight-qt/src/timeline_scene/scene_snapshot_parser.cpp`
- Create: `crates/autolight-qt/src/timeline_scene/scene_frame_builder.h`
- Create: `crates/autolight-qt/src/timeline_scene/scene_frame_builder.cpp`
- Create: `crates/autolight-qt/src/timeline_scene/scene_graph_nodes.h`
- Create: `crates/autolight-qt/src/timeline_scene/scene_graph_nodes.cpp`
- Create: `crates/autolight-qt/src/timeline_scene/timeline_input.h`
- Create: `crates/autolight-qt/src/timeline_scene/timeline_input.cpp`
- Modify: `crates/autolight-qt/src/timeline_scene/perf.rs` for Rust-side performance snapshots and budget summaries.
- Modify: `crates/autolight-qt/src/app_controller/mod.rs` to expose perf snapshot JSON if needed.
- Modify: `crates/autolight-qt/src/app_controller/tests.rs` for structure, perf-contract, budget, and QML contract regressions.
- Modify: `crates/autolight-analysis/src/waveform.rs` for memory budget helpers around waveform LOD counts.
- Modify: `crates/autolight-qt/src/app_controller/jobs.rs` to clamp requested waveform detail against memory policy rather than only bucket count.
- Modify: `UI/AppRuntime.qml` only if real-window follow testing shows `SmoothedAnimation` conflicts with native follow.
- Modify: `UI/components/TimelineView.qml` only if real-window gesture testing shows the 220 ms native quiet period needs tuning.
- Create: `docs/manual-testing/native-timeline-risk-hardening.md` for the macOS real-window checklist and observed thresholds.

## Non-Goals

- Do not replace the legacy `renderTimelineWaveform` JSON bridge with a binary protocol in this batch. It is not active in the Rust `TimelineView.qml` path and should be deleted when the Python/reference path is removed.
- Do not invest in Python reference UI parity beyond keeping the existing reference smoke/tests green.
- Do not lower waveform detail blindly. Use a budget calculation first; only clamp when a project would exceed the explicit budget.

## Task 1: Establish Performance And Lifecycle Baselines

**Files:**
- Modify: `crates/autolight-qt/src/app_controller/tests.rs`
- Modify: `docs/manual-testing/native-timeline-risk-hardening.md`
- Modify: `docs/NOW.md`

- [x] **Step 1: Add a QML/static regression proving viewport-only changes do not reparse snapshots**

Add or tighten this test in `crates/autolight-qt/src/app_controller/tests.rs`:

```rust
#[test]
fn native_timeline_viewport_changes_do_not_reparse_scene_snapshot() {
    let scene_cpp = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/timeline_scene/timeline_scene_item.cpp"),
    )
    .unwrap();
    let update_paint_node_start = scene_cpp
        .find("QSGNode* TimelineSceneItem::updatePaintNode")
        .unwrap();
    let update_paint_node = &scene_cpp[update_paint_node_start..];

    assert!(scene_cpp.contains("m_snapshot->snapshot = parseSnapshot(m_sceneSnapshotJson);"));
    assert!(!update_paint_node.contains("parseSnapshot("));
    assert!(scene_cpp.contains("void TimelineSceneItem::setViewportScrollSeconds"));
    assert!(scene_cpp.contains("void TimelineSceneItem::setViewportPixelsPerSecond"));
    assert!(scene_cpp.contains("void TimelineSceneItem::setPlaybackPositionSeconds"));
}
```

- [x] **Step 2: Run the focused test**

Run:

```bash
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked native_timeline_viewport_changes_do_not_reparse_scene_snapshot
```

Expected: pass. If it fails, fix `TimelineSceneItem` before proceeding.

- [x] **Step 3: Create the manual testing checklist**

Create `docs/manual-testing/native-timeline-risk-hardening.md`:

```markdown
# Native Timeline Risk Hardening Manual Gate

Date:
Branch:
Commit:
Machine:
Qt:

## Fixture

- Open the Rust app with `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo run -p autolight-app`.
- Load the demo project.
- Import or create enough tracks to reach 50 visible/project tracks.
- Run waveform generation on at least one long source track.

## Pass Criteria

- Playback follow in Band mode keeps the playhead visible with no visible freezes.
- Playback follow in Center mode scrolls continuously and does not stall at high zoom.
- Horizontal two-finger trackpad pan follows natural macOS direction.
- Pinch zoom anchors near the pointer and does not resume follow mid-gesture.
- Zoom slider movement while playing does not block playback follow for more than one visible frame.
- Ruler drag scrubs continuously and releases cleanly.
- Long-session memory is stable after 10 minutes of playback and repeated zoom/pan.

## Measurements

| Scenario | Observation | Pass/Fail |
| --- | --- | --- |
| 50-track snapshot load | | |
| High-zoom playback follow | | |
| Pinch zoom while playing | | |
| Repeated waveform rerender | | |
| 10-minute memory stability | | |
```

- [x] **Step 4: Commit**

Run:

```bash
git add crates/autolight-qt/src/app_controller/tests.rs docs/manual-testing/native-timeline-risk-hardening.md docs/NOW.md
git commit -m "Document native timeline hardening gate"
```

## Task 2: Add Native Scene Timing Counters

**Files:**
- Modify: `crates/autolight-qt/src/timeline_scene/timeline_scene_item.h`
- Modify: `crates/autolight-qt/src/timeline_scene/timeline_scene_item.cpp`
- Modify: `crates/autolight-qt/src/app_controller/tests.rs`

- [x] **Step 1: Write a structure test for native timing fields**

Add this test:

```rust
#[test]
fn timeline_scene_item_exposes_native_timing_counters_for_manual_profiling() {
    let scene_header = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/timeline_scene/timeline_scene_item.h"),
    )
    .unwrap();
    let scene_cpp = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/timeline_scene/timeline_scene_item.cpp"),
    )
    .unwrap();

    assert!(scene_header.contains("sceneSnapshotParseCount"));
    assert!(scene_header.contains("worstSceneSnapshotParseMicros"));
    assert!(scene_header.contains("worstSceneGraphUpdateMicros"));
    assert!(scene_header.contains("textTextureCreateCount"));
    assert!(scene_cpp.contains("QElapsedTimer"));
    assert!(scene_cpp.contains("m_sceneSnapshotParseCount"));
    assert!(scene_cpp.contains("m_textTextureCreateCount"));
}
```

- [x] **Step 2: Run the failing test**

Run:

```bash
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_scene_item_exposes_native_timing_counters_for_manual_profiling
```

Expected: fail until the Q_PROPERTY counters exist.

- [x] **Step 3: Implement C++ counters**

In `timeline_scene_item.h`, add read-only properties and getters:

```cpp
Q_PROPERTY(qulonglong sceneSnapshotParseCount READ sceneSnapshotParseCount NOTIFY scenePerfCountersChanged)
Q_PROPERTY(qulonglong worstSceneSnapshotParseMicros READ worstSceneSnapshotParseMicros NOTIFY scenePerfCountersChanged)
Q_PROPERTY(qulonglong worstSceneGraphUpdateMicros READ worstSceneGraphUpdateMicros NOTIFY scenePerfCountersChanged)
Q_PROPERTY(qulonglong textTextureCreateCount READ textTextureCreateCount NOTIFY scenePerfCountersChanged)
```

Add private fields:

```cpp
qulonglong m_sceneSnapshotParseCount = 0;
qulonglong m_worstSceneSnapshotParseMicros = 0;
qulonglong m_worstSceneGraphUpdateMicros = 0;
qulonglong m_textTextureCreateCount = 0;
```

In `timeline_scene_item.cpp`, time `parseSnapshot` in `setSceneSnapshotJson`, time `updateRootNode` in `updatePaintNode`, and increment `m_textTextureCreateCount` only when `updateTextNode` creates a new texture. Use `QElapsedTimer`, `std::max`, and emit `scenePerfCountersChanged()` after counter mutations.

- [x] **Step 4: Run focused tests**

Run:

```bash
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_scene_item_exposes_native_timing_counters_for_manual_profiling
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_scene_item_draws_ruler_markers_selection_and_playhead_without_qml_repeaters
```

Expected: both pass.

- [x] **Step 5: Commit**

Run:

```bash
git add crates/autolight-qt/src/timeline_scene/timeline_scene_item.h crates/autolight-qt/src/timeline_scene/timeline_scene_item.cpp crates/autolight-qt/src/app_controller/tests.rs
git commit -m "Add native timeline scene timing counters"
```

## Task 3: Split `timeline_scene_item.cpp` By Responsibility

**Files:**
- Modify: `crates/autolight-qt/build.rs`
- Modify: `crates/autolight-qt/src/timeline_scene/timeline_scene_item.cpp`
- Create: `crates/autolight-qt/src/timeline_scene/scene_snapshot_parser.h`
- Create: `crates/autolight-qt/src/timeline_scene/scene_snapshot_parser.cpp`
- Create: `crates/autolight-qt/src/timeline_scene/scene_frame_builder.h`
- Create: `crates/autolight-qt/src/timeline_scene/scene_frame_builder.cpp`
- Create: `crates/autolight-qt/src/timeline_scene/scene_graph_nodes.h`
- Create: `crates/autolight-qt/src/timeline_scene/scene_graph_nodes.cpp`
- Create: `crates/autolight-qt/src/timeline_scene/timeline_input.h`
- Create: `crates/autolight-qt/src/timeline_scene/timeline_input.cpp`
- Modify: `crates/autolight-qt/src/app_controller/tests.rs`

- [x] **Step 1: Add a file-size and compile-registration regression**

Add:

```rust
#[test]
fn native_timeline_scene_cpp_is_split_into_focused_units() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let build_rs = std::fs::read_to_string(manifest_dir.join("build.rs")).unwrap();
    let scene_cpp = std::fs::read_to_string(
        manifest_dir.join("src/timeline_scene/timeline_scene_item.cpp"),
    )
    .unwrap();
    let required_files = [
        "scene_snapshot_parser.cpp",
        "scene_frame_builder.cpp",
        "scene_graph_nodes.cpp",
        "timeline_input.cpp",
    ];

    for file in required_files {
        assert!(build_rs.contains(file), "{file} must be compiled by cxx-qt-build");
    }
    assert!(
        scene_cpp.lines().count() < 650,
        "timeline_scene_item.cpp should stay as the QQuickItem shell"
    );
}
```

- [x] **Step 2: Run the failing test**

Run:

```bash
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked native_timeline_scene_cpp_is_split_into_focused_units
```

Expected: fail until the split is done.

- [x] **Step 3: Extract data types and parser**

Move `MarkerSpec`, `WaveformSampleSpec`, `AnalysisSampleSpec`, `TrackSpec`, `SceneSnapshot`, `finiteJsonNumber`, `parseColor`, `parseSnapshot`, and related parser-only helpers into `scene_snapshot_parser.h/.cpp`. Keep the API narrow:

```cpp
SceneSnapshot parseTimelineSceneSnapshot(const QString& sceneSnapshotJson);
```

- [x] **Step 4: Extract frame building**

Move `RectSpec`, `BandSpec`, `TextSpec`, `SceneFrameSpec`, `appendRulerTicks`, label building, row/lane/waveform/analysis/marker drawing specification, `secondsToX`, `rowIndexForY`, `markerRectForTrack`, and `buildSceneFrame` into `scene_frame_builder.h/.cpp`. Keep the API:

```cpp
SceneFrameSpec buildTimelineSceneFrame(
  const SceneSnapshot& snapshot,
  double scrollSeconds,
  double pixelsPerSecond,
  double visibleSeconds,
  double playbackPositionSeconds,
  double trackScrollPixels,
  int selectedTrackIndex,
  double width,
  double height);
```

- [x] **Step 5: Extract QSG node updating**

Move `TextTextureNode`, `updateTextNode`, `updateTextNodes`, `updateBandNode`, `trimChildNodes`, and `updateRootNode` into `scene_graph_nodes.h/.cpp`. Return a small stats struct so Task 2 counters keep working:

```cpp
struct SceneGraphUpdateStats {
  qulonglong textTexturesCreated = 0;
};

QSGNode* updateTimelineSceneGraph(
  QSGNode* root,
  const SceneFrameSpec& frame,
  QQuickWindow* window,
  SceneGraphUpdateStats* stats);
```

- [x] **Step 6: Extract input math**

Move wheel constants, lane origin helpers, scrub second conversion, modifier handling, and row hit-testing helpers into `timeline_input.h/.cpp`. Keep event methods in `TimelineSceneItem`, but delegate pure calculations to functions that can be unit-tested later:

```cpp
double timelineSecondsForPosition(double x, double scrollSeconds, double pixelsPerSecond, const SceneSnapshot& snapshot);
double timelineHorizontalScrollDelta(const QWheelEvent& event);
double timelineVerticalScrollDelta(const QWheelEvent& event);
double timelineZoomFactor(const QWheelEvent& event);
```

- [x] **Step 7: Register new C++ files**

Update `crates/autolight-qt/build.rs` so CXX-Qt compiles all new `.cpp` files next to `timeline_scene_item.cpp` and `scene_graph.cpp`.

- [x] **Step 8: Run focused and full Qt tests**

Run:

```bash
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked native_timeline_scene_cpp_is_split_into_focused_units
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_scene_item_
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_timeline_
```

Expected: all pass.

- [x] **Step 9: Commit**

Run:

```bash
git add crates/autolight-qt/build.rs crates/autolight-qt/src/timeline_scene crates/autolight-qt/src/app_controller/tests.rs
git commit -m "Split native timeline scene item internals"
```

## Task 4: Add Waveform Memory Budgeting

**Files:**
- Modify: `crates/autolight-analysis/src/waveform.rs`
- Modify: `crates/autolight-qt/src/app_controller/jobs.rs`
- Modify: `crates/autolight-qt/src/app_controller/tests.rs`

- [x] **Step 1: Add tests for budgeted LOD counts**

Actual implementation note: budget enforcement stayed in `autolight-analysis` after frame count is
known. `waveform_bucket_param` tests only preserve default/clamp behavior; `maxBytes` is parsed by a
separate helper.

In `crates/autolight-analysis/src/waveform.rs` tests, add:

```rust
#[test]
fn waveform_level_counts_respect_memory_budget() {
    let counts = waveform_level_bucket_counts_for_budget(4_096, 1_000_000, 1_000_000);
    let estimated_samples: usize = counts.iter().sum();

    assert!(estimated_samples <= 1_000_000 / std::mem::size_of::<WaveformSample>());
    assert_eq!(counts.first(), Some(&4_096));
}
```

In `crates/autolight-qt/src/app_controller/jobs.rs` tests, add:

```rust
#[test]
fn waveform_bucket_param_clamps_to_memory_budget_and_max_lod() {
    let params = serde_json::json!({
        "buckets": MAX_WAVEFORM_LOD_BUCKETS * 2,
        "maxBytes": 1024 * 1024
    });

    let buckets = waveform_bucket_param(&params).unwrap();

    assert!(buckets <= MAX_WAVEFORM_LOD_BUCKETS);
    assert!(buckets >= DEFAULT_WAVEFORM_BUCKETS);
}
```

- [x] **Step 2: Run failing tests**

Run:

```bash
cargo test -p autolight-analysis --locked waveform_level_counts_
cargo test -p autolight-analysis --locked waveform_payload_build_uses_budgeted_lod_counts
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked waveform_max_bytes_param
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked waveform_bucket_param
```

Expected: fail until helper/policy exists.

- [x] **Step 3: Implement budget helper**

Add this public helper near `waveform_level_bucket_counts`:

```rust
pub fn waveform_level_bucket_counts_for_budget(
    base_bucket_count: usize,
    frame_count: usize,
    max_bytes: usize,
) -> Vec<usize> {
    let bytes_per_sample = std::mem::size_of::<WaveformSample>().max(1);
    let max_samples = (max_bytes / bytes_per_sample).max(base_bucket_count.max(1));
    let mut counts = waveform_level_bucket_counts(base_bucket_count, frame_count);
    while counts.iter().sum::<usize>() > max_samples && counts.len() > 1 {
        counts.pop();
    }
    counts
}
```

Then route waveform payload building through this helper if a budget is passed from the job params. Keep the default behavior equivalent for normal demo/default jobs.

- [x] **Step 4: Run focused tests**

Run:

```bash
cargo test -p autolight-analysis --locked waveform_level_counts_
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked waveform_bucket_param
```

Expected: pass.

- [x] **Step 5: Commit**

Run:

```bash
git add crates/autolight-analysis/src/waveform.rs crates/autolight-qt/src/app_controller/jobs.rs crates/autolight-qt/src/app_controller/tests.rs
git commit -m "Budget waveform LOD memory"
```

## Task 5: Fence Legacy JSON Geometry And Python Reference Drift

**Files:**
- Modify: `crates/autolight-qt/src/app_controller/tests.rs`
- Modify: `UI/components/LegacyTimelineView.qml`
- Modify: `UI/components/TimelineLane.qml`
- Modify: `docs/NOW.md`

- [x] **Step 1: Add a regression proving legacy geometry is not used by the active Rust timeline**

Add:

```rust
#[test]
fn active_rust_timeline_does_not_use_legacy_geometry_invokables() {
    let timeline_view_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../UI/components/TimelineView.qml"),
    )
    .unwrap();
    let legacy_view_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../UI/components/LegacyTimelineView.qml"),
    )
    .unwrap();
    let lane_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../UI/components/TimelineLane.qml"),
    )
    .unwrap();

    assert!(timeline_view_qml.contains("TimelineSceneItem"));
    assert!(!timeline_view_qml.contains("renderTimelineWaveform"));
    assert!(!timeline_view_qml.contains("TimelineWaveformItem"));
    assert!(legacy_view_qml.contains("TrackRow"));
    assert!(lane_qml.contains("renderTimelineWaveform"));
}
```

- [x] **Step 2: Add comments at the legacy boundary**

Add a short comment at the top of `LegacyTimelineView.qml` and near the retained legacy call in `TimelineLane.qml`:

```qml
// Reference-only Python timeline path. The Rust runtime uses TimelineSceneItem
// in TimelineView.qml; do not optimize this path for new Rust timeline work.
```

- [x] **Step 3: Run the focused test**

Run:

```bash
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked active_rust_timeline_does_not_use_legacy_geometry_invokables
```

Expected: pass.

- [x] **Step 4: Commit**

Run:

```bash
git add crates/autolight-qt/src/app_controller/tests.rs UI/components/LegacyTimelineView.qml UI/components/TimelineLane.qml docs/NOW.md
git commit -m "Fence legacy timeline geometry path"
```

## Task 6: Real-Window Gesture And Follow Pass

**Files:**
- Modify if needed: `UI/AppRuntime.qml`
- Modify if needed: `UI/components/TimelineView.qml`
- Modify if needed: `crates/autolight-qt/src/app_controller/tests.rs`
- Modify: `docs/manual-testing/native-timeline-risk-hardening.md`
- Modify: `docs/NOW.md`

- [ ] **Step 1: Run the app in a real window**

Run:

```bash
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo run -p autolight-app
```

Expected: the Rust app opens with the demo project and visible native timeline tracks.

- [ ] **Step 2: Execute the manual checklist**

Fill in `docs/manual-testing/native-timeline-risk-hardening.md` for:

- 50-track snapshot load
- high-zoom playback follow in Band and Center modes
- two-finger horizontal pan
- pinch zoom while playing
- zoom slider while playing
- ruler scrub
- 10-minute memory stability

- [ ] **Step 3: If follow animation conflicts with rapid programmatic updates, replace it with native-follow-only smoothing**

Only if the manual gate fails because `Behavior on timelineScrollSeconds` lags or fights manual interaction, add a failing QML structure test that asserts follow smoothing is disabled during native viewport gestures and then tune the existing guards:

```rust
#[test]
fn qml_follow_smoothing_is_disabled_during_native_viewport_gestures() {
    let app_runtime_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../UI/AppRuntime.qml"),
    )
    .unwrap();
    let timeline_view_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../UI/components/TimelineView.qml"),
    )
    .unwrap();

    assert!(app_runtime_qml.contains("!timelineUserNavigationActive"));
    assert!(timeline_view_qml.contains("begin_timeline_user_navigation()"));
    assert!(timeline_view_qml.contains("end_timeline_user_navigation()"));
}
```

Run:

```bash
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_follow_smoothing_is_disabled_during_native_viewport_gestures
```

Expected: pass after any tuning.

- [ ] **Step 4: If the 220 ms quiet period feels wrong, tune with a named constant and test**

Only if manual testing shows the timer is too sluggish or premature, replace the literal in `TimelineView.qml` with:

```qml
readonly property int nativeViewportGestureQuietMillis: 160
```

and:

```qml
interval: timelineRoot.nativeViewportGestureQuietMillis
```

Add a test that asserts the named property exists and no raw `interval: 220` remains in `TimelineView.qml`.

- [ ] **Step 5: Commit manual-gate fixes and notes**

Run:

```bash
git add UI/AppRuntime.qml UI/components/TimelineView.qml crates/autolight-qt/src/app_controller/tests.rs docs/manual-testing/native-timeline-risk-hardening.md docs/NOW.md
git commit -m "Validate native timeline real-window feel"
```

## Task 7: Final Verification And PR Follow-Through

**Files:**
- Modify: `docs/NOW.md`

- [ ] **Step 1: Run formatting and full automated checks**

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

Expected: all pass; smoke may emit known host audio/font warnings.

- [ ] **Step 2: Update `docs/NOW.md`**

Record:

- timing counter implementation;
- scene item split;
- waveform memory budget result;
- legacy path fencing;
- real-window manual results;
- exact verification commands and outcomes;
- next batch.

- [ ] **Step 3: Commit docs if needed**

Run:

```bash
git add docs/NOW.md docs/manual-testing/native-timeline-risk-hardening.md
git commit -m "Record native timeline hardening results"
```

Skip this commit if those files were already committed in Task 6 with complete results.

- [ ] **Step 4: Push**

Run:

```bash
git push
```

- [ ] **Step 5: Refresh PR bots**

Run the PR comment/status refresh and address any new unresolved bot comments before final handoff:

```bash
gh api repos/Un3qual/autolight/commits/HEAD/status
gh api repos/Un3qual/autolight/commits/HEAD/check-runs
```

If DeepSource fails with no inline comment, open the status target URL and inspect the hidden run payload before concluding the branch is clean.

## Completion Criteria

- Native scene timing counters are visible to QML/manual testing.
- `timeline_scene_item.cpp` is reduced to the QQuickItem shell and delegates parser/frame/node/input work to focused files.
- Active Rust timeline still has no QML repeater/Canvas/legacy geometry hot path.
- Waveform LOD memory has an explicit budget policy and tests.
- Legacy Python/reference timeline is fenced and documented as reference-only.
- Real-window macOS playback/trackpad gate is recorded.
- Full Cargo/clippy/smoke verification passes.
