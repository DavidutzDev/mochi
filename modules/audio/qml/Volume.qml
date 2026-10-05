import QtQuick
import Quickshell
import qs.island

// One volume in the mixer: an icon that mutes on click, the name, a slider
// and the percent. `symbol` is a built-in icon, `appIcon` an icon theme
// name for apps. With `choosable`, clicking the name sends `choose`, for
// picking another device.
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
    signal choose

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
            source: root.appIcon === "" ? "" : Quickshell.iconPath(root.appIcon, true)
            sourceSize.width: 52
            sourceSize.height: 52
            fillMode: Image.PreserveAspectFit
            asynchronous: true
        }

        // Apps without an icon in the theme.
        Symbol {
            anchors.centerIn: parent
            visible: root.appIcon !== "" && picture.status !== Image.Ready
            name: "music"
            size: 20
            color: Theme.muted
            opacity: root.muted ? 0.35 : 1
        }

        Symbol {
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            anchors.margins: 2
            visible: root.appIcon !== "" && root.muted
            name: "volume-muted"
            size: 14
            color: Theme.foreground
        }
    }

    Item {
        id: heading

        anchors.left: mute.right
        anchors.leftMargin: 10
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.topMargin: 4
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
            font.weight: Font.DemiBold
        }

        Symbol {
            id: chevron

            anchors.left: name.right
            anchors.leftMargin: 4
            anchors.verticalCenter: parent.verticalCenter
            visible: root.choosable
            name: "chevron"
            rotation: root.choosing ? 270 : 90
            size: 12
            color: Theme.muted
        }

        Text {
            anchors.left: root.choosable ? chevron.right : name.right
            anchors.leftMargin: 8
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            visible: root.subtitle !== ""
            text: root.subtitle
            elide: Text.ElideRight
            textFormat: Text.PlainText
            color: Theme.muted
            font.pixelSize: Theme.textLabel
            font.family: Theme.fontFamily
        }

        MouseArea {
            anchors.fill: parent
            enabled: root.choosable
            cursorShape: Qt.PointingHandCursor
            onClicked: root.choose()
        }
    }

    Slider {
        id: slider

        anchors.left: heading.left
        anchors.right: percent.left
        anchors.rightMargin: 10
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 4
        thickness: 6
        // Above 100%, louder than the device's normal level, in the accent
        // color, as in the OSD.
        fill: root.muted ? Theme.muted : shownPercent > 100 ? Theme.accent : Theme.foreground
        value: root.volume / root.maxVolume

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

    Text {
        id: percent

        anchors.right: parent.right
        anchors.verticalCenter: slider.verticalCenter
        width: 38
        horizontalAlignment: Text.AlignRight
        text: `${slider.shownPercent}%`
        color: Theme.muted
        font.pixelSize: Theme.textLabel
        font.family: Theme.fontFamily
        font.features: { "tnum": 1 }
    }
}
