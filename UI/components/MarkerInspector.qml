import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: inspectorPanel
    property var appController
    property var markerColorOptions: []
    property color panelBackground: "#1c1f26"
    property color borderSubtle: "#2f333d"
    property color textPrimary: "#f4f4f5"
    property color textMuted: "#a1a1aa"
    property color selectedMarkerBackground: "#2f4366"
    property string selectedMarkerId: ""
    signal addCueRequested(real timestamp, string label, string category, string colorKey)
    signal deleteCueRequested(string markerId)
    signal updateCueRequested(real timestamp, string label, string category, string colorKey)
    signal bulkUpdateRequested(string label, string category, string colorKey)
    signal toggleMarkerSelectionRequested(string markerId, bool extendSelection)

    function markerColorIndex(colorKey) {
        for (var i = 0; i < inspectorPanel.markerColorOptions.length; i++) {
            if (inspectorPanel.markerColorOptions[i].key === colorKey) {
                return i
            }
        }
        return 0
    }

    function selectedMarkerCount() {
        return inspectorPanel.appController.selectedMarkerIds.length
    }

    function clearSelectionId() {
        inspectorPanel.selectedMarkerId = ""
    }

    function syncMarkerEditor(marker) {
        markerTimestampField.text = Number(marker.timestamp).toFixed(2)
        markerLabelField.text = marker.label.length > 0 ? marker.label : "Cue"
        markerCategoryField.text = marker.category.length > 0 ? marker.category : "cue"
        markerColorPicker.currentIndex = inspectorPanel.markerColorIndex(marker.colorKey)
    }

    function syncMarkerEditorFromSelection() {
        inspectorPanel.selectedMarkerId = ""
        if (inspectorPanel.appController.selectedMarkerIds.length !== 1) {
            return
        }

        var markerId = inspectorPanel.appController.selectedMarkerIds[0]
        for (var i = 0; i < inspectorPanel.appController.selectedTrackMarkers.length; i++) {
            var marker = inspectorPanel.appController.selectedTrackMarkers[i]
            if (marker.id === markerId && marker.selected) {
                inspectorPanel.selectedMarkerId = marker.id
                inspectorPanel.syncMarkerEditor(marker)
                return
            }
        }
    }

    color: inspectorPanel.panelBackground
    border.color: inspectorPanel.borderSubtle

    Connections {
        target: inspectorPanel.appController
        function onSelectedTrackIdChanged() {
            inspectorPanel.selectedMarkerId = ""
        }
        function onSelectedMarkerIdsChanged() {
            inspectorPanel.syncMarkerEditorFromSelection()
        }
    }

    Column {
        anchors.fill: parent
        anchors.margins: 12
        spacing: 8

        Label {
            text: "Inspector"
            color: inspectorPanel.textPrimary
            font.bold: true
        }

        Text {
            text: inspectorPanel.appController.selectedTrackId.length === 0 ? "No track selected" : ""
            visible: inspectorPanel.appController.selectedTrackId.length === 0
            color: inspectorPanel.textMuted
            font.pixelSize: 12
            wrapMode: Text.WordWrap
            width: parent.width
        }

        TextField {
            id: markerTimestampField
            placeholderText: "Timestamp"
            text: "0.0"
            enabled: inspectorPanel.appController.selectedTrackIsEditable
            width: parent.width
        }

        TextField {
            id: markerLabelField
            placeholderText: "Label"
            text: "Cue"
            enabled: inspectorPanel.appController.selectedTrackIsEditable
            width: parent.width
        }

        TextField {
            id: markerCategoryField
            placeholderText: "Category"
            text: "cue"
            enabled: inspectorPanel.appController.selectedTrackIsEditable
            width: parent.width
        }

        ComboBox {
            id: markerColorPicker
            model: inspectorPanel.markerColorOptions
            textRole: "label"
            valueRole: "key"
            enabled: inspectorPanel.appController.selectedTrackIsEditable
            width: parent.width
            delegate: ItemDelegate {
                width: markerColorPicker.width
                text: modelData.label
                contentItem: Row {
                    spacing: 8
                    Rectangle {
                        width: 12
                        height: 12
                        radius: 6
                        color: modelData.color
                        anchors.verticalCenter: parent.verticalCenter
                    }
                    Text {
                        text: modelData.label
                        color: inspectorPanel.textPrimary
                        anchors.verticalCenter: parent.verticalCenter
                    }
                }
            }
        }

        ScrollView {
            id: markerScroll
            width: parent.width
            height: 120
            clip: true

            Column {
                id: markerList
                width: markerScroll.availableWidth
                spacing: 2

                Repeater {
                    model: inspectorPanel.appController.selectedTrackMarkers
                    delegate: Rectangle {
                        required property var modelData
                        width: markerList.width
                        height: 34
                        radius: 3
                        color: modelData.selected ? inspectorPanel.selectedMarkerBackground : "transparent"
                        border.color: modelData.selected ? modelData.color : "transparent"

                        Rectangle {
                            id: markerColorSwatch
                            width: 10
                            height: 10
                            radius: 5
                            color: modelData.color
                            anchors.left: parent.left
                            anchors.leftMargin: 4
                            anchors.verticalCenter: parent.verticalCenter
                        }

                        Text {
                            anchors.left: markerColorSwatch.right
                            anchors.leftMargin: 8
                            anchors.right: parent.right
                            anchors.rightMargin: 4
                            anchors.verticalCenter: parent.verticalCenter
                            text: Number(modelData.timestamp).toFixed(2) + "  " + modelData.label
                            color: inspectorPanel.textPrimary
                            elide: Text.ElideRight
                        }

                        MouseArea {
                            anchors.fill: parent
                            onClicked: function(mouse) {
                                inspectorPanel.toggleMarkerSelectionRequested(modelData.id, (mouse.modifiers & Qt.ShiftModifier) !== 0)
                            }
                        }
                    }
                }
            }
        }

        Button {
            text: "Add Cue"
            enabled: inspectorPanel.appController.selectedTrackId.length > 0 && inspectorPanel.appController.selectedTrackIsEditable
            onClicked: inspectorPanel.addCueRequested(
                Number(markerTimestampField.text),
                markerLabelField.text,
                markerCategoryField.text,
                markerColorPicker.currentValue
            )
        }

        Button {
            text: "Delete Cue"
            enabled: inspectorPanel.selectedMarkerId.length > 0 && inspectorPanel.appController.selectedTrackIsEditable
            onClicked: inspectorPanel.deleteCueRequested(inspectorPanel.selectedMarkerId)
        }

        Button {
            text: "Update Cue"
            enabled: inspectorPanel.appController.selectedTrackIsEditable && inspectorPanel.selectedMarkerCount() === 1
            onClicked: inspectorPanel.updateCueRequested(
                Number(markerTimestampField.text),
                markerLabelField.text,
                markerCategoryField.text,
                markerColorPicker.currentValue
            )
        }

        Button {
            text: inspectorPanel.selectedMarkerCount() > 0 ? "Apply To Selected" : "Apply To Track"
            enabled: inspectorPanel.appController.selectedTrackIsEditable && inspectorPanel.appController.selectedTrackMarkers.length > 0
            onClicked: inspectorPanel.bulkUpdateRequested(
                markerLabelField.text,
                markerCategoryField.text,
                markerColorPicker.currentValue
            )
        }
    }
}
