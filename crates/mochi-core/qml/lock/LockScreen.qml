import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.island

// The lock screen, as the renderer draws it on mochi-lock's private
// display: a panel on each monitor, with the widgets arranged for it over
// the background. It never sees a key: the field shows how many characters
// mochi-lock holds, from the `lock` state, and what went wrong. Without a
// layout, a fixed one: the clock, the date and the weather on the left,
// what's playing and the field on the right, the battery in the corner.
//
// Software rendering draws this, so no shader effects: the wallpaper
// comes blurred from mochid.
Variants {
    model: Quickshell.screens

    PanelWindow {
        id: window

        required property ShellScreen modelData

        readonly property var lock: Daemon.state("lock") ?? ({})
        readonly property var media: Daemon.state("media")
        readonly property var battery: Daemon.state("battery")
        readonly property var weather: Daemon.state("weather")
        readonly property real gutter: Math.round(width * 0.07)

        screen: modelData
        anchors {
            top: true
            bottom: true
            left: true
            right: true
        }
        exclusionMode: ExclusionMode.Ignore
        color: Theme.background

        // The time, kept here: the lock screen needs no daemon for it.
        property date now: new Date()

        Timer {
            interval: 1000
            running: true
            repeat: true
            onTriggered: window.now = new Date()
        }

        Image {
            anchors.fill: parent
            visible: status === Image.Ready
            source: window.lock.background ? `file://${window.lock.background}` : ""
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            smooth: true
        }

        // Dims the wallpaper, so the text reads on any of them.
        Rectangle {
            anchors.fill: parent
            color: Theme.background
            opacity: window.lock.background ? 0.45 : 0
        }

        // The layout arranged with `mochi ipc lock edit`, drawn by the
        // widgets module's own view: mochi-lock passes the lock screen's
        // layout on as the widgets module's state.
        Loader {
            id: arranged

            anchors.fill: parent
            active: (Daemon.state("widgets")?.widgets ?? []).length > 0
            source: "root:/modules/widgets/Desktop.qml"
        }

        Binding {
            target: arranged.item
            property: "output"
            value: window.modelData.name
            when: arranged.item !== null
        }

        // Without a layout, or when its view can't load: this one.
        Item {
            anchors.fill: parent
            visible: arranged.status !== Loader.Ready

            // The clock: the focal point.
            Column {
                x: window.gutter
                anchors.verticalCenter: parent.verticalCenter
                spacing: Theme.spaceLarge

                StackedTime {
                    width: Math.round(window.width * 0.32)
                    height: Math.round(window.height * 0.46)
                    horizontalAlignment: Text.AlignLeft
                    animated: false
                    hours: {
                        const hour = window.now.getHours();
                        const shown = window.lock.hours === "12" ? (hour % 12 || 12) : hour;
                        return String(shown).padStart(2, "0");
                    }
                    minutes: String(window.now.getMinutes()).padStart(2, "0")
                }

                Text {
                    text: window.now.toLocaleDateString(Qt.locale(), "dddd d MMMM")
                    color: Theme.foreground
                    font.pixelSize: Theme.textHeadline
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightLabel
                }

                Row {
                    visible: (window.weather?.current ?? null) !== null
                    spacing: Theme.spaceSmall

                    Symbol {
                        anchors.verticalCenter: parent.verticalCenter
                        name: window.weather?.current?.icon ?? "cloud"
                        size: Theme.textHeadline
                        color: Theme.muted
                    }

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: {
                            const current = window.weather?.current;
                            if (!current)
                                return "";
                            const unit = window.weather?.unit ?? "°";
                            return [`${Math.round(current.temperature ?? 0)}${unit}`, current.text].filter(part => part).join("  ");
                        }
                        color: Theme.muted
                        font.pixelSize: Theme.textTitle
                        font.family: Theme.fontFamily
                    }
                }
            }

            // What's playing, and the field.
            Column {
                id: side

                width: Math.min(380, window.width * 0.3)
                x: window.width - window.gutter - width
                anchors.verticalCenter: parent.verticalCenter
                spacing: Theme.spaceHuge

                Rectangle {
                    id: playing

                    readonly property var media: window.media

                    visible: media !== null && media !== undefined && (media.title ?? "") !== ""
                    width: parent.width
                    height: Theme.spaceMedium * 2 + 64
                    radius: Theme.radiusSurface
                    color: Theme.surface

                    Row {
                        x: Theme.spaceMedium
                        y: Theme.spaceMedium
                        width: parent.width - Theme.spaceMedium * 2
                        spacing: Theme.spaceMedium

                        Image {
                            width: 64
                            height: 64
                            source: playing.media?.art ?? ""
                            fillMode: Image.PreserveAspectCrop
                            asynchronous: true
                            visible: status === Image.Ready
                        }

                        Column {
                            width: parent.width - (64 + Theme.spaceMedium) - controls.width - Theme.spaceMedium
                            anchors.verticalCenter: parent.verticalCenter
                            spacing: 2

                            Text {
                                width: parent.width
                                elide: Text.ElideRight
                                text: playing.media?.title ?? ""
                                color: Theme.foreground
                                font.pixelSize: Theme.textBody
                                font.family: Theme.fontFamily
                                font.weight: Theme.weightTitle
                            }

                            Text {
                                width: parent.width
                                elide: Text.ElideRight
                                text: playing.media?.artist ?? ""
                                color: Theme.muted
                                font.pixelSize: Theme.textCaption
                                font.family: Theme.fontFamily
                            }
                        }

                        Row {
                            id: controls

                            anchors.verticalCenter: parent.verticalCenter

                            IconButton {
                                visible: playing.media?.can_previous ?? false
                                icon: "previous"
                                onClicked: Daemon.command("media", "previous", [])
                            }

                            IconButton {
                                icon: playing.media?.status === "playing" ? "pause" : "play"
                                tone: "accent"
                                onClicked: Daemon.command("media", "play-pause", [])
                            }

                            IconButton {
                                visible: playing.media?.can_next ?? false
                                icon: "next"
                                onClicked: Daemon.command("media", "next", [])
                            }
                        }
                    }
                }

                Column {
                    width: parent.width
                    spacing: Theme.spaceSmall

                    Text {
                        text: window.lock.user ? `Locked · ${window.lock.user}` : "Locked"
                        color: Theme.muted
                        font.pixelSize: Theme.textCaption
                        font.family: Theme.fontFamily
                        font.weight: Theme.weightLabel
                    }

                    // The field: a dot for each character mochi-lock holds.
                    Rectangle {
                        readonly property bool failed: (window.lock.message ?? null) !== null && (window.lock.typed ?? 0) === 0

                        width: parent.width
                        height: 52
                        radius: height / 2
                        color: Theme.raised
                        border.width: failed ? 1 : 0
                        border.color: Theme.danger

                        Symbol {
                            x: Theme.spaceLarge
                            anchors.verticalCenter: parent.verticalCenter
                            name: "lock"
                            size: Theme.textTitle
                            color: Theme.muted
                        }

                        Row {
                            anchors.centerIn: parent
                            spacing: Theme.spaceSmall
                            visible: (window.lock.typed ?? 0) > 0 && window.lock.phase !== "checking"

                            Repeater {
                                model: Math.min(window.lock.typed ?? 0, 18)

                                Rectangle {
                                    width: 9
                                    height: 9
                                    radius: width / 2
                                    color: Theme.foreground
                                }
                            }
                        }

                        Text {
                            anchors.centerIn: parent
                            visible: (window.lock.typed ?? 0) === 0 || window.lock.phase === "checking"
                            text: {
                                switch (window.lock.phase) {
                                case "checking":
                                    return "Checking…";
                                case "waiting":
                                    return `Try again in ${window.lock.wait} s`;
                                default:
                                    return "Type your password";
                                }
                            }
                            color: Theme.muted
                            font.pixelSize: Theme.textBody
                            font.family: Theme.fontFamily
                        }
                    }

                    Text {
                        width: parent.width
                        visible: text !== ""
                        wrapMode: Text.Wrap
                        text: window.lock.message ?? ""
                        color: Theme.danger
                        font.pixelSize: Theme.textCaption
                        font.family: Theme.fontFamily
                    }

                    Text {
                        visible: (window.lock.capsLock ?? false) && (window.lock.message ?? null) === null
                        text: "Caps Lock is on"
                        color: Theme.accent
                        font.pixelSize: Theme.textCaption
                        font.family: Theme.fontFamily
                        font.weight: Theme.weightLabel
                    }
                }
            }

            // The battery, in the corner, when there is one.
            Row {
                visible: window.battery?.present ?? false
                anchors.top: parent.top
                anchors.right: parent.right
                anchors.margins: Theme.spaceHuge
                spacing: Theme.spaceTiny

                Symbol {
                    anchors.verticalCenter: parent.verticalCenter
                    name: window.battery?.charging ? "battery_charging_full" : "battery_full"
                    size: Theme.textTitle
                    color: (window.battery?.critical ?? false) ? Theme.danger : Theme.muted
                }

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: `${Math.round(window.battery?.level ?? 0)}%`
                    color: Theme.muted
                    font.pixelSize: Theme.textBody
                    font.family: Theme.fontFamily
                }
            }
        }
    }
}
