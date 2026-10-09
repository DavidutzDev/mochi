import QtQuick
import qs.island

// The widget's forecast look: a row a day, from today, as many as fit, each
// with its weekday, its sky, the chance of rain or snow when it's likely,
// and its low and high on either side of a bar. The bars share one scale,
// from the week's lowest to its highest, so a cold day sits to the left and
// a warm one to the right; today's has a dot at the temperature now.
Item {
    id: root

    property var payload: null
    property var settings: ({})
    property string instance: ""
    property string variant: ""
    readonly property var current: payload?.current ?? null
    readonly property var days: payload?.daily ?? []
    // A row is at least this high, and the age takes one while it shows.
    readonly property real least: 22
    readonly property int rows: Math.max(1, Math.min(days.length, Math.floor(height / least) - (age.stale ? 1 : 0)))
    readonly property var shown: days.slice(0, rows)
    readonly property real row: (height - (age.stale ? age.height + Theme.spaceSmall : 0)) / rows
    readonly property real pixel: Math.max(Theme.textBody, Math.min(row * 0.42, Theme.textHeadline))
    // The week's range, with today's temperature now in it.
    readonly property real low: Math.min(...shown.map(day => day.min), current?.temperature ?? Infinity)
    readonly property real high: Math.max(...shown.map(day => day.max), current?.temperature ?? -Infinity)
    // The columns are as wide as what they hold: the longest name of a day
    // shown, a temperature like -10°, and the chances only when a day has
    // one, so the bars get the rest. Reading the font keeps them in step
    // with it.
    readonly property real names: metrics.height > 0 ? Math.max(0, ...shown.map((day, index) => metrics.advanceWidth(name(day, index)))) : 0
    readonly property real figure: metrics.height > 0 ? metrics.advanceWidth("-10°") : 0
    readonly property bool wet: shown.some(day => (day.precipitation ?? 0) >= 20)

    function degrees(value: real): string {
        return `${Math.round(value)}°`;
    }

    function name(day: var, index: int): string {
        return index === 0 ? "Today" : Qt.locale().dayName(day.weekday, Locale.ShortFormat);
    }

    FontMetrics {
        id: metrics

        font.pixelSize: root.pixel
        font.family: Theme.fontFamily
        font.weight: Theme.weightTitle
    }

    // Where a temperature is along a bar, from 0 to 1.
    function along(value: real): real {
        return high > low ? (value - low) / (high - low) : 0.5;
    }

    implicitWidth: 256
    implicitHeight: 224

    Missing {
        anchors.verticalCenter: parent.verticalCenter
        width: parent.width
        visible: root.current === null
        payload: root.payload
        room: root.height
    }

    Column {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        visible: root.current !== null

        Repeater {
            model: root.current !== null ? root.shown : []

            Item {
                id: day

                required property var modelData
                required property int index
                readonly property bool likely: (modelData.precipitation ?? 0) >= 20

                width: parent.width
                height: root.row

                Text {
                    id: weekday

                    anchors.left: parent.left
                    anchors.verticalCenter: parent.verticalCenter
                    width: root.names + Theme.spaceSmall
                    text: root.name(day.modelData, day.index)
                    elide: Text.ElideRight
                    color: Theme.foreground
                    font.pixelSize: root.pixel
                    font.family: Theme.fontFamily
                    font.weight: day.index === 0 ? Theme.weightTitle : Theme.weightLabel
                }

                Symbol {
                    id: sky

                    anchors.left: weekday.right
                    anchors.verticalCenter: parent.verticalCenter
                    name: day.modelData.icon
                    size: Math.round(root.pixel * 1.45)
                    color: Theme.foreground
                }

                Text {
                    id: chance

                    anchors.left: sky.right
                    anchors.leftMargin: root.wet ? Theme.spaceTiny : 0
                    anchors.verticalCenter: parent.verticalCenter
                    width: root.wet ? root.pixel * 2.4 : 0
                    text: day.likely ? `${Math.round(day.modelData.precipitation)}%` : ""
                    color: Theme.muted
                    font.pixelSize: Math.max(Theme.textCaption, root.pixel * 0.8)
                    font.family: Theme.fontFamily
                }

                Text {
                    id: min

                    anchors.left: chance.right
                    anchors.verticalCenter: parent.verticalCenter
                    width: root.figure + Theme.spaceSmall
                    horizontalAlignment: Text.AlignRight
                    text: root.degrees(day.modelData.min)
                    color: Theme.muted
                    font.pixelSize: root.pixel
                    font.family: Theme.fontFamily
                }

                // The track, the day's range on it, and today's dot.
                Rectangle {
                    id: track

                    anchors.left: min.right
                    anchors.leftMargin: Theme.spaceSmall
                    anchors.right: max.left
                    anchors.rightMargin: Theme.spaceSmall
                    anchors.verticalCenter: parent.verticalCenter
                    height: Math.max(4, Math.round(root.pixel * 0.36))
                    radius: height / 2
                    color: Theme.highlight

                    Rectangle {
                        x: root.along(day.modelData.min) * parent.width
                        width: Math.max(parent.height, (root.along(day.modelData.max) - root.along(day.modelData.min)) * parent.width)
                        height: parent.height
                        radius: parent.radius
                        color: Theme.accent
                    }

                    Rectangle {
                        readonly property real side: parent.height + 4

                        visible: day.index === 0 && root.current !== null
                        x: root.along(root.current?.temperature ?? 0) * parent.width - side / 2
                        anchors.verticalCenter: parent.verticalCenter
                        width: side
                        height: side
                        radius: side / 2
                        color: Theme.foreground
                        border.width: 2
                        border.color: Theme.background
                    }
                }

                Text {
                    id: max

                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    width: root.figure
                    horizontalAlignment: Text.AlignRight
                    text: root.degrees(day.modelData.max)
                    color: Theme.foreground
                    font.pixelSize: root.pixel
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightLabel
                }
            }
        }
    }

    Age {
        id: age

        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        visible: stale && root.current !== null
        payload: root.payload
    }
}
