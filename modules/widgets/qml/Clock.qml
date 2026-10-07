import QtQuick
import qs.island

// The clock widget: the time, big, and the date under it, here or in
// another time zone. The text grows with the widget, and its digits roll
// as they change.
Item {
    id: root

    // The widgets module's state, which has each time zone's offset.
    property var payload: null
    property var settings: ({})
    property string instance: ""

    readonly property string zone: settings.timezone ?? ""
    // Seconds from UTC, once the module has read the zone.
    readonly property var offset: zone !== "" ? payload?.zones?.[zone] : undefined
    readonly property bool twelve: settings.hours === "12"
    readonly property bool seconds: settings.seconds === true

    property date now: new Date()

    Timer {
        interval: root.seconds ? 1000 : 5000
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: root.now = new Date()
    }

    // The time's parts here, or in the zone: the zone's time is UTC moved
    // by its offset, read with the UTC getters.
    readonly property var parts: {
        const utc = zone !== "" && offset !== undefined;
        const time = utc ? new Date(now.getTime() + offset * 1000) : now;
        return {
            hours: utc ? time.getUTCHours() : time.getHours(),
            minutes: utc ? time.getUTCMinutes() : time.getMinutes(),
            seconds: utc ? time.getUTCSeconds() : time.getSeconds(),
            day: utc ? time.getUTCDay() : time.getDay(),
            date: utc ? time.getUTCDate() : time.getDate(),
            month: utc ? time.getUTCMonth() : time.getMonth()
        };
    }

    readonly property string time: {
        const pad = value => `${value}`.padStart(2, "0");
        let hours = parts.hours;
        if (twelve)
            hours = hours % 12 === 0 ? 12 : hours % 12;
        const text = `${twelve ? hours : pad(hours)}:${pad(parts.minutes)}`;
        return seconds ? `${text}:${pad(parts.seconds)}` : text;
    }

    Column {
        anchors.verticalCenter: parent.verticalCenter
        width: parent.width
        spacing: 2

        Row {
            spacing: Theme.spaceSmall

            RollingText {
                id: clock

                text: root.time
                family: Theme.displayFamily
                weight: Theme.weightTitle
                // As big as the widget allows, by height and by width.
                // "AM" takes about a digit and a half more, at a third of the size.
                pixelSize: Math.max(12, Math.min(root.height * (root.settings.date === false ? 0.8 : 0.55), root.width / (root.time.length * 0.62 + (root.twelve ? 0.75 : 0))))
            }

            Text {
                id: half

                // On the time's baseline.
                y: clockMetrics.ascent - halfMetrics.ascent
                visible: root.twelve
                text: root.parts.hours < 12 ? "AM" : "PM"
                color: Theme.muted
                font.family: Theme.fontFamily
                font.weight: Theme.weightTitle
                font.pixelSize: Math.max(10, clock.pixelSize * 0.3)

                FontMetrics {
                    id: halfMetrics

                    font: half.font
                }

                FontMetrics {
                    id: clockMetrics

                    font: clock.font
                }
            }
        }

        Text {
            visible: root.settings.date !== false
            width: parent.width
            elide: Text.ElideRight
            text: {
                const locale = Qt.locale();
                const day = locale.dayName(root.parts.day, Locale.LongFormat);
                const month = locale.monthName(root.parts.month, Locale.LongFormat);
                const date = `${day}, ${root.parts.date} ${month}`;
                return root.zone !== "" ? `${date} · ${root.zone.split("/").pop().replace(/_/g, " ")}` : date;
            }
            color: Theme.muted
            font.family: Theme.fontFamily
            font.pixelSize: Math.max(Theme.textCaption, clock.font.pixelSize * 0.24)
        }
    }
}
