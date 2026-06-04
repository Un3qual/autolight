//! Autolight desktop application entry point.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use cxx_qt::casting::Upcast;
use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QQmlEngine, QString, QUrl};

struct EmbeddedQmlAsset {
    relative_path: &'static str,
    contents: &'static str,
}

fn main() -> ExitCode {
    match run(std::env::args().skip(1)) {
        Ok(status) => exit_code_from_qt_status(status),
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

fn run(args: impl IntoIterator<Item = String>) -> Result<i32, String> {
    let smoke = args.into_iter().any(|arg| arg == "--smoke");
    autolight_qt::init_qml_module();

    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();
    let qml_url = main_qml_url()?;
    let root_loaded = Arc::new(AtomicBool::new(false));
    let root_loaded_for_signal = Arc::clone(&root_loaded);

    let _object_created_guard = engine
        .as_mut()
        .ok_or_else(|| "failed to create QML engine".to_string())?
        .on_object_created(move |_, object, _| {
            root_loaded_for_signal.store(!object.is_null(), Ordering::SeqCst);
        });

    engine
        .as_mut()
        .ok_or_else(|| "failed to create QML engine".to_string())?
        .load(&qml_url);

    if smoke {
        if root_loaded.load(Ordering::SeqCst) {
            println!("Rust smoke loaded UI/Main.qml with Autolight.Qt AppController");
            return Ok(0);
        }
        return Err("QML root failed to load".to_string());
    }
    if !root_loaded.load(Ordering::SeqCst) {
        return Err("QML root failed to load".to_string());
    }

    let Some(engine) = engine.as_mut() else {
        return Err("failed to create QML engine".to_string());
    };
    let engine: core::pin::Pin<&mut QQmlEngine> = engine.upcast_pin();
    let _quit_guard = engine.on_quit(|_| {});

    let Some(app) = app.as_mut() else {
        return Err("failed to create Qt application".to_string());
    };
    Ok(app.exec())
}

fn main_qml_url() -> Result<QUrl, String> {
    let qml_path = prepare_embedded_qml_assets()?.join("Main.qml");
    let qml_path = qml_path
        .to_str()
        .ok_or_else(|| format!("QML path is not valid UTF-8: {}", qml_path.display()))?;

    Ok(QUrl::from_local_file(&QString::from(qml_path)))
}

fn prepare_embedded_qml_assets() -> Result<PathBuf, String> {
    let root = std::env::temp_dir().join(format!(
        "autolight-qml-assets-{}-{}",
        env!("CARGO_PKG_VERSION"),
        std::process::id()
    ));
    for asset in embedded_qml_assets() {
        let path = root.join(asset.relative_path);
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create QML asset directory {}: {error}",
                    parent.display()
                )
            })?;
        }
        std::fs::write(&path, asset.contents).map_err(|error| {
            format!(
                "failed to write embedded QML asset {}: {error}",
                path.display()
            )
        })?;
    }
    Ok(root)
}

fn embedded_qml_assets() -> &'static [EmbeddedQmlAsset] {
    &[
        EmbeddedQmlAsset {
            relative_path: "Main.qml",
            contents: include_str!("../../../UI/Main.qml"),
        },
        EmbeddedQmlAsset {
            relative_path: "AppRuntime.qml",
            contents: include_str!("../../../UI/AppRuntime.qml"),
        },
        EmbeddedQmlAsset {
            relative_path: "qmldir",
            contents: include_str!("../../../UI/qmldir"),
        },
        EmbeddedQmlAsset {
            relative_path: "components/AnalysisStrip.qml",
            contents: include_str!("../../../UI/components/AnalysisStrip.qml"),
        },
        EmbeddedQmlAsset {
            relative_path: "components/MarkerBlock.qml",
            contents: include_str!("../../../UI/components/MarkerBlock.qml"),
        },
        EmbeddedQmlAsset {
            relative_path: "components/MarkerInspector.qml",
            contents: include_str!("../../../UI/components/MarkerInspector.qml"),
        },
        EmbeddedQmlAsset {
            relative_path: "components/PlaybackBar.qml",
            contents: include_str!("../../../UI/components/PlaybackBar.qml"),
        },
        EmbeddedQmlAsset {
            relative_path: "components/ProjectToolbar.qml",
            contents: include_str!("../../../UI/components/ProjectToolbar.qml"),
        },
        EmbeddedQmlAsset {
            relative_path: "components/StatusFooter.qml",
            contents: include_str!("../../../UI/components/StatusFooter.qml"),
        },
        EmbeddedQmlAsset {
            relative_path: "components/TimelineLane.qml",
            contents: include_str!("../../../UI/components/TimelineLane.qml"),
        },
        EmbeddedQmlAsset {
            relative_path: "components/TimelineRuler.qml",
            contents: include_str!("../../../UI/components/TimelineRuler.qml"),
        },
        EmbeddedQmlAsset {
            relative_path: "components/TimelineView.qml",
            contents: include_str!("../../../UI/components/TimelineView.qml"),
        },
        EmbeddedQmlAsset {
            relative_path: "components/TrackRow.qml",
            contents: include_str!("../../../UI/components/TrackRow.qml"),
        },
        EmbeddedQmlAsset {
            relative_path: "components/TransformBar.qml",
            contents: include_str!("../../../UI/components/TransformBar.qml"),
        },
        EmbeddedQmlAsset {
            relative_path: "components/WaveformStrip.qml",
            contents: include_str!("../../../UI/components/WaveformStrip.qml"),
        },
    ]
}

fn exit_code_from_qt_status(status: i32) -> ExitCode {
    if status == 0 {
        return ExitCode::SUCCESS;
    }
    ExitCode::from(u8::try_from(status).unwrap_or(1))
}

#[cfg(test)]
mod tests {
    use super::{embedded_qml_assets, exit_code_from_qt_status};
    use std::process::ExitCode;

    #[test]
    fn exit_code_from_qt_status_preserves_zero_and_small_nonzero_statuses() {
        assert_eq!(exit_code_from_qt_status(0), ExitCode::SUCCESS);
        assert_eq!(exit_code_from_qt_status(3), ExitCode::from(3));
    }

    #[test]
    fn exit_code_from_qt_status_maps_unrepresentable_status_to_failure() {
        assert_eq!(exit_code_from_qt_status(-1), ExitCode::from(1));
        assert_eq!(exit_code_from_qt_status(300), ExitCode::from(1));
    }

    #[test]
    fn embedded_qml_bundle_contains_runtime_and_components() {
        let asset_names = embedded_qml_assets()
            .iter()
            .map(|asset| asset.relative_path)
            .collect::<Vec<_>>();

        assert!(asset_names.contains(&"Main.qml"));
        assert!(asset_names.contains(&"AppRuntime.qml"));
        assert!(asset_names.contains(&"components/TimelineView.qml"));
        assert!(asset_names.contains(&"components/WaveformStrip.qml"));
    }
}
