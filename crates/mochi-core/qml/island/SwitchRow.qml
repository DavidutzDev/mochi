import QtQuick

// A setting that is on or off: an icon, a title and a subtitle, with a
// switch at the end. A click anywhere on the row flips it. Like Switch, it
// says `toggled` and the owner sets `checked` to the real state.
ListRow {
    id: root

    property bool checked: false
    signal toggled(bool checked)

    flat: true
    onClicked: root.toggled(!root.checked)

    trailing: Switch {
        anchors.verticalCenter: parent.verticalCenter
        checked: root.checked
        onToggled: checked => root.toggled(checked)
    }
}
