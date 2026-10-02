import QtQuick
import QtQuick.Controls

ApplicationWindow {
    visible: true
    width: 900
    height: 700
    title: "SnipExpand compatibility target"

    TextArea {
        anchors.fill: parent
        focus: true
        font.family: "monospace"
        font.pixelSize: 20
        wrapMode: TextEdit.NoWrap
    }
}
