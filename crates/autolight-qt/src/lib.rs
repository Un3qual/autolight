//! CXX-Qt bridge and QML-facing runtime models for the Autolight desktop app.

pub mod app_controller;
pub mod timeline_model;
pub mod transform_model;

pub fn init_qml_module() {
    cxx_qt::init_qml_module!("Autolight.Qt");
}
