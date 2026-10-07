import QtQuick
import qs.island

// The hub's card, and a desktop widget: the temperature and sky now, and
// the day's low and high, in one row.
Item {
    id: root

    property var payload: null
    // What the desktop gives a widget; this one has no settings.
    property var settings: ({})
    property string instance: ""
    readonly property bool ready: payload?.temperature !== undefined && payload?.temperature !== null
    readonly property var today: payload?.days?.[0] ?? null

    implicitHeight: Theme.rowHeight

    Text {
        anchors.verticalCenter: parent.verticalCenter
        visible: !root.ready
        width: parent.width
        wrapMode: Text.Wrap
        text: root.payload?.error ?? "Fetching the weather…"
        color: Theme.muted
        font.pixelSize: Theme.textCaption
        font.family: Theme.fontFamily
    }

    Row {
        anchors.verticalCenter: parent.verticalCenter
        width: parent.width
        visible: root.ready
        spacing: Theme.spaceMedium

        WeatherIcon {
            id: icon

            anchors.verticalCenter: parent.verticalCenter
            kind: root.payload?.kind ?? "cloudy"
            day: root.payload?.day ?? true
            color: Theme.accent
            size: 30
        }

        Column {
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - icon.width - parent.spacing

            RollingText {
                text: `${root.payload?.temperature}${root.payload?.unit ?? ""}`
                pixelSize: Theme.textTitle
                weight: Theme.weightTitle
            }

            Text {
                width: parent.width
                elide: Text.ElideRight
                text: {
                    const range = root.today ? ` · ${root.today.min}° to ${root.today.max}°` : "";
                    return `${root.payload?.description ?? ""} in ${root.payload?.place ?? ""}${range}`;
                }
                color: Theme.muted
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }
        }
    }

    MouseArea {
        anchors.fill: parent
        enabled: root.ready
        cursorShape: Qt.PointingHandCursor
        onClicked: Daemon.command("weather", "show", [])
    }
}
