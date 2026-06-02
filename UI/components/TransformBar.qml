import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ColumnLayout {
    id: root
    property var appController
    property int compactButtonHeight: 30
    property color controlTextColor: "#f4f4f5"
    property color controlMutedTextColor: "#a1a1aa"
    property color secondaryText: "#d4d4d8"
    property color textMuted: "#a1a1aa"
    signal addMarkersRequested()
    signal runRequested()
    signal rerunRequested()
    signal cancelRequested()
    signal addTransformRequested(var transformId, var transformVersion, string params)
    signal addVocalsStemRequested()
    signal refreshCacheRequested()
    signal deriveEditableRequested()

    RowLayout {
        id: transformActions
        Layout.fillWidth: true
        Layout.leftMargin: 12
        Layout.rightMargin: 12
        Layout.topMargin: 6
        spacing: 6

        Button {
            text: "Add Markers"
            implicitHeight: root.compactButtonHeight
            palette.buttonText: root.controlTextColor
            enabled: root.appController.selectedTrackId.length > 0
            onClicked: root.addMarkersRequested()
        }
        Button {
            text: "Run"
            implicitHeight: root.compactButtonHeight
            palette.buttonText: root.controlTextColor
            enabled: root.appController.selectedTrackCanRerun && !root.appController.selectedTrackHasRunningJob
            onClicked: root.runRequested()
        }
        Button {
            text: "Rerun"
            implicitHeight: root.compactButtonHeight
            palette.buttonText: root.controlTextColor
            enabled: root.appController.selectedTrackCanRerun && !root.appController.selectedTrackHasRunningJob
            onClicked: root.rerunRequested()
        }
        Button {
            text: "Cancel"
            implicitHeight: root.compactButtonHeight
            palette.buttonText: root.controlTextColor
            enabled: root.appController.selectedTrackHasRunningJob
            onClicked: root.cancelRequested()
        }
        Item { Layout.fillWidth: true }
    }

    RowLayout {
        id: transformDetailBar
        Layout.fillWidth: true
        Layout.leftMargin: 12
        Layout.rightMargin: 12
        Layout.topMargin: 6
        Layout.bottomMargin: 6
        spacing: 8

        ComboBox {
            id: transformPicker
            model: root.appController.transformModel
            textRole: "name"
            valueRole: "transformId"
            Layout.preferredWidth: 190
            palette.text: root.controlTextColor
            palette.buttonText: root.controlTextColor
        }

        TextField {
            id: transformParamsField
            text: "{\"duration\": 8.0, \"interval\": 0.5}"
            placeholderText: "JSON params"
            Layout.preferredWidth: 210
            color: root.controlTextColor
            placeholderTextColor: root.controlMutedTextColor
            selectedTextColor: root.controlTextColor
            selectionColor: "#2563eb"
        }

        Button {
            text: "Add Transform"
            palette.buttonText: root.controlTextColor
            enabled: root.appController.selectedTrackId.length > 0 && transformPicker.currentIndex >= 0
            onClicked: root.addTransformRequested(
                transformPicker.currentValue,
                root.appController.transformModel.version_at(transformPicker.currentIndex),
                transformParamsField.text
            )
        }

        Button {
            text: "Add Vocals Stem"
            palette.buttonText: root.controlTextColor
            enabled: root.appController.selectedTrackId.length > 0
            onClicked: root.addVocalsStemRequested()
        }

        Button {
            text: "Check Cache"
            palette.buttonText: root.controlTextColor
            onClicked: root.refreshCacheRequested()
        }

        Button {
            text: "Derive Editable"
            palette.buttonText: root.controlTextColor
            enabled: root.appController.selectedTrackId.length > 0
            onClicked: root.deriveEditableRequested()
        }

        Item { Layout.fillWidth: true }
    }
}
