import QtQuick

// A big clock's numerals, the hour above the minutes, in the display font
// and as large as the box allows, with an optional line under them, like
// the date. The owner formats the parts, so a clock in another time zone or
// with 12 hours works the same. The digits roll as they change.
Item {
    id: root

    property string hours: ""
    property string minutes: ""
    // A line under the numerals, like "Wed 7 Oct"; none when empty.
    property string date: ""
    property color color: Theme.foreground
    property color dateColor: Theme.muted
    property int weight: Theme.weightTitle
    property string family: Theme.displayFamily
    // Text.AlignLeft, Text.AlignHCenter or Text.AlignRight.
    property int horizontalAlignment: Text.AlignHCenter
    property bool animated: true

    // The numerals' size, from the box.
    readonly property int pixelSize: Math.max(Theme.textCaption, Math.floor(Math.min(height / tall, width / wide)))
    readonly property int dateSize: Math.max(Theme.textCaption, Math.round(pixelSize * 0.17))

    implicitWidth: 120
    implicitHeight: 160

    // The digits' shapes at a reference size, to scale from: how tall a
    // digit is, how far its top sits below the top of its line, and how
    // wide the widest of the two lines is.
    readonly property real reference: 100
    readonly property real digit: zero.tightBoundingRect.height / reference
    readonly property real above: (metrics.ascent - zero.tightBoundingRect.height) / reference
    readonly property real wide: Math.max(hours.length, minutes.length, 1) * zero.advanceWidth / reference
    // The space between the numerals, and above the date.
    readonly property real between: 0.08
    readonly property real beforeDate: 0.12
    // The whole block's height for each pixel of the numerals' size.
    readonly property real tall: digit * 2 + between + (date !== "" ? beforeDate + 0.17 * 1.3 : 0)

    TextMetrics {
        id: zero

        font.family: root.family
        font.weight: root.weight
        font.pixelSize: root.reference
        font.features: {
            "tnum": 1
        }
        text: "0"
    }

    FontMetrics {
        id: metrics

        font: zero.font
    }

    // Where the block starts, so it's centered in the box.
    readonly property real blockTop: {
        const block = pixelSize * (digit * 2 + between) + (date !== "" ? pixelSize * beforeDate + label.implicitHeight : 0);
        return (height - block) / 2;
    }

    function place(lineWidth: real): real {
        if (horizontalAlignment === Text.AlignLeft)
            return 0;
        if (horizontalAlignment === Text.AlignRight)
            return width - lineWidth;
        return (width - lineWidth) / 2;
    }

    RollingText {
        id: upper

        x: root.place(width)
        y: root.blockTop - root.pixelSize * root.above
        text: root.hours
        pixelSize: root.pixelSize
        family: root.family
        weight: root.weight
        color: root.color
        animated: root.animated
    }

    RollingText {
        id: lower

        x: root.place(width)
        y: root.blockTop + root.pixelSize * (root.digit + root.between - root.above)
        text: root.minutes
        pixelSize: root.pixelSize
        family: root.family
        weight: root.weight
        color: root.color
        animated: root.animated
    }

    Text {
        id: label

        x: root.place(width)
        y: root.blockTop + root.pixelSize * (root.digit * 2 + root.between + root.beforeDate)
        width: Math.min(implicitWidth, root.width)
        visible: root.date !== ""
        text: root.date
        elide: Text.ElideRight
        color: root.dateColor
        font.family: Theme.fontFamily
        font.weight: Theme.weightLabel
        font.pixelSize: root.dateSize
    }
}
