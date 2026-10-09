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
    property bool playing: true
    property real position: 0.3
    property date now: new Date()

    Timer {
        interval: 1000
        running: true
        repeat: true
        onTriggered: root.now = new Date()
    }

    implicitWidth: 640
    // Taller than the island can be, so it scrolls.
    implicitHeight: Math.min(column.implicitHeight + Theme.spaceLarge * 2, Theme.surfaceHeight)

    Flickable {
        anchors.fill: parent
        contentWidth: width
        contentHeight: column.implicitHeight + Theme.spaceLarge * 2
        clip: true
        boundsBehavior: Flickable.StopAtBounds

        Column {
            id: column

            x: Theme.spaceLarge
            y: Theme.spaceLarge
            width: parent.width - Theme.spaceLarge * 2
            spacing: Theme.spaceMedium

            SectionLabel {
                text: "Tiles"
            }

            Row {
                width: parent.width
                spacing: Theme.spaceSmall

                Tile {
                    width: (parent.width - parent.spacing) / 2
                    vertical: false
                    icon: "wifi"
                    title: "Wi-Fi"
                    subtitle: root.wifi ? "Home network" : "Off"
                    checked: root.wifi
                    onClicked: root.wifi = !root.wifi
                }

                Tile {
                    width: (parent.width - parent.spacing) / 2
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
                spacing: Theme.spaceMedium

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
                text: "Accents"
            }

            // A seek line that waves while playing, stacked numerals, rings and
            // the shapes, each an accent for one widget.
            Row {
                width: parent.width
                spacing: Theme.spaceMedium

                IconButton {
                    id: play

                    anchors.verticalCenter: parent.verticalCenter
                    icon: root.playing ? "pause" : "play"
                    size: 20
                    onClicked: root.playing = !root.playing
                }

                WavyProgress {
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - play.width - parent.spacing
                    playing: root.playing
                    value: root.position
                    onMoved: value => root.position = value
                }
            }

            Row {
                spacing: Theme.spaceMedium

                StackedTime {
                    width: 64
                    height: Theme.tileHeight
                    hours: String(root.now.getHours()).padStart(2, "0")
                    minutes: String(root.now.getMinutes()).padStart(2, "0")
                    date: Qt.locale().toString(root.now, "ddd d MMM")
                }

                WavyRing {
                    anchors.verticalCenter: parent.verticalCenter
                    size: 64
                    value: root.brightness

                    Text {
                        anchors.centerIn: parent
                        text: `${Math.round(root.brightness * 100)}%`
                        color: Theme.foreground
                        font.pixelSize: Theme.textCaption
                        font.family: Theme.fontFamily
                        font.weight: Theme.weightLabel
                    }
                }

                WavyRing {
                    anchors.verticalCenter: parent.verticalCenter
                    size: 64
                    wavy: false
                    color: Theme.foreground
                    value: root.volume

                    Symbol {
                        anchors.centerIn: parent
                        name: "volume"
                        size: 18
                        color: Theme.muted
                    }
                }
            }

            // Every shape, the day's date in each.
            Row {
                spacing: Theme.spaceMedium

                Repeater {
                    model: ["circle", "pentagon", "cookie", "clover", "burst", "hexagon", "octagon", "squircle", "pill"]

                    Column {
                        required property string modelData

                        spacing: Theme.spaceTiny

                        ExpressiveShape {
                            anchors.horizontalCenter: parent.horizontalCenter
                            shape: parent.modelData
                            size: 40

                            Text {
                                anchors.centerIn: parent
                                text: root.now.getDate()
                                color: Theme.onAccent
                                font.pixelSize: Theme.textBody
                                font.family: Theme.fontFamily
                                font.weight: Theme.weightTitle
                            }
                        }

                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: parent.modelData
                            color: Theme.muted
                            font.pixelSize: Theme.textCaption
                            font.family: Theme.fontFamily
                        }
                    }
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
                spacing: Theme.spaceSmall

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
}
