# Autolight Interactive Timeline Design

Date: 2026-06-01

## Context

Milestone 1 established the graph-backed project model, transform jobs, cache-aware artifacts, editable derived tracks, playback controls, timeline viewport state, and a usable QML shell. The next milestone should make Autolight feel like an authoring tool: users should be able to create cue tracks directly, edit markers on the timeline while listening, recover from mistakes, and inspect dense waveform detail at practical zoom levels.

The current implementation also needs structural and performance work before more interaction is added. `autolight/app_controller.py` is over one thousand lines and owns project lifecycle, transform actions, editing, playback, viewport state, waveform loading, and demo setup. `UI/Main.qml` is roughly nine hundred lines and owns dialogs, toolbars, playback, timeline rows, marker rendering, waveform rendering, and the inspector. Playback follow, scrubbing, and zooming currently trigger broad QML binding updates and can become slow or choppy while audio is playing.

## Goals

- Add blank manual cue tracks so a user can author cues without first deriving from generated markers.
- Support direct timeline editing for editable tracks: select, multi-select, drag to move, drag edges to resize, add at a clicked time or playhead, and delete selected cues.
- Keep the inspector as the precision editor for timestamp, duration, label, category, and color.
- Add non-persistent undo and redo for user marker and manual-track edits.
- Split large controller and QML files into focused modules without changing the public app behavior.
- Make playback, scrubbing, zooming, scrolling, and automatic playhead following smooth enough for interactive editing.
- Make waveform display zoom-adaptive so zooming in reveals more waveform detail instead of spreading a fixed set of bars apart.

## Non-Goals

- Lighting-console export is not part of this milestone.
- Multi-input graph transforms remain deferred.
- Full NLE feature parity is deferred: no lasso selection, copy/paste, split/merge, track grouping, clip lanes, or editable automation curves.
- Undo history is in-memory only and does not persist into `.autolight` project files.
- Real source separation remains outside this milestone unless needed for test fixtures.

## Architecture

The milestone keeps Python as the owner of project truth and uses QML as the interactive rendering layer. `AppController` remains the single stable object exposed to QML, but it should become a facade over smaller collaborators. This preserves existing QML integration while making each area easier to test.

Recommended Python units:

- `autolight/app/session.py`: new/open/save/import/demo lifecycle, project path, dirty state, and replacement checks.
- `autolight/app/marker_editing.py`: manual track creation, marker add/delete/update/move/resize, selection rules, snapping inputs, and downstream stale propagation.
- `autolight/app/edit_history.py`: undo/redo command stack with command objects for manual track and marker edits.
- `autolight/app/timeline_viewport.py`: zoom, scroll, visible duration, playhead follow policy, throttling, and clamping.
- `autolight/app/waveform_lod.py`: waveform artifact loading, visible-range slicing, and level selection from zoom.

`autolight/app_controller.py` should hold Qt signals, properties, and slots that delegate into these units. Existing public slot and property names should remain stable unless the implementation plan explicitly migrates tests and QML together.

Recommended QML units under `UI/components/`:

- `ProjectToolbar.qml`
- `TransformBar.qml`
- `PlaybackBar.qml`
- `TimelineRuler.qml`
- `TimelineView.qml`
- `TrackRow.qml`
- `TimelineLane.qml`
- `MarkerBlock.qml`
- `WaveformStrip.qml`
- `MarkerInspector.qml`
- `StatusFooter.qml`

`UI/Main.qml` should own the root window, dialogs, app-level constants, and component composition. It should not contain the full timeline implementation inline.

## Editing Model

Editable marker tracks come in two forms:

- Derived editable tracks, already supported, created from generated markers.
- Manual editable tracks, new in this milestone, created empty and associated with the selected source-audio context when possible.

Generated tracks stay read-only. Direct marker interactions are enabled only for editable tracks.

The timeline editing behavior:

- Clicking a marker selects it.
- Shift-click toggles a marker in the selection.
- Clicking an editable lane can place a new cue at that time when the add-cue action is active, or seek playback when normal seek mode is active.
- Dragging a selected marker body moves all selected markers by the same delta.
- Dragging a marker edge changes duration for that marker. Instant markers can become duration markers once resized.
- Delete removes selected markers from the selected editable track.
- Inspector edits remain available for exact numeric changes.

The first direct editing implementation should prefer clear behavior over dense tooling. It should not add lasso selection, copy/paste, or complex mode palettes in this milestone.

## Snapping

Snapping should help align cue edits with musically relevant timing while preserving free placement.

The first snapping implementation uses visible generated timing markers as snap targets, especially beats and onsets. It should consider markers on generated tracks that are complete or stale with visible markers. Snapping should operate within a small pixel threshold derived from the current zoom, with a default threshold of 8 screen pixels. Holding Alt/Option bypasses snapping for free movement.

The controller should expose enough data for QML to preview snapped positions consistently, but committed edits must be validated in Python before mutating project state.

## Undo And Redo

Undo and redo are required for marker editing safety. The stack is scoped to the current project session and clears when replacing projects. It is not saved in project files.

Undoable commands include:

- Create manual editable track.
- Add marker.
- Delete marker or selected markers.
- Move marker or selected markers.
- Resize marker.
- Inspector field edits.
- Bulk label, category, or color edits.

Drag operations should create one undoable command on release, not one command per pointer move. Commands should store enough before/after state to restore marker timestamps, durations, labels, categories, colors, selected marker IDs, and affected track state. Undo and redo should mark the project dirty and refresh relevant timeline models.

## Playback And Viewport Performance

Playback position changes should not force expensive timeline relayout. The design separates frequent playhead updates from less frequent viewport changes.

Expected behavior:

- The playhead updates smoothly while audio is playing.
- Automatic follow keeps the playhead visible while playing.
- Follow scroll adjusts only when the playhead enters the leading or trailing 20% viewport band, not on every position tick.
- Follow scroll updates are throttled to at most 30 times per second.
- Manual user scrolling temporarily disables or delays follow so the viewport does not fight the user.
- Scrubbing updates playback position immediately, but heavy timeline state updates are debounced or committed at slider release where appropriate.
- Zoom changes keep the chosen anchor time stable, ideally the playhead while loaded or the viewport center otherwise.

The QML timeline should avoid recalculating every marker and waveform item on each playhead tick. The lane should render content in a translated layer or batched drawing surface so scrolling moves a group instead of rebinding every child item. Markers may remain QML items if the visible count is small, but waveform rendering should move away from one rectangle per bucket when bucket counts grow.

## Waveform Level Of Detail

The current waveform summary stores one fixed bucket resolution. This makes zoomed-in waveforms less informative because bars are simply spaced farther apart.

Milestone 2 should store zoom-adaptive waveform data. The preferred artifact format is a waveform pyramid:

- Level 0: coarse overview for full-track display.
- Higher levels: progressively more buckets for detailed zoom.
- Each bucket stores peak and RMS values.
- The payload includes sample rate, duration, level metadata, and version.

The runtime chooses a waveform level from current pixels-per-second and visible duration, returns only the visible slice to QML, and lets the waveform component render it in batch form. Generated waveform artifacts should remain cache-backed and validated like current waveform artifacts. Older single-level waveform artifacts can be treated as readable legacy payloads during the transition, but new artifacts should use the multi-level format.

## Data Model

The existing project schema can support this milestone without a schema-version bump if marker duration, marker metadata, track provenance, and UI state remain sufficient.

Manual track creation should require an audio-backed editing context. If the selected track is a source track, use that track as the input. If the selected track is generated or editable, resolve its source-audio ancestor and use that source track as the input. If no source-audio ancestor exists, reject manual track creation with a clear error instead of creating a standalone track.

Manual tracks should use:

- `Track.type = editable`
- `Track.result_state = complete`
- `Track.input_track_ids` set to the resolved source track.
- `Track.provenance["created_by"] = "user"`
- `Track.provenance["manual_track"] = true`

Marker duration remains optional. A missing duration or zero duration means an instant cue. Positive duration means a region cue.

Undo history is not part of the project model.

Waveform artifact payloads should version their internal JSON independently from the `.autolight` project schema. Cache entries can continue to refer to waveform artifacts by cache ID and artifact kind.

## Error Handling

Generated tracks remain immutable from timeline editing actions. Attempts to edit generated markers should return a clear error and leave project state unchanged.

Marker edits must reject non-finite timestamps, negative timestamps, negative durations, and invalid color keys. Multi-marker moves should be atomic: if any target marker would become invalid, no selected marker should move.

Undo and redo should be no-ops with observable disabled state when the stack cannot move. Undo or redo failures should leave the current project state unchanged and set `lastError`.

Waveform LOD loading should tolerate missing or legacy payload fields by falling back to the best readable level or clearing waveform display for that track. Cache corruption should keep the existing stale/rerun behavior.

## Testing

Unit tests should cover:

- Manual editable track creation.
- Marker move and resize helpers, including atomic multi-marker move validation.
- Snap target selection and bypass behavior.
- Undo/redo command application and stack clearing on project replacement.
- Viewport follow throttling and edge hysteresis.
- Zoom anchoring.
- Waveform pyramid generation, level selection, visible slicing, and legacy payload fallback.
- Controller slots and properties for the new editing and history behaviors.

QML wiring tests should cover:

- Componentized `Main.qml` imports and uses the new timeline components.
- Generated tracks are not directly editable.
- Marker blocks expose drag and resize handles.
- Undo and redo buttons or shortcuts are wired.
- Timeline lane and waveform strip use batched or sliced data paths instead of rendering all waveform buckets as root-level delegates.

Performance checks should be pragmatic and deterministic:

- Add focused tests around throttling behavior in Python rather than timing-sensitive UI benchmarks.
- Keep the existing offscreen smoke and screenshot checks.
- Add a visual or structural check that zoomed-in waveform display uses a higher-detail level than zoomed-out display for the same audio.

## Milestone 2 Acceptance Criteria

- A user can create a blank manual cue track.
- A user can add, select, move, resize, delete, and bulk edit markers on editable tracks directly from the timeline.
- Generated tracks remain read-only.
- Undo and redo work for manual track and marker edits in the current session.
- `app_controller.py` and `UI/Main.qml` are reduced to facade/composition files with major responsibilities moved into focused Python and QML modules.
- Playback follow keeps the playhead visible while audio is playing without fighting manual scroll.
- Scrubbing, zooming, and scrolling avoid broad per-marker or per-waveform recomputation during playback.
- Waveform display becomes more detailed when zooming in and renders only the visible slice.
- Existing project workflow, transform, cache, playback, and UI smoke tests continue to pass.

## Approved Decisions

- Milestone 2 targets direct timeline editing as the primary product step.
- Blank manual cue tracks are in scope.
- Undo and redo are hard requirements.
- Timeline editing should be direct manipulation, with the inspector retained for precision.
- Snapping should use visible generated timing markers, with a modifier to bypass snapping.
- Component extraction, playback follow performance, and waveform LOD are part of the same milestone because they are prerequisites for comfortable direct editing.
