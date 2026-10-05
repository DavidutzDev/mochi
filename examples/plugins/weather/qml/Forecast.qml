import QtQuick
import qs.island

// The forecast on the island: now, then three days.
Item {
    id: root

    property var payload: ({})
    readonly property var days: payload.days ?? []

    implicitWidth: Math.max(now.implicitWidth, week.implicitWidth) + Theme.padding * 2
    implicitHeight: column.implicitHeight + Theme.padding * 2

    function dayName(date: string, index: int): string {
        if (index === 0)
            return "Today";
        return new Date(`${date}T12:00:00`).toLocaleDateString(Qt.locale(), "ddd");
    }

    Column {
        id: column

        anchors.centerIn: parent
        spacing: 12

        Row {
            id: now

            anchors.horizontalCenter: parent.horizontalCenter
            spacing: 12

            WeatherIcon {
                anchors.verticalCenter: parent.verticalCenter
                kind: root.payload.kind ?? "cloudy"
                day: root.payload.day ?? true
                color: Theme.accent
                size: 34
            }

            Column {
                anchors.verticalCenter: parent.verticalCenter
                spacing: 2

                Text {
                    text: `${root.payload.temperature}${root.payload.unit ?? ""} · ${root.payload.description ?? ""}`
                    color: Theme.foreground
                    font.pixelSize: Theme.textTitle
                    font.family: Theme.fontFamily
                    font.weight: Font.DemiBold
                }

                Text {
                    text: `${root.payload.place ?? ""} · feels like ${root.payload.feels_like}° · wind ${root.payload.wind} ${root.payload.wind_unit ?? ""}`
                    color: Theme.muted
                    font.pixelSize: Theme.textLabel
                    font.family: Theme.fontFamily
                }
            }
        }

        Row {
            id: week

            anchors.horizontalCenter: parent.horizontalCenter
            spacing: 22

            Repeater {
                model: root.days

                Column {
                    required property var modelData
                    required property int index

                    spacing: 4

                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: root.dayName(modelData.date, index)
                        color: Theme.muted
                        font.pixelSize: Theme.textLabel
                        font.family: Theme.fontFamily
                    }

                    WeatherIcon {
                        anchors.horizontalCenter: parent.horizontalCenter
                        kind: modelData.kind
                        size: 20
                    }

                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: `${modelData.min}° ${modelData.max}°`
                        color: Theme.foreground
                        font.pixelSize: Theme.textLabel
                        font.family: Theme.fontFamily
                        font.features: { "tnum": 1 }
                    }
                }
            }
        }
    }
}
