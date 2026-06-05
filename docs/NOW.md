# Autolight NOW

Updated: 2026-06-05

## Active Batch: Rust-Only Codebase Cleanup

**Status:** complete

**Goal:** Declutter the stacked Rust PR by removing the superseded runtime surface and historical migration notes now that the app is Rust/CXX-Qt only.

## Scope

- Remove the superseded runtime package, app entry points, dependency files, notebooks, screenshot helper, and old runtime tests.
- Remove legacy QML timeline components that are not bundled by the Rust app.
- Remove historical migration plan/spec documents that no longer drive execution.
- Simplify `Main.qml` so it always loads the Rust timeline view.
- Keep the active dispatcher docs, manual timeline hardening notes, fixtures, Rust crates, and current QML components.

## Target Paths

- `.gitignore`
- `AGENTS.md`
- `README.md`
- `UI/Main.qml`
- `UI/components/`
- `crates/autolight-qt/src/app_controller/tests.rs`
- `crates/autolight-core/src/transforms.rs`
- `docs/NOW.md`
- `docs/PROCESS.md`
- `docs/ROADMAP.md`
- removed runtime, notebook, test, dependency, and historical docs paths

## Verification

Passed:

```bash
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked repo_surface_is_rust_only_after_port_cleanup
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked current_docs_describe_rust_only_runtime
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_timeline_native_scrub_omits_label_width_with_rust_only_runtime
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-app --locked embedded_qml_bundle_contains_runtime_and_components
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked active_rust_timeline_removes_legacy_geometry_invokables
cargo fmt --all -- --check
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D clippy::perf
QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke
git diff --check
```

`cargo fmt --all -- --check` initially found only rustfmt wrapping in the new docs-surface test; `cargo fmt --all` applied it and the check then passed. The offscreen smoke loaded `UI/Main.qml` with `Autolight.Qt AppController` and emitted only known host audio/font warnings. The stale-reference `rg` check returned no matches.

## Handoff

Next: push this cleanup commit to the stacked PR branch and watch PR checks.
