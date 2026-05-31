# Autolight Cache Recovery And Rerun Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Detect invalid cached artifacts on project open, mark affected tracks stale, and let the user rerun stale/failed generated tracks from QML.

**Architecture:** Reuse `LocalJobQueue.refresh_cache_validity` for detection and wire it into controller open/refresh flows. Keep recovery explicit: invalid generated tracks stay visible with stale/error status until the user reruns them.

**Tech Stack:** Python 3.14, PySide6/QML, `unittest`, existing `CacheStore`, `LocalJobQueue`, `ProjectStore`, and `TimelineTrackModel`.

**Prerequisite:** Complete `2026-05-31-autolight-project-workflow.md` and `2026-05-31-autolight-job-progress-controls.md` first. The workflow plan introduces `open_project`; the job-progress plan owns the canonical `rerun_track` slot and the single toolbar `Rerun` button. This plan only adds cache validation, stale styling, and the `Check Cache` action.

---

## File Structure

- Modify `autolight/app_controller.py`: call cache validation on open and expose `refresh_cache_status`.
- Modify `UI/Main.qml`: show stale/error text and a `Check Cache` action while reusing the existing `Rerun` action from the job-progress-controls plan.
- Modify `tests/test_app_controller.py`: cover cache refresh behavior and QML wiring.
- Modify `README.md`: document cache recovery behavior.

## Task 1: Controller Cache Refresh

**Files:**
- Modify: `autolight/app_controller.py`
- Modify: `tests/test_app_controller.py`

- [x] **Step 1: Add failing controller cache refresh test**

Add this test to `AppControllerTest`:

```python
    def test_refresh_cache_status_marks_invalid_cached_track_stale(self):
        from autolight.project.models import CacheEntry, ResultState

        controller = self._controller()
        controller.load_demo_project()
        generated = controller._project.tracks[1]
        generated.result_state = ResultState.COMPLETE
        generated.cache_refs = ["missing_cache"]
        controller._project.cache_entries.append(
            CacheEntry(
                id="missing_cache",
                dependency_hash="dep",
                artifact_kind="stem",
                path="stem/missing.bin",
                created_at="",
                transform_version="1",
                size_bytes=10,
            )
        )

        invalid_refs = controller.refresh_cache_status()

        self.assertEqual(invalid_refs, ["missing_cache"])
        self.assertEqual(generated.result_state, ResultState.STALE)
        self.assertIn("cache artifact", generated.error)
```

- [x] **Step 2: Run cache refresh test and verify failure**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_refresh_cache_status_marks_invalid_cached_track_stale -v
```

Expected: FAIL because `refresh_cache_status` is missing.

- [x] **Step 3: Implement `refresh_cache_status`**

Add this slot to `autolight/app_controller.py`:

```python
    @Slot(result=list)
    def refresh_cache_status(self) -> list[str]:
        try:
            invalid_refs = self._job_queue.refresh_cache_validity(self._project)
            self._track_model.set_project(self._project)
            self._set_last_error("" if not invalid_refs else f"invalid cache artifacts: {len(invalid_refs)}")
            return invalid_refs
        except Exception as exc:
            self._set_last_error(str(exc))
            return []
```

When integrating it into `open_project`, clear a successful file-open error before refreshing cache status, and do not clear `lastError` after `refresh_cache_status()`:

```python
            self._set_last_error("")
            self.refresh_cache_status()
```

- [x] **Step 4: Run app controller tests**

Run:

```bash
uv run python -m unittest tests.test_app_controller -v
```

Expected: PASS.

- [x] **Step 5: Commit cache refresh controller**

Run:

```bash
git add autolight/app_controller.py tests/test_app_controller.py
git commit -m "Refresh cache validity from controller"
```

Expected: commit succeeds.

## Task 2: Stale Recovery UI

**Files:**
- Modify: `UI/Main.qml`
- Modify: `tests/test_app_controller.py`

- [x] **Step 1: Add failing QML stale recovery test**

Add this test:

```python
    def test_qml_exposes_cache_refresh_and_rerun_recovery(self):
        qml = (Path(__file__).resolve().parents[1] / "UI" / "Main.qml").read_text(encoding="utf-8")

        self.assertIn("appController.refresh_cache_status()", qml)
        self.assertIn("appController.rerun_track(appController.selectedTrackId)", qml)
        self.assertIn("resultState === \"stale\"", qml)
        self.assertIn("resultState === \"failed\"", qml)
```

- [x] **Step 2: Run QML stale recovery test and verify failure**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_qml_exposes_cache_refresh_and_rerun_recovery -v
```

Expected: FAIL until QML includes cache refresh and stale/failed styling. The `rerun_track` assertion verifies that the prerequisite job-progress plan's canonical `Rerun` button remains present; do not add a second `Rerun` button in this plan.

- [x] **Step 3: Add cache refresh and stale styling to QML**

Add a toolbar button:

```qml
                Button {
                    text: "Check Cache"
                    onClicked: appController.refresh_cache_status()
                }
```

Update the status text color expression in the track metadata area:

```qml
                            color: resultState === "failed" || resultState === "stale" ? "#f87171" : "#a1a1aa"
```

- [x] **Step 4: Run QML recovery tests and smoke**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_qml_exposes_cache_refresh_and_rerun_recovery -v
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
```

Expected: test passes and smoke exits 0.

- [x] **Step 5: Commit stale recovery UI**

Run:

```bash
git add UI/Main.qml tests/test_app_controller.py
git commit -m "Add cache recovery controls"
```

Expected: commit succeeds.

## Task 3: README Cache Recovery Notes

**Files:**
- Modify: `README.md`

- [x] **Step 1: Update README cache section**

Add this section to `README.md`:

```markdown
## Cache Recovery

Autolight records generated artifact metadata in the `.autolight` project file and stores artifact bytes under the app runtime cache. If a cached artifact is missing or corrupted, `Check Cache` marks affected generated tracks as `stale` while preserving visible markers and editable derived tracks. Select a stale or failed generated track and choose `Rerun` to regenerate its output.
```

- [x] **Step 2: Run diff check**

Run:

```bash
git diff --check
```

Expected: no output.

- [x] **Step 3: Commit README cache recovery docs**

Run:

```bash
git add README.md
git commit -m "Document cache recovery workflow"
```

Expected: commit succeeds.

## Final Verification

- [x] **Step 1: Run full tests and smoke**

Run:

```bash
uv run python -m unittest discover -s tests -v
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
```

Expected: tests pass and smoke exits 0.

- [x] **Step 2: Check diff**

Run:

```bash
git diff --check
git status --short --branch
```

Expected: no whitespace errors and only intentional changes remain.
