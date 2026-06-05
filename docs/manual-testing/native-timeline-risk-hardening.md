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
