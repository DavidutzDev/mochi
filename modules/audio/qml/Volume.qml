import QtQuick
import Quickshell
import qs.island

// One volume in the mixer: an icon that mutes on click, the name, a slider
// and the percent. `symbol` is a built-in icon, `appIcon` an icon theme
// name for apps, which falls back to `symbol` when the theme lacks it, and
// to a note without one. With `choosable`, clicking the name sends `choose`, for
// picking another device or opening an app's streams. With `routeIcon`, a
// button with that icon at the end sends `route`, for the app's output.
// `level` is the meter's, from 0 to 1 on the scale of 100%; below 0, none.
Item {
    id: root

    property string target: ""
    property string title: ""
    property string subtitle: ""
    property string symbol: ""
    property string mutedSymbol: ""
    property string appIcon: ""
    property int volume: 0
    property bool muted: false
    property int maxVolume: 100
    property bool choosable: false
    property bool choosing: false
    property string routeIcon: ""
    property bool routing: false
    property real level: -1
    signal choose
    signal route

    implicitWidth: 360
    implicitHeight: 52

    function send(level: int): void {
        Daemon.command("audio", "volume", [root.target, `${level}`]);
    }

    // Dragging sends a level at most every 50 ms; letting go sends the last.
    property int pending: -1

    Timer {
        id: throttle

        interval: 50
        onTriggered: {
            if (root.pending >= 0)
                root.send(root.pending);
            root.pending = -1;
        }
    }

    Button {
        id: mute

        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        implicitHeight: 40
        tone: "ghost"
        icon: root.appIcon === "" ? (root.muted ? root.mutedSymbol : root.symbol) : ""
        iconSize: 20
        onClicked: Daemon.command("audio", "mute", [root.target])

        Image {
            id: picture

            anchors.centerIn: parent
            width: 26
            height: 26
            visible: root.appIcon !== "" && status === Image.Ready
            opacity: root.muted ? 0.35 : 1
            // A desktop entry can name a picture file instead of an icon.
            source: {
                if (root.appIcon === "")
                    return "";
                if (root.appIcon.startsWith("/"))
                    return `file://${root.appIcon.split("/").map(encodeURIComponent).join("/")}`;
                return Quickshell.iconPath(root.appIcon, true);
            }
            sourceSize.width: 52
            sourceSize.height: 52
            fillMode: Image.PreserveAspectFit
            asynchronous: true
        }

        // Apps without an icon in the theme: `symbol`, or a note.
        Symbol {
            anchors.centerIn: parent
            visible: root.appIcon !== "" && picture.status !== Image.Ready
            name: root.symbol !== "" ? root.symbol : "music"
            size: 20
            color: Theme.muted
            opacity: root.muted ? 0.35 : 1
        }

        Symbol {
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            visible: root.appIcon !== "" && root.muted
            name: root.mutedSymbol !== "" ? root.mutedSymbol : "volume-muted"
            size: 14
            color: Theme.foreground
        }
    }

    Item {
        id: heading

        anchors.left: mute.right
        anchors.leftMargin: Theme.spaceSmall
        anchors.right: routeButton.visible ? routeButton.left : parent.right
        anchors.rightMargin: routeButton.visible ? 4 : 0
        anchors.top: parent.top
        anchors.topMargin: Theme.spaceTiny
        height: 20

        Text {
            id: name

            width: Math.min(implicitWidth, parent.width - (root.choosable ? chevron.width + 4 : 0))
            anchors.verticalCenter: parent.verticalCenter
            text: root.title
            elide: Text.ElideRight
            textFormat: Text.PlainText
            color: Theme.foreground
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
            font.weight: Theme.weightTitle
        }

        Symbol {
            id: chevron

            anchors.left: name.right
            anchors.leftMargin: Theme.spaceTiny
            anchors.verticalCenter: parent.verticalCenter
            visible: root.choosable
            name: "chevron"
            rotation: root.choosing ? 270 : 90
            size: 12
            color: Theme.muted
        }

        Text {
            anchors.left: root.choosable ? chevron.right : name.right
            anchors.leftMargin: Theme.spaceSmall
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            visible: root.subtitle !== ""
            text: root.subtitle
            elide: Text.ElideRight
            textFormat: Text.PlainText
            color: Theme.muted
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }

        MouseArea {
            anchors.fill: parent
            enabled: root.choosable
            cursorShape: Qt.PointingHandCursor
            onClicked: root.choose()
        }
    }

    IconButton {
        id: routeButton

        anchors.right: parent.right
        anchors.verticalCenter: heading.verticalCenter
        visible: root.routeIcon !== ""
        icon: root.routeIcon
        size: 14
        tone: root.routing ? "neutral" : "ghost"
        onClicked: root.route()
    }

    Slider {
        id: slider

        anchors.left: heading.left
        anchors.right: percent.left
        anchors.rightMargin: Theme.spaceSmall
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.spaceTiny
        thickness: 6
        // Above 100%, louder than the device's normal level, in the accent
        // color, as in the OSD.
        fill: root.muted ? Theme.muted : shownPercent > 100 ? Theme.accent : Theme.foreground
        value: root.volume / root.maxVolume
        // A double click puts it back to 100%.
        reset: 100 / root.maxVolume
        level: root.level < 0 ? -1 : root.level * 100 / root.maxVolume

        readonly property int shownPercent: dragging ? Math.round(shown * root.maxVolume) : root.volume

        // Where 100% is, when the slider goes further.
        Rectangle {
            visible: root.maxVolume > 100
            x: slider.width * 100 / root.maxVolume - 1
            anchors.verticalCenter: parent.verticalCenter
            width: 2
            height: slider.thickness + 8
            radius: 1
            color: Theme.muted
        }
        onMoved: value => {
            root.pending = Math.round(value * root.maxVolume);
            if (!throttle.running)
                throttle.start();
        }
        onReleased: value => {
            throttle.stop();
            root.pending = -1;
            root.send(Math.round(value * root.maxVolume));
        }
    }

    // As wide as "100%", so the slider keeps its length.
    Item {
        id: percent

        anchors.right: parent.right
        anchors.verticalCenter: slider.verticalCenter
        width: widest.advanceWidth
        height: number.height

        RollingText {
            id: number

            anchors.right: parent.right
            text: `${slider.shownPercent}%`
            color: Theme.muted
            pixelSize: Theme.textCaption
        }

        TextMetrics {
            id: widest

            font: number.font
            text: "100%"
        }
    }
}
