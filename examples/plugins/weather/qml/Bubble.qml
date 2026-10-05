import QtQuick
import qs.island

// The temperature now, with its icon. A click shows the forecast.
Item {
    id: root

    property var payload: ({})

    implicitWidth: row.implicitWidth + 12
    implicitHeight: 26

    Row {
        id: row

        anchors.centerIn: parent
        spacing: 4

        WeatherIcon {
            anchors.verticalCenter: parent.verticalCenter
            kind: root.payload.kind ?? "cloudy"
            day: root.payload.day ?? true
            size: 14
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: `${root.payload.temperature ?? "–"}°`
            color: Theme.foreground
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
            font.weight: Font.DemiBold
            font.features: { "tnum": 1 }
        }
    }
}
