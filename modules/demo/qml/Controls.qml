import QtQuick
import qs.island

// Every built-in control, for trying them and checking a theme. The state
// lives here; nothing is sent anywhere.
Item {
    id: root

    property var payload: ({})
    property bool wifi: true
    property bool bluetooth: false
    property bool airplane: false
    property real volume: 0.6
    property real brightness: 0.35
    property string profile: "balanced"
    property int selected: 0

    implicitWidth: 640
    implicitHeight: column.implicitHeight + 32

    Column {
        id: column

        x: 16
        y: 16
        width: parent.width - 32
        spacing: 12

        SectionLabel {
            text: "Tiles"
        }

        Row {
            width: parent.width
            spacing: 10

            Tile {
                width: (parent.width - 10) / 2
                vertical: false
                icon: "wifi"
                title: "Wi-Fi"
                subtitle: root.wifi ? "Home network" : "Off"
                checked: root.wifi
                onClicked: root.wifi = !root.wifi
            }

            Tile {
                width: (parent.width - 10) / 2
                vertical: false
                icon: "bluetooth"
                title: "Bluetooth"
                subtitle: root.bluetooth ? "Earbuds" : "Off"
                checked: root.bluetooth
                onClicked: root.bluetooth = !root.bluetooth
            }
        }

        SectionLabel {
            text: "Sliders"
        }

        Slider {
            width: parent.width
            icon: "volume"
            value: root.volume
            onMoved: value => root.volume = value
        }

        Slider {
            width: parent.width
            icon: "moon"
            value: root.brightness
            onMoved: value => root.brightness = value
        }

        Row {
            width: parent.width
            spacing: 12

            Slider {
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width - progress.width - toggle.width - 24
                thickness: 4
                value: root.volume
                onMoved: value => root.volume = value
            }

            ProgressBar {
                id: progress

                anchors.verticalCenter: parent.verticalCenter
                width: 120
                value: root.brightness
                fill: Theme.accent
            }

            Switch {
                id: toggle

                anchors.verticalCenter: parent.verticalCenter
                checked: root.airplane
                onToggled: checked => root.airplane = checked
            }
        }

        SectionLabel {
            text: "Choices"
        }

        Segmented {
            width: parent.width
            options: [
                {
                    "value": "power-saver",
                    "label": "Saver",
                    "icon": "leaf"
                },
                {
                    "value": "balanced",
                    "label": "Balanced",
                    "icon": "scale"
                },
                {
                    "value": "performance",
                    "label": "Performance",
                    "icon": "bolt"
                }
            ]
            current: root.profile
            onPicked: value => root.profile = value
        }

        SectionLabel {
            text: "Rows and buttons"
        }

        Repeater {
            model: [
                {
                    "icon": "headset",
                    "title": "Earbuds",
                    "subtitle": "Connected · 80%"
                },
                {
                    "icon": "speakers",
                    "title": "Speakers",
                    "subtitle": "Not connected"
                }
            ]

            ListRow {
                required property var modelData
                required property int index

                width: column.width
                icon: modelData.icon
                title: modelData.title
                subtitle: modelData.subtitle
                selected: root.selected === index
                marker: true
                onClicked: root.selected = index

                trailing: [
                    Badge {
                        anchors.verticalCenter: parent.verticalCenter
                        count: 3
                    },
                    IconButton {
                        icon: "chevron"
                        size: 14
                    }
                ]
            }
        }

        Row {
            spacing: 8

            Button {
                text: "Neutral"
            }

            Button {
                text: "Accent"
                icon: "check"
                tone: "accent"
            }

            Button {
                text: "Danger"
                icon: "power"
                tone: "danger"
            }

            Button {
                text: "Ghost"
                tone: "ghost"
            }

            Button {
                icon: "plus"
            }

            IconButton {
                icon: "play"
                size: 22
            }

            Button {
                text: "Disabled"
                enabled: false
            }
        }

        PanelHeader {
            width: parent.width
            title: "Panel header"
            back: true

            IconButton {
                icon: "search"
            }

            IconButton {
                icon: "edit"
            }
        }

        SwitchRow {
            width: parent.width
            icon: "wifi"
            title: "Switch row"
            subtitle: root.wifi ? "On" : "Off"
            checked: root.wifi
            onToggled: checked => root.wifi = checked
        }

        SliderRow {
            width: parent.width
            icon: "volume"
            value: root.volume
            reset: 1
            opens: true
            onMoved: value => root.volume = value
        }

        // Rolling digits, an edge light that sweeps while working and
        // flashes when done, and a list with fading edges.
        Rectangle {
            id: lit

            property bool working: false
            property int seconds: 0

            width: parent.width
            height: Theme.tileHeight
            radius: Theme.radiusSurface
            color: Theme.surface

            EdgeLight {
                id: light

                radius: lit.radius
                working: lit.working
            }

            Timer {
                interval: 1000
                running: true
                repeat: true
                onTriggered: lit.seconds++
            }

            Row {
                anchors.centerIn: parent
                spacing: Theme.spaceLarge

                RollingText {
                    anchors.verticalCenter: parent.verticalCenter
                    text: `${Math.floor(lit.seconds / 60)}:${String(lit.seconds % 60).padStart(2, "0")}`
                    pixelSize: Theme.textHeadline
                    weight: Theme.weightTitle
                }

                Button {
                    text: lit.working ? "Done" : "Work"
                    onClicked: {
                        if (lit.working)
                            light.flash();
                        lit.working = !lit.working;
                    }
                }
            }
        }

        ListView {
            id: faded

            width: parent.width
            height: Theme.rowHeight * 3
            clip: true
            model: 12
            delegate: ListRow {
                required property int index

                width: faded.width
                flat: true
                icon: "music"
                title: `Row ${index + 1}`
            }

            ScrollFade {
                view: faded
            }
        }
    }
}
