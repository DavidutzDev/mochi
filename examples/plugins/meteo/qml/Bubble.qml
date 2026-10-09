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
        spacing: Theme.spaceTiny

        WeatherIcon {
            anchors.verticalCenter: parent.verticalCenter
            kind: root.payload.kind ?? "cloudy"
            day: root.payload.day ?? true
            size: 14
        }

        RollingText {
            anchors.verticalCenter: parent.verticalCenter
            text: `${root.payload.temperature ?? "–"}°`
            pixelSize: Theme.textCaption
            weight: Theme.weightTitle
        }
    }
}
