import QtQuick
import qs.island

// The calendar widget's looks: a month, with arrows to the months around
// it, or this week on a strip. Today is a pentagon in the accent color, the
// widget's one accent. The text grows with the widget.
Item {
    id: root

    property var payload: null
    property var settings: ({})
    property string instance: ""
    property string variant: ""

    readonly property bool week: variant === "week"
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
    // The days from the start of the week `from` is in.
    function daysFrom(from: date, count: int): var {
        const back = sundays ? from.getDay() : (from.getDay() + 6) % 7;
        const start = new Date(from.getFullYear(), from.getMonth(), from.getDate() - back);
        const days = [];
        for (let index = 0; index < count; index++)
            days.push(new Date(start.getFullYear(), start.getMonth(), start.getDate() + index));
        return days;
    }
    // Six weeks of days, from the start of the week the month starts in, or
    // this week.
    readonly property var days: week ? daysFrom(today, 7) : daysFrom(month, 42)
    readonly property int rows: week ? 1 : 6
    // A day's place: square in the month, as wide as the strip allows in
    // the week. The days' names take most of a row.
    // The header's height, from an estimate of the text's size, since
    // the days' size depends on it.
    readonly property real headerHeight: Math.max(22, Math.max(9, Math.min(width / 7, height / (rows + 3)) * 0.38) * 1.8)
    readonly property real cellHeight: Math.min(width / 7, (height - headerHeight - Theme.spaceSmall) / (rows + 0.8))
    readonly property real cellWidth: week ? width / 7 : cellHeight
    readonly property real text: Math.max(9, cellHeight * 0.38)

    Item {
        id: header

        width: parent.width
        height: root.headerHeight

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: `${Qt.locale().standaloneMonthName(root.month.getMonth(), Locale.LongFormat)} ${root.month.getFullYear()}`
            color: Theme.foreground
            font.pixelSize: root.text * 1.1
            font.family: Theme.fontFamily
            font.weight: Theme.weightTitle
        }

        Row {
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            visible: !root.week

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

                width: root.cellWidth
                height: root.cellHeight * 0.8
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
                // On the strip, every day is this week's.
                readonly property bool inMonth: root.week || modelData.getMonth() === root.month.getMonth()
                readonly property bool isToday: modelData.toDateString() === root.today.toDateString()

                width: root.cellWidth
                height: root.cellHeight

                ExpressiveShape {
                    anchors.centerIn: parent
                    width: Math.min(root.cellWidth, root.cellHeight) * 0.9
                    height: width
                    visible: day.isToday
                    shape: "pentagon"
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
