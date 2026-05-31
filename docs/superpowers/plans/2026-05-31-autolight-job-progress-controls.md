# Autolight Job Progress Controls Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose running job progress, cancel, and rerun controls in the controller, timeline model, and QML shell.

**Architecture:** `LocalJobQueue` already stores `JobRun.progress` and cancellation state. Add a QML-friendly job summary role to `TimelineTrackModel`, controller slots for cancel/rerun, and toolbar/status UI that acts on the selected track.

**Tech Stack:** Python 3.14, PySide6/QML, `unittest`, existing `LocalJobQueue`, `TimelineTrackModel`, and `AppController`.

---

## File Structure

- Modify `autolight/timeline/model.py`: add `jobState`, `jobProgress`, and `activeJobId` roles.
- Modify `autolight/app_controller.py`: track selected job, expose `cancel_selected_job`, and add `rerun_track`.
- Modify `UI/Main.qml`: show progress and Cancel/Rerun buttons for selected/running tracks.
- Modify `tests/test_timeline_model.py`: cover job roles.
- Modify `tests/test_app_controller.py`: cover controller cancel/rerun and QML wiring.

## Task 1: Timeline Job Roles

**Files:**
- Modify: `autolight/timeline/model.py`
- Modify: `tests/test_timeline_model.py`

- [ ] **Step 1: Add failing timeline job role test**

Add this test to `TimelineTrackModelTest`:

```python
    def test_model_exposes_latest_job_state_progress_and_id(self):
        project = ProjectDocument(id="project_1", name="Demo")
        track = Track(id="track_1", type=TrackType.GENERATED, name="Beats")
        project.tracks.append(track)
        project.job_runs.append(
            JobRun(
                id="job_1",
                track_id="track_1",
                transform_id="markers.fixed_interval",
                parameters_hash="hash",
                state=ResultState.RUNNING,
                progress=0.25,
            )
        )
        model = TimelineTrackModel()
        model.set_project(project)

        index = model.index(0, 0)

        self.assertEqual(model.data(index, model.role_for_name("activeJobId")), "job_1")
        self.assertEqual(model.data(index, model.role_for_name("jobState")), "running")
        self.assertEqual(model.data(index, model.role_for_name("jobProgress")), 0.25)
```

Ensure these imports exist in `tests/test_timeline_model.py`:

```python
from autolight.project.models import JobRun, ProjectDocument, ResultState, Track, TrackType
```

- [ ] **Step 2: Run timeline model tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_timeline_model -v
```

Expected: FAIL with `KeyError: 'activeJobId'`.

- [ ] **Step 3: Add job roles to `TimelineTrackModel`**

Extend `ROLE_NAMES` in `autolight/timeline/model.py`:

```python
        Qt.ItemDataRole.UserRole + 8: b"activeJobId",
        Qt.ItemDataRole.UserRole + 9: b"jobState",
        Qt.ItemDataRole.UserRole + 10: b"jobProgress",
```

Add these role branches in `data` after the `error` role:

```python
        latest_job = self._latest_job_for_track(track.id)
        if role == self.role_for_name("activeJobId"):
            return "" if latest_job is None or latest_job.state.value != "running" else latest_job.id
        if role == self.role_for_name("jobState"):
            return "" if latest_job is None else latest_job.state.value
        if role == self.role_for_name("jobProgress"):
            return 0.0 if latest_job is None else latest_job.progress
```

Add this helper below `_markers_for_track`:

```python
    def _latest_job_for_track(self, track_id: str):
        if self._project is None:
            return None
        jobs = [run for run in self._project.job_runs if run.track_id == track_id]
        return jobs[-1] if jobs else None
```

- [ ] **Step 4: Run timeline model tests**

Run:

```bash
uv run python -m unittest tests.test_timeline_model -v
```

Expected: PASS.

- [ ] **Step 5: Commit timeline job roles**

Run:

```bash
git add autolight/timeline/model.py tests/test_timeline_model.py
git commit -m "Expose job progress roles in timeline model"
```

Expected: commit succeeds.

## Task 2: Controller Cancel And Rerun Slots

**Files:**
- Modify: `autolight/app_controller.py`
- Modify: `tests/test_app_controller.py`

- [ ] **Step 1: Add failing controller job-control tests**

Add this test to `AppControllerTest`:

```python
    def test_cancel_selected_job_cancels_running_track(self):
        from autolight.project.store import add_generated_track

        controller = self._controller()
        controller.load_demo_project()
        source_id = self._track_id(controller, 0)
        stem = add_generated_track(
            controller._project,
            source_id,
            "Vocals Stem",
            "stems.vocals_stand_in",
            {"label": "vocals"},
            "1",
            "artifact.stem.v1",
            "stem_dependency",
        )
        controller.trackModel.set_project(controller._project)
        controller.select_track(stem.id)

        job_id = controller.run_track(stem.id)
        self.assertNotEqual(job_id, "")
        controller.cancel_selected_job()
        controller._job_queue.wait(job_id, timeout=2)

        self.assertEqual(stem.result_state.value, "cancelled")

    def test_rerun_track_submits_existing_transform(self):
        controller = self._controller()
        controller.load_demo_project()
        generated_id = self._track_id(controller, 1)

        job_id = controller.rerun_track(generated_id)
        controller._job_queue.wait(job_id, timeout=2)

        self.assertNotEqual(job_id, "")
        generated = next(track for track in controller._project.tracks if track.id == generated_id)
        self.assertEqual(generated.result_state.value, "complete")
```

- [ ] **Step 2: Run controller job-control tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_cancel_selected_job_cancels_running_track tests.test_app_controller.AppControllerTest.test_rerun_track_submits_existing_transform -v
```

Expected: FAIL because `cancel_selected_job` and `rerun_track` are missing.

- [ ] **Step 3: Implement controller job-control slots**

Add this helper to `autolight/app_controller.py`:

```python
    def _active_job_id_for_track(self, track_id: str) -> str:
        for run in reversed(self._project.job_runs):
            if run.track_id == track_id and run.state == ResultState.RUNNING:
                return run.id
        return ""
```

Add these slots above `cancel_job`:

```python
    @Slot()
    def cancel_selected_job(self) -> None:
        job_id = self._active_job_id_for_track(self._selected_track_id)
        if not job_id:
            self._set_last_error("selected track has no running job")
            return
        self.cancel_job(job_id)
        self._set_last_error("")

    @Slot(str, result=str)
    def rerun_track(self, track_id: str) -> str:
        track = find_track(self._project, track_id)
        if track is None:
            self._set_last_error(f"track not found: {track_id}")
            return ""
        track.error = ""
        return self.run_track(track_id)
```

- [ ] **Step 4: Run controller tests**

Run:

```bash
uv run python -m unittest tests.test_app_controller -v
```

Expected: PASS.

- [ ] **Step 5: Commit controller job controls**

Run:

```bash
git add autolight/app_controller.py tests/test_app_controller.py
git commit -m "Add job cancel and rerun controller actions"
```

Expected: commit succeeds.

## Task 3: QML Progress And Job Buttons

**Files:**
- Modify: `UI/Main.qml`
- Modify: `tests/test_app_controller.py`

- [ ] **Step 1: Add failing QML wiring test**

Add this test to `AppControllerTest`:

```python
    def test_qml_exposes_job_progress_controls(self):
        qml = (Path(__file__).resolve().parents[1] / "UI" / "Main.qml").read_text(encoding="utf-8")

        self.assertIn("jobProgress", qml)
        self.assertIn("activeJobId", qml)
        self.assertIn("ProgressBar", qml)
        self.assertIn("appController.cancel_selected_job()", qml)
        self.assertIn("appController.rerun_track(appController.selectedTrackId)", qml)
```

- [ ] **Step 2: Run QML wiring test and verify failure**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_qml_exposes_job_progress_controls -v
```

Expected: FAIL because the QML does not yet reference job roles or job actions.

- [ ] **Step 3: Add QML progress and buttons**

Add these toolbar buttons near the existing Run button:

```qml
                Button {
                    text: "Cancel"
                    enabled: appController.selectedTrackId.length > 0
                    onClicked: appController.cancel_selected_job()
                }

                Button {
                    text: "Rerun"
                    enabled: appController.selectedTrackId.length > 0
                    onClicked: appController.rerun_track(appController.selectedTrackId)
                }
```

Add this `ProgressBar` inside the delegate's left metadata column below the status text:

```qml
                        ProgressBar {
                            width: parent.width
                            from: 0
                            to: 1
                            value: jobProgress
                            visible: activeJobId.length > 0
                        }
```

- [ ] **Step 4: Run QML wiring and smoke checks**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_qml_exposes_job_progress_controls -v
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
```

Expected: unittest passes and smoke exits 0.

- [ ] **Step 5: Commit QML job controls**

Run:

```bash
git add UI/Main.qml tests/test_app_controller.py
git commit -m "Show job progress controls in QML"
```

Expected: commit succeeds.

## Final Verification

- [ ] **Step 1: Run full tests**

Run:

```bash
uv run python -m unittest discover -s tests -v
```

Expected: all tests pass.

- [ ] **Step 2: Run smoke**

Run:

```bash
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
```

Expected: exits 0.

- [ ] **Step 3: Check diff**

Run:

```bash
git diff --check
git status --short --branch
```

Expected: no whitespace errors; only intentional changes remain.
