use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("Autolight.Qt"))
        .qt_module("Network")
        .files(["src/app_controller/mod.rs"])
        .build();
}
