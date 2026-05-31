# Autolight Project Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the graph-backed timeline shell into a usable local project workflow where a user can create, open, save, import audio, add/run generated marker tracks, and derive editable cue tracks from the UI.

**Architecture:** Keep Python as the project and graph authority. `AppController` exposes small QML-safe commands, owns path/error/selection state, and delegates persistence, graph mutation, dependency hashing, and jobs to existing domain modules. QML remains a thin view over controller properties and `TimelineTrackModel`, with file dialogs and toolbar actions but no analysis logic.

**Tech Stack:** Python 3.14, PySide6/QML, `unittest`, JSON `.autolight` project files, existing `ProjectStore`, `track_dependency_hash`, `LocalJobQueue`, and deterministic built-in transforms.

---

## File Structure

- Modify `autolight/app_controller.py`: add project path, last error, selected track state, project file slots, import slot, generated-track slot, editable-track slot, and safer `run_track` error handling.
- Modify `UI/Main.qml`: add file dialogs, toolbar commands, selected-row handling, status/error text, and action enablement.
- Modify `tests/test_app_controller.py`: add controller unit tests for new/open/save/import/add-transform/derive/run error behavior and QML workflow wiring checks.
- Modify `README.md`: document the interactive app workflow and the existing test/smoke commands.

## Task 1: Controller Project File Workflow

**Files:**
- Modify: `autolight/app_controller.py`
- Modify: `tests/test_app_controller.py`

- [x] **Step 1: Add failing tests for new, import, save, and open**

Append these imports near the top of `tests/test_app_controller.py`:

```python
import tempfile
import wave
```

Add this helper near the test class:

```python
def write_wav(path: Path) -> None:
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(8000)
        handle.writeframes(b"\0\0" * 8000)
```

Add these test methods to `AppControllerTest`:

```python
    def test_new_project_resets_project_path_and_timeline_model(self):
        controller = self._controller()
        controller.load_demo_project()

        controller.new_project()

        self.assertEqual(controller.projectName, "Untitled")
        self.assertEqual(controller.projectPath, "")
        self.assertEqual(controller.lastError, "")
        self.assertEqual(controller.trackModel.rowCount(), 0)

    def test_import_audio_adds_source_track_and_selects_it(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path)
            track_id = controller.import_audio(str(audio_path))

        self.assertNotEqual(track_id, "")
        self.assertEqual(controller.trackModel.rowCount(), 1)
        self.assertEqual(controller.selectedTrackId, track_id)
        self.assertEqual(controller.lastError, "")

    def test_import_audio_records_error_for_missing_file(self):
        controller = self._controller()

        track_id = controller.import_audio("/missing/song.wav")

        self.assertEqual(track_id, "")
        self.assertIn("No such file", controller.lastError)
        self.assertEqual(controller.trackModel.rowCount(), 0)

    def test_save_and_open_project_round_trip_updates_path_and_model(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            audio_path = root / "song.wav"
            write_wav(audio_path)
            project_path = root / "show.autolight"
            controller.import_audio(str(audio_path))

            self.assertTrue(controller.save_project(str(project_path)))
            controller.new_project()
            self.assertTrue(controller.open_project(str(project_path)))

        self.assertEqual(controller.projectName, "Untitled")
        self.assertTrue(controller.projectPath.endswith("show.autolight"))
        self.assertEqual(controller.trackModel.rowCount(), 1)
        self.assertEqual(controller.lastError, "")

    def test_save_project_requires_path_for_unsaved_project(self):
        controller = self._controller()

        self.assertFalse(controller.save_project(""))
        self.assertIn("project path is required", controller.lastError)
```

- [x] **Step 2: Run the controller tests and verify they fail**

Run:

```bash
uv run python -m unittest tests.test_app_controller -v
```

Expected: FAIL with `AttributeError` for missing `new_project`, `projectPath`, `lastError`, `selectedTrackId`, `import_audio`, `save_project`, or `open_project`.

- [x] **Step 3: Add controller state, properties, and file workflow slots**

Update the imports in `autolight/app_controller.py`:

```python
from PySide6.QtCore import Property, QObject, QUrl, Signal, Slot

from autolight.project.store import (
    ProjectStore,
    add_generated_track,
    create_editable_track_from_markers,
    import_audio_asset,
    new_project,
)
```

Add these signals to `AppController`:

```python
    projectPathChanged = Signal()
    lastErrorChanged = Signal()
    selectedTrackIdChanged = Signal()
```

Add these fields in `__init__` after `self._project = new_project("Untitled")`:

```python
        self._project_path = ""
        self._last_error = ""
        self._selected_track_id = ""
```

Add these properties below `projectName`:

```python
    @Property(str, notify=projectPathChanged)
    def projectPath(self) -> str:
        return self._project_path

    @Property(str, notify=lastErrorChanged)
    def lastError(self) -> str:
        return self._last_error

    @Property(str, notify=selectedTrackIdChanged)
    def selectedTrackId(self) -> str:
        return self._selected_track_id
```

Add these slots above `load_demo_project`:

```python
    @Slot()
    def new_project(self) -> None:
        self._set_project(new_project("Untitled"))
        self._set_project_path("")
        self._set_selected_track_id("")
        self._set_last_error("")

    @Slot(str, result=bool)
    def open_project(self, path: str) -> bool:
        try:
            project_path = self._path_from_qml(path)
            self._set_project(ProjectStore.load(project_path))
            self._set_project_path(str(project_path))
            self._set_selected_track_id("")
            self._set_last_error("")
            return True
        except Exception as exc:
            self._set_last_error(str(exc))
            return False

    @Slot(str, result=bool)
    def save_project(self, path: str = "") -> bool:
        try:
            if not path and not self._project_path:
                raise ValueError("project path is required")
            project_path = self._path_from_qml(path) if path else Path(self._project_path)
            if project_path.suffix != ".autolight":
                project_path = project_path.with_suffix(".autolight")
            ProjectStore.save(self._project, project_path)
            self._set_project_path(str(project_path))
            self._set_last_error("")
            return True
        except Exception as exc:
            self._set_last_error(str(exc))
            return False

    @Slot(str, result=str)
    def import_audio(self, path: str) -> str:
        try:
            audio_path = self._path_from_qml(path)
            track = import_audio_asset(self._project, audio_path)
            self._track_model.set_project(self._project)
            self._set_selected_track_id(track.id)
            self._set_last_error("")
            return track.id
        except FileNotFoundError as exc:
            self._set_last_error(f"No such file: {exc}")
            return ""
        except Exception as exc:
            self._set_last_error(str(exc))
            return ""
```

Update `load_demo_project` so it uses the new helpers after creating the demo project:

```python
        self._set_project(new_project("Autolight Demo"))
        self._set_project_path("")
        source = import_audio_asset(self._project, demo_audio_path)
```

At the end of `load_demo_project`, replace `self._track_model.set_project(self._project)` with:

```python
        self._track_model.set_project(self._project)
        self._set_selected_track_id(source.id)
        self._set_last_error("")
```

Add these private helpers before `__del__`:

```python
    def _set_project(self, project) -> None:
        self._project = project
        self._track_model.set_project(self._project)
        self.projectNameChanged.emit()

    def _set_project_path(self, path: str) -> None:
        if self._project_path == path:
            return
        self._project_path = path
        self.projectPathChanged.emit()

    def _set_last_error(self, message: str) -> None:
        if self._last_error == message:
            return
        self._last_error = message
        self.lastErrorChanged.emit()

    def _set_selected_track_id(self, track_id: str) -> None:
        if self._selected_track_id == track_id:
            return
        self._selected_track_id = track_id
        self.selectedTrackIdChanged.emit()

    def _path_from_qml(self, value: str) -> Path:
        text = str(value)
        if text.startswith("file:"):
            return Path(QUrl(text).toLocalFile())
        return Path(text)
```

- [x] **Step 4: Run the controller tests and verify the project workflow passes**

Run:

```bash
uv run python -m unittest tests.test_app_controller -v
```

Expected: PASS for the new project workflow tests. Existing demo and smoke tests continue to pass.

- [x] **Step 5: Commit the project file workflow**

Run:

```bash
git add autolight/app_controller.py tests/test_app_controller.py
git commit -m "Add project file workflow controller slots"
```

Expected: commit succeeds.

## Task 2: Controller Graph Actions For Generated And Editable Tracks

**Files:**
- Modify: `autolight/app_controller.py`
- Modify: `tests/test_app_controller.py`

- [x] **Step 1: Add failing tests for selecting, adding transforms, deriving editable tracks, and run errors**

Add these imports to `tests/test_app_controller.py`:

```python
from autolight.project.models import ResultState
```

Add these test methods to `AppControllerTest`:

```python
    def test_select_track_updates_selected_track_id(self):
        controller = self._controller()
        controller.load_demo_project()
        second_track_id = self._track_id(controller, 1)

        controller.select_track(second_track_id)

        self.assertEqual(controller.selectedTrackId, second_track_id)

    def test_add_fixed_interval_track_uses_parent_and_selects_generated_track(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path)
            source_id = controller.import_audio(str(audio_path))
            generated_id = controller.add_fixed_interval_track(source_id, 2.0, 0.5)

        self.assertNotEqual(generated_id, "")
        self.assertEqual(controller.trackModel.rowCount(), 2)
        self.assertEqual(controller.selectedTrackId, generated_id)
        generated = next(track for track in controller._project.tracks if track.id == generated_id)
        self.assertEqual(generated.input_track_ids, [source_id])
        self.assertEqual(generated.transform_id, "markers.fixed_interval")
        self.assertEqual(generated.transform_params, {"duration": 2.0, "interval": 0.5})
        self.assertNotEqual(generated.dependency_hash, "")

    def test_run_track_records_error_for_non_transform_track(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path)
            source_id = controller.import_audio(str(audio_path))
            job_id = controller.run_track(source_id)

        self.assertEqual(job_id, "")
        self.assertIn("no transform", controller.lastError)

    def test_create_editable_track_from_generated_markers_selects_editable_track(self):
        controller = self._controller()
        controller.load_demo_project()
        generated_id = self._track_id(controller, 1)

        editable_id = controller.create_editable_track_from_track(generated_id)

        self.assertNotEqual(editable_id, "")
        self.assertEqual(controller.trackModel.rowCount(), 4)
        self.assertEqual(controller.selectedTrackId, editable_id)
        editable = next(track for track in controller._project.tracks if track.id == editable_id)
        self.assertEqual(editable.input_track_ids, [generated_id])
        self.assertEqual(editable.result_state, ResultState.COMPLETE)
```

Add this helper to `AppControllerTest`:

```python
    def _track_id(self, controller: AppController, row: int) -> str:
        model = controller.trackModel
        return model.data(model.index(row, 0), model.role_for_name("trackId"))
```

- [x] **Step 2: Run the controller tests and verify they fail**

Run:

```bash
uv run python -m unittest tests.test_app_controller -v
```

Expected: FAIL with `AttributeError` for missing `select_track`, `add_fixed_interval_track`, or `create_editable_track_from_track`, and with current `run_track` raising instead of recording `lastError`.

- [x] **Step 3: Implement controller graph action slots**

Add these imports to `autolight/app_controller.py`:

```python
from autolight.cache.keys import track_dependency_hash
from autolight.project.store import find_track
```

Add these slots above `run_track`:

```python
    @Slot(str)
    def select_track(self, track_id: str) -> None:
        if find_track(self._project, track_id) is None:
            self._set_last_error(f"track not found: {track_id}")
            return
        self._set_selected_track_id(track_id)
        self._set_last_error("")

    @Slot(str, float, float, result=str)
    def add_fixed_interval_track(self, parent_track_id: str, duration: float, interval: float) -> str:
        try:
            parent = find_track(self._project, parent_track_id)
            if parent is None:
                raise ValueError(f"parent track not found: {parent_track_id}")
            transform_id = "markers.fixed_interval"
            transform_version = "1"
            params = {"duration": float(duration), "interval": float(interval)}
            dependency_hash = track_dependency_hash(
                parent.cache_refs,
                transform_id,
                transform_version,
                params,
            )
            track = add_generated_track(
                self._project,
                parent_track_id=parent.id,
                name="Fixed Interval Markers",
                transform_id=transform_id,
                transform_params=params,
                transform_version=transform_version,
                output_schema="markers.v1",
                dependency_hash=dependency_hash,
            )
            self._track_model.set_project(self._project)
            self._set_selected_track_id(track.id)
            self._set_last_error("")
            return track.id
        except Exception as exc:
            self._set_last_error(str(exc))
            return ""

    @Slot(str, result=str)
    def create_editable_track_from_track(self, source_track_id: str) -> str:
        try:
            marker_ids = [
                marker.id for marker in self._project.markers if marker.track_id == source_track_id
            ]
            if not marker_ids:
                raise ValueError("source track has no markers")
            track = create_editable_track_from_markers(
                self._project,
                source_track_id,
                "Editable Cues",
                marker_ids,
            )
            self._track_model.set_project(self._project)
            self._set_selected_track_id(track.id)
            self._set_last_error("")
            return track.id
        except Exception as exc:
            self._set_last_error(str(exc))
            return ""
```

Replace `run_track` with an error-recording version:

```python
    @Slot(str, result=str)
    def run_track(self, track_id: str) -> str:
        try:
            job_id = self._job_queue.submit(self._project, track_id)
            self._set_last_error("")
            return job_id
        except Exception as exc:
            self._set_last_error(str(exc))
            return ""
```

- [x] **Step 4: Run the controller tests and verify graph actions pass**

Run:

```bash
uv run python -m unittest tests.test_app_controller -v
```

Expected: PASS for controller selection, transform creation, editable derivation, and run-error behavior.

- [x] **Step 5: Commit controller graph actions**

Run:

```bash
git add autolight/app_controller.py tests/test_app_controller.py
git commit -m "Add controller graph workflow actions"
```

Expected: commit succeeds.

## Task 3: QML File Dialogs, Selection, And Toolbar Actions

**Files:**
- Modify: `UI/Main.qml`
- Modify: `tests/test_app_controller.py`

- [ ] **Step 1: Add failing QML wiring checks**

Add this test method to `AppControllerTest`:

```python
    def test_qml_exposes_project_workflow_actions(self):
        qml = (Path(__file__).resolve().parents[1] / "UI" / "Main.qml").read_text(encoding="utf-8")

        self.assertIn("import QtQuick.Dialogs", qml)
        self.assertIn("id: openProjectDialog", qml)
        self.assertIn("id: saveProjectDialog", qml)
        self.assertIn("id: importAudioDialog", qml)
        self.assertIn("appController.new_project()", qml)
        self.assertIn("appController.open_project(String(selectedFile))", qml)
        self.assertIn("appController.save_project(String(selectedFile))", qml)
        self.assertIn("appController.import_audio(String(selectedFile))", qml)
        self.assertIn("appController.add_fixed_interval_track(appController.selectedTrackId, 8.0, 0.5)", qml)
        self.assertIn("appController.run_track(appController.selectedTrackId)", qml)
        self.assertIn("appController.create_editable_track_from_track(appController.selectedTrackId)", qml)
        self.assertIn("appController.select_track(trackId)", qml)
        self.assertIn("appController.lastError", qml)
```

- [ ] **Step 2: Run the QML wiring test and verify it fails**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_qml_exposes_project_workflow_actions -v
```

Expected: FAIL because `UI/Main.qml` does not yet include dialogs or workflow actions.

- [ ] **Step 3: Add dialogs and toolbar actions to QML**

Update the imports at the top of `UI/Main.qml`:

```qml
import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
```

Inside `Window`, before `ColumnLayout`, add the dialogs:

```qml
    FileDialog {
        id: openProjectDialog
        title: "Open Autolight Project"
        nameFilters: ["Autolight projects (*.autolight)"]
        fileMode: FileDialog.OpenFile
        onAccepted: appController.open_project(String(selectedFile))
    }

    FileDialog {
        id: saveProjectDialog
        title: "Save Autolight Project"
        nameFilters: ["Autolight projects (*.autolight)"]
        fileMode: FileDialog.SaveFile
        onAccepted: appController.save_project(String(selectedFile))
    }

    FileDialog {
        id: importAudioDialog
        title: "Import Audio"
        nameFilters: ["Audio files (*.wav *.mp3 *.flac *.aiff *.aif *.m4a)", "All files (*)"]
        fileMode: FileDialog.OpenFile
        onAccepted: appController.import_audio(String(selectedFile))
    }
```

Replace the current toolbar `Button` with these command buttons while keeping the project-name label and spacer:

```qml
                Button {
                    text: "New"
                    onClicked: appController.new_project()
                }

                Button {
                    text: "Open"
                    onClicked: openProjectDialog.open()
                }

                Button {
                    text: "Save"
                    onClicked: appController.projectPath.length > 0 ? appController.save_project("") : saveProjectDialog.open()
                }

                Button {
                    text: "Save As"
                    onClicked: saveProjectDialog.open()
                }

                Button {
                    text: "Import Audio"
                    onClicked: importAudioDialog.open()
                }

                Button {
                    text: "Add Markers"
                    enabled: appController.selectedTrackId.length > 0
                    onClicked: appController.add_fixed_interval_track(appController.selectedTrackId, 8.0, 0.5)
                }

                Button {
                    text: "Run"
                    enabled: appController.selectedTrackId.length > 0
                    onClicked: appController.run_track(appController.selectedTrackId)
                }

                Button {
                    text: "Derive Editable"
                    enabled: appController.selectedTrackId.length > 0
                    onClicked: appController.create_editable_track_from_track(appController.selectedTrackId)
                }

                Button {
                    text: "Load Demo"
                    onClicked: appController.load_demo_project()
                }
```

Add row selection to the `ListView` delegate by placing this `MouseArea` as the last child inside the left metadata `Rectangle`:

```qml
                    MouseArea {
                        anchors.fill: parent
                        acceptedButtons: Qt.LeftButton
                        onClicked: appController.select_track(trackId)
                    }
```

Place the same `MouseArea` as the last child inside the right timeline `Rectangle`:

```qml
                    MouseArea {
                        anchors.fill: parent
                        acceptedButtons: Qt.LeftButton
                        onClicked: appController.select_track(trackId)
                    }
```

Give selected rows a visible border by changing the left metadata `Rectangle` border color to:

```qml
                    border.color: appController.selectedTrackId === trackId ? "#facc15" : "#343842"
```

Change the right timeline `Rectangle` border color to:

```qml
                    border.color: appController.selectedTrackId === trackId ? "#facc15" : "#2f333d"
```

Add this status area below the `ListView`:

```qml
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 34
            color: "#111318"
            border.color: "#2f333d"

            Text {
                anchors.verticalCenter: parent.verticalCenter
                anchors.left: parent.left
                anchors.leftMargin: 12
                width: parent.width - 24
                text: appController.lastError.length > 0
                    ? appController.lastError
                    : (appController.projectPath.length > 0 ? appController.projectPath : "Unsaved project")
                color: appController.lastError.length > 0 ? "#f87171" : "#a1a1aa"
                elide: Text.ElideMiddle
                font.pixelSize: 12
            }
        }
```

- [ ] **Step 4: Run the QML wiring test and smoke check**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_qml_exposes_project_workflow_actions -v
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
```

Expected: unittest passes and the smoke command exits 0.

- [ ] **Step 5: Commit QML project workflow controls**

Run:

```bash
git add UI/Main.qml tests/test_app_controller.py
git commit -m "Add QML project workflow controls"
```

Expected: commit succeeds.

## Task 4: README Workflow Documentation

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Update README with the interactive workflow**

Replace the `Current Scope` section in `README.md` with:

```markdown
## Current Scope

- Create, open, and save `.autolight` project files.
- Import one local audio file into a project.
- Create graph-backed source, generated, and editable tracks.
- Run deterministic built-in transforms through a local background job queue.
- Persist project tracks, markers, provenance, job summaries, and cache references as JSON.
- Render project tracks and marker counts in a QML timeline shell.

## Basic Workflow

1. Launch the app with `uv run python main.py`.
2. Use `Import Audio` to add a local audio file as a source track.
3. Select the source track and choose `Add Markers` to create a generated fixed-interval marker track.
4. Select the generated marker track and choose `Run`.
5. Select a completed marker track and choose `Derive Editable` to create editable cue markers.
6. Use `Save` or `Save As` to write a `.autolight` project file.
7. Use `Open` to reload a saved project.
```

- [ ] **Step 2: Run README and diff checks**

Run:

```bash
git diff --check
```

Expected: command exits 0 with no whitespace errors.

- [ ] **Step 3: Commit README workflow docs**

Run:

```bash
git add README.md
git commit -m "Document project workflow"
```

Expected: commit succeeds.

## Task 5: Final Verification

**Files:**
- Verify: `autolight/app_controller.py`
- Verify: `UI/Main.qml`
- Verify: `tests/test_app_controller.py`
- Verify: `README.md`

- [ ] **Step 1: Run the full unit suite**

Run:

```bash
uv run python -m unittest discover -s tests -v
```

Expected: all tests pass.

- [ ] **Step 2: Run the headless QML smoke check**

Run:

```bash
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
```

Expected: command exits 0. A Qt font alias warning is acceptable if the process still exits 0.

- [ ] **Step 3: Check the final diff**

Run:

```bash
git diff --check
git status --short --branch
```

Expected: `git diff --check` exits 0. `git status` shows only intentional changes if commits were not created during task execution, or a clean worktree if each task was committed.

- [ ] **Step 4: Commit any remaining final adjustments**

If any final verification edits were made, run:

```bash
git add autolight/app_controller.py UI/Main.qml tests/test_app_controller.py README.md
git commit -m "Verify project workflow batch"
```

Expected: commit succeeds only if there are remaining staged changes; skip this command when `git status --short` is clean.
