# Native Timeline Risk Hardening Manual Gate

Date: 2026-06-05
Branch: codex/native-timeline-risk-hardening
Commit: 263d39a during real-window launch; Task 6 notes committed after the launch
Machine: macOS 26.3.1 (a) arm64
Qt: 6.11.1

## Harness Result

- `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo run -p autolight-app` built and launched the Rust app in a real macOS window.
- The terminal emitted only the known host audio-channel warnings before the app was stopped with Ctrl-C.
- Follow-up automation found and fixed a QML wrapper refresh bug where the native controller loaded
  the demo project but `AppRuntime` wrapper properties stayed on the initial smoke values.
- After the wrapper refresh fix, macOS Accessibility verified the real window title as
  `Autolight Rust Demo`; clicking `Play` advanced the visible playback label to `0:02 / 0:02`.
- Follow-up screenshot review found waveform geometry drawing into the track-information column
  during horizontal scroll. Native scene moving primitives now clip to the timeline lane origin.
- `screencapture -x /private/tmp/autolight-native-window.png` failed with `could not create image from display`, so the agent harness could not capture visual proof.
- Physical trackpad-only checks, pinch gesture feel, and 10-minute memory observation were not executable by this non-interactive harness. Keep those rows open for a human real-device pass.

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
| Real-window app launch | Built and launched with `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo run -p autolight-app`; no fatal runtime errors before Ctrl-C. | Pass |
| Demo startup state | macOS Accessibility read the real window title as `Autolight Rust Demo` after `AppRuntime` mirrored native qproperties through `reloadModels()`. | Pass |
| Playback source state | Clicking `Play` in the real window updated the visible playback label to `0:02 / 0:02`, proving the playback source/duration mirror is live. | Pass |
| Scrolled waveform label-column bleed | Fixed in native scene frame construction by clipping waveform, analysis, marker, ruler, and playhead geometry to `timelineLaneOriginX()`. | Pass by code regression |
| Screenshot capture | `screencapture -x /private/tmp/autolight-native-window.png` failed with `could not create image from display`. | Blocked by harness |
| 50-track snapshot load | Requires interactive fixture creation/import in the real app window. | Needs human pass |
| High-zoom playback follow | Code guard verified by `qml_follow_smoothing_is_disabled_during_native_viewport_gestures`; physical visual smoothness still requires trackpad/window observation. | Needs human pass |
| Pinch zoom while playing | Requires physical trackpad gesture; not synthesizable from this harness. | Needs human pass |
| Repeated waveform rerender | Covered by waveform budget/job regressions; repeated interactive rerender still requires a real-window pass. | Needs human pass |
| 10-minute memory stability | Requires long-running interactive observation or profiler session outside this harness. | Needs human pass |
