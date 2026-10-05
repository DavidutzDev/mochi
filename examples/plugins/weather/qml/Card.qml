import QtQuick
import qs.island

// The hub's card: the temperature and sky now, and the day's low and high.
Item {
    id: root

    property var payload: null
    readonly property bool ready: payload?.temperature !== undefined && payload?.temperature !== null
    readonly property var today: payload?.days?.[0] ?? null

    implicitHeight: 64

    Text {
        anchors.verticalCenter: parent.verticalCenter
        visible: !root.ready
        width: parent.width
        wrapMode: Text.Wrap
        text: root.payload?.error ?? "Fetching the weather…"
        color: Theme.muted
        font.pixelSize: Theme.textLabel
        font.family: Theme.fontFamily
    }

    Row {
        anchors.verticalCenter: parent.verticalCenter
        visible: root.ready
        spacing: 14

        WeatherIcon {
            anchors.verticalCenter: parent.verticalCenter
            kind: root.payload?.kind ?? "cloudy"
            day: root.payload?.day ?? true
            color: Theme.accent
            size: 30
        }

        Column {
            anchors.verticalCenter: parent.verticalCenter
            spacing: 2

            Text {
                text: `${root.payload?.temperature}${root.payload?.unit ?? ""}`
                color: Theme.foreground
                font.pixelSize: Theme.textTitle
                font.family: Theme.fontFamily
                font.weight: Font.DemiBold
                font.features: { "tnum": 1 }
            }

            Text {
                text: {
                    const range = root.today ? ` · ${root.today.min}° to ${root.today.max}°` : "";
                    return `${root.payload?.description ?? ""} in ${root.payload?.place ?? ""}${range}`;
                }
                color: Theme.muted
                font.pixelSize: Theme.textLabel
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
