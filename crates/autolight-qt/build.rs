use cxx_qt_build::{CppFile, CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("Autolight.Qt"))
        .qt_module("Network")
        .qt_module("Quick")
        .files(["src/app_controller/mod.rs"])
        .cpp_file(CppFile::from("src/timeline_renderer/scene_graph.h").compile(false))
        .cpp_file("src/timeline_renderer/scene_graph.cpp")
        .cpp_file(CppFile::from("src/timeline_scene/timeline_scene_item.h").compile(false))
        .cpp_file("src/timeline_scene/timeline_scene_item.cpp")
        .build();
}
