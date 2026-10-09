import QtQuick

// The time now, here or in a time zone, in parts, for clocks. QML has no
// time zone database, so a module reads each zone's offset from the system
// and publishes it, see mochi_core::zones: the zone's time is UTC moved by
// that offset, read with the UTC getters. Until it has, and for a zone the
// system doesn't know, it's this computer's time.
Item {
    id: root

    // The state of the module that read the zones, like the widgets or the
    // clock module's: `zones`, offsets in seconds by zone, and
    // `unknownZones`.
    property var payload: null
    // Like "Europe/Paris"; empty for this computer's.
    property string zone: ""
    // Ticks every second instead of every few.
    property bool seconds: false
    // Off for one that follows another's `now`.
    property bool ticking: true

    // Seconds from UTC, once the module has read the zone.
    readonly property var offset: zone !== "" ? payload?.zones?.[zone] : undefined
    readonly property bool unknown: zone !== "" && (payload?.unknownZones ?? []).includes(zone)
    readonly property bool known: zone === "" || offset !== undefined
    // The zone's last part, like "New York".
    readonly property string city: zone.split("/").pop().replace(/_/g, " ")

    property date now: new Date()

    Timer {
        interval: root.seconds ? 1000 : 5000
        running: root.ticking
        repeat: true
        triggeredOnStart: true
        onTriggered: root.now = new Date()
    }

    readonly property var parts: {
        const utc = zone !== "" && offset !== undefined;
        const time = utc ? new Date(now.getTime() + offset * 1000) : now;
        return {
            hours: utc ? time.getUTCHours() : time.getHours(),
            minutes: utc ? time.getUTCMinutes() : time.getMinutes(),
            seconds: utc ? time.getUTCSeconds() : time.getSeconds(),
            day: utc ? time.getUTCDay() : time.getDay(),
            date: utc ? time.getUTCDate() : time.getDate(),
            month: utc ? time.getUTCMonth() : time.getMonth(),
            year: utc ? time.getUTCFullYear() : time.getFullYear()
        };
    }

    function pad(value: int): string {
        return `${value}`.padStart(2, "0");
    }

    // The hour as a clock shows it: 9 or 12 on a 12-hour clock, 09 on a
    // 24-hour one.
    function hour(twelve: bool): string {
        if (!twelve)
            return pad(parts.hours);
        return `${parts.hours % 12 === 0 ? 12 : parts.hours % 12}`;
    }

    // "09:50", "9:50" on a 12-hour clock, with ":07" for the seconds.
    function time(twelve: bool, withSeconds: bool): string {
        const text = `${hour(twelve)}:${pad(parts.minutes)}`;
        return withSeconds ? `${text}:${pad(parts.seconds)}` : text;
    }

    readonly property string half: parts.hours < 12 ? "AM" : "PM"

    // "Wednesday, 7 October", or "Wed 7 Oct" short.
    function date(short: bool): string {
        const locale = Qt.locale();
        const format = short ? Locale.ShortFormat : Locale.LongFormat;
        const day = locale.dayName(parts.day, format);
        const month = locale.monthName(parts.month, format);
        return short ? `${day} ${parts.date} ${month}` : `${day}, ${parts.date} ${month}`;
    }
}
