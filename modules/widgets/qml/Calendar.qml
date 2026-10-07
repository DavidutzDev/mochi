import QtQuick
import qs.island

// The calendar widget: a month, today marked, with arrows to the months
// around it. The text grows with the widget.
Item {
    id: root

    property var payload: null
    property var settings: ({})
    property string instance: ""

    readonly property bool sundays: settings.first_day === "sunday"
    // Months away from this one, from the arrows.
    property int shift: 0
    property date today: new Date()

    // Today changes at midnight; checking every minute is enough.
    Timer {
        interval: 60000
        running: true
        repeat: true
        onTriggered: root.today = new Date()
    }

    readonly property date month: new Date(today.getFullYear(), today.getMonth() + shift, 1)
    // Six weeks of days, from the start of the week the month starts in.
    readonly property var days: {
        const first = month.getDay();
        const back = sundays ? first : (first + 6) % 7;
        const start = new Date(month.getFullYear(), month.getMonth(), 1 - back);
        const days = [];
        for (let index = 0; index < 42; index++)
            days.push(new Date(start.getFullYear(), start.getMonth(), start.getDate() + index));
        return days;
    }
    readonly property real cell: Math.min(width / 7, (height - header.height - 8) / 7)
    readonly property real text: Math.max(9, cell * 0.38)

    Item {
        id: header

        width: parent.width
        height: Math.max(22, root.text * 1.8)

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: `${Qt.locale().standaloneMonthName(root.month.getMonth(), Locale.LongFormat)} ${root.month.getFullYear()}`
            color: Theme.accent
            font.pixelSize: root.text * 1.1
            font.family: Theme.fontFamily
            font.weight: Theme.weightTitle
        }

        Row {
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter

            IconButton {
                icon: "chevron"
                rotation: 180
                size: 12
                onClicked: root.shift -= 1
            }

            IconButton {
                visible: root.shift !== 0
                icon: "dot"
                size: 10
                onClicked: root.shift = 0
            }

            IconButton {
                icon: "chevron"
                size: 12
                onClicked: root.shift += 1
            }
        }
    }

    Grid {
        anchors.top: header.bottom
        anchors.topMargin: Theme.spaceSmall
        anchors.horizontalCenter: parent.horizontalCenter
        columns: 7

        Repeater {
            model: 7

            Text {
                required property int index

                width: root.cell
                height: root.cell * 0.8
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
                text: Qt.locale().dayName((index + (root.sundays ? 0 : 1)) % 7, Locale.NarrowFormat)
                color: Theme.muted
                font.pixelSize: root.text * 0.9
                font.family: Theme.fontFamily
                font.weight: Theme.weightTitle
            }
        }

        Repeater {
            model: root.days

            Item {
                id: day

                required property date modelData
                readonly property bool inMonth: modelData.getMonth() === root.month.getMonth()
                readonly property bool isToday: modelData.toDateString() === root.today.toDateString()

                width: root.cell
                height: root.cell

                Rectangle {
                    anchors.centerIn: parent
                    width: root.cell * 0.82
                    height: width
                    radius: width / 2
                    visible: day.isToday
                    color: Theme.accent
                }

                Text {
                    anchors.centerIn: parent
                    text: day.modelData.getDate()
                    color: day.isToday ? Theme.onAccent : day.inMonth ? Theme.foreground : Theme.muted
                    opacity: day.inMonth || day.isToday ? 1 : 0.5
                    font.pixelSize: root.text
                    font.family: Theme.fontFamily
                    font.weight: day.isToday ? Font.DemiBold : Font.Normal
                    font.features: {
                        "tnum": 1
                    }
                }
            }
        }
    }
}
