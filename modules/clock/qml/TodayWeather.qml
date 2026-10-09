import QtQuick
import qs.island
import "Time.js" as Time

// Today's weather, from the weather module's state: the sky and the
// temperature now with the place, the details, the next hours and the next
// days. Without a forecast it says why, with the button that fixes it: turn
// the module on, set a place, change one Open-Meteo doesn't know, or try
// again after a failure.
Rectangle {
    id: root

    property var weather: null
    // Whether the weather module runs.
    property bool on: false
    property bool twelve: false

    readonly property bool configured: weather?.configured ?? false
    readonly property var current: weather?.current ?? null
    readonly property var hours: (weather?.hourly ?? []).slice(0, 8)
    readonly property var days: (weather?.daily ?? []).slice(0, 5)
    readonly property string unit: weather?.unit?.temperature ?? "°"
    readonly property string speed: weather?.unit?.speed ?? "km/h"
    // What the button under a missing forecast does.
    readonly property string fix: {
        if (!on)
            return "enable";
        if (weather === null)
            return "";
        if (!configured)
            return "set";
        if (weather.loading || weather.error == null)
            return "";
        return weather.next == null ? "change" : "retry";
    }

    function degrees(value: real): string {
        return `${Math.round(value)}°`;
    }

    radius: Theme.radiusSurface
    color: Theme.surface

    // No forecast: why, and what to do about it.
    Column {
        anchors.centerIn: parent
        width: parent.width - Theme.spaceHuge * 2
        visible: root.current === null
        spacing: Theme.spaceMedium

        Symbol {
            anchors.horizontalCenter: parent.horizontalCenter
            name: !root.on ? "cloud_off" : root.configured ? "cloud" : "location_on"
            size: 28
            color: Theme.muted
        }

        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
            maximumLineCount: 3
            elide: Text.ElideRight
            text: {
                if (!root.on)
                    return "The weather module is off. Turn it on for the forecast here.";
                if (root.weather === null || root.weather.loading)
                    return "Fetching the weather…";
                if (!root.configured)
                    return "Set a place to see its weather here.";
                return root.weather.error ?? "No forecast yet.";
            }
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        ActionButton {
            anchors.horizontalCenter: parent.horizontalCenter
            visible: root.fix !== ""
            text: ({
                    "enable": "Open the weather settings",
                    "set": "Set a place",
                    "change": "Change the place",
                    "retry": "Try again"
                })[root.fix] ?? ""
            icon: root.fix === "retry" ? "refresh" : root.fix === "enable" ? "settings" : "edit"
            onClicked: {
                if (root.fix === "retry")
                    Daemon.command("weather", "refresh", []);
                else if (root.fix === "enable")
                    Daemon.command("settings", "open", ["weather"]);
                else
                    Daemon.command("settings", "open", ["config.module.weather.place"]);
            }
        }
    }

    // Spread over the card's height, the days at the bottom.
    Column {
        x: Theme.spaceLarge
        y: Theme.spaceLarge
        width: parent.width - Theme.spaceLarge * 2
        height: parent.height - Theme.spaceLarge * 2
        visible: root.current !== null
        spacing: Math.max(Theme.spaceSmall, (height - now.height - details.height - hoursRow.height - 1 - daysRow.height) / 4)

        // The sky and the temperature now, and where.
        Item {
            id: now

            width: parent.width
            height: Math.max(sky.height, place.height)

            Row {
                id: sky

                spacing: Theme.spaceMedium

                Symbol {
                    anchors.verticalCenter: parent.verticalCenter
                    name: root.current?.icon ?? "cloud"
                    size: 40
                    color: Theme.foreground
                }

                Column {
                    anchors.verticalCenter: parent.verticalCenter

                    RollingText {
                        text: `${Math.round(root.current?.temperature ?? 0)}${root.unit}`
                        pixelSize: Theme.textHeadline
                        family: Theme.displayFamily
                        weight: Theme.weightTitle
                    }

                    Text {
                        text: {
                            const today = root.days[0];
                            const range = today ? ` · ${root.degrees(today.min)} / ${root.degrees(today.max)}` : "";
                            return `${root.current?.text ?? ""}${range}`;
                        }
                        color: Theme.muted
                        font.pixelSize: Theme.textCaption
                        font.family: Theme.fontFamily
                    }
                }
            }

            Text {
                id: place

                anchors.right: parent.right
                anchors.top: parent.top
                width: Math.min(implicitWidth, parent.width - sky.width - Theme.spaceMedium)
                elide: Text.ElideRight
                text: root.weather?.place?.name ?? ""
                color: Theme.muted
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
                font.weight: Theme.weightLabel
            }
        }

        // The details, as chips.
        Flow {
            id: details

            width: parent.width
            spacing: Theme.spaceSmall

            Repeater {
                model: root.current ? [
                    {
                        "icon": "thermostat",
                        "text": `Feels ${root.degrees(root.current.feels_like)}`
                    },
                    {
                        "icon": "humidity_percentage",
                        "text": `${Math.round(root.current.humidity)}%`
                    },
                    {
                        "icon": "air",
                        "text": `${Math.round(root.current.wind_speed)} ${root.speed}`
                    },
                    {
                        "icon": "wb_sunny",
                        "text": `UV ${Math.round(root.current.uv_index ?? 0)}`
                    }
                ] : []

                Rectangle {
                    id: detail

                    required property var modelData

                    width: chip.implicitWidth + Theme.spaceMedium * 2
                    height: 24
                    radius: Theme.radiusControl
                    color: Theme.raised

                    Row {
                        id: chip

                        anchors.centerIn: parent
                        spacing: Theme.spaceTiny

                        Symbol {
                            anchors.verticalCenter: parent.verticalCenter
                            name: detail.modelData.icon
                            size: 14
                            color: Theme.muted
                        }

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: detail.modelData.text
                            color: Theme.foreground
                            font.pixelSize: Theme.textCaption
                            font.family: Theme.fontFamily
                        }
                    }
                }
            }
        }

        // The next hours, from the one that runs.
        Row {
            id: hoursRow

            width: parent.width

            Repeater {
                model: root.hours

                Column {
                    required property var modelData
                    required property int index

                    width: parent.width / Math.max(1, root.hours.length)
                    spacing: 2

                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: parent.index === 0 ? "Now" : Time.hour(parent.modelData.hour, root.twelve)
                        color: parent.index === 0 ? Theme.foreground : Theme.muted
                        font.pixelSize: Theme.textCaption
                        font.family: Theme.fontFamily
                        font.weight: parent.index === 0 ? Theme.weightTitle : Theme.weightBody
                    }

                    Symbol {
                        anchors.horizontalCenter: parent.horizontalCenter
                        name: parent.modelData.icon
                        size: 18
                        color: Theme.foreground
                    }

                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        // The running hour has the temperature now.
                        text: root.degrees(parent.index === 0 ? root.current?.temperature ?? 0 : parent.modelData.temperature)
                        color: Theme.foreground
                        font.pixelSize: Theme.textCaption
                        font.family: Theme.fontFamily
                        font.weight: Theme.weightLabel
                    }
                }
            }
        }

        Rectangle {
            width: parent.width
            height: 1
            color: Theme.raised
        }

        // The next days, each with its sky, its low and its high.
        Row {
            id: daysRow

            width: parent.width

            Repeater {
                model: root.days

                Column {
                    id: dayItem

                    required property var modelData
                    required property int index

                    width: parent.width / Math.max(1, root.days.length)
                    spacing: 2

                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: dayItem.index === 0 ? "Today" : Qt.locale().dayName(dayItem.modelData.weekday, Locale.ShortFormat)
                        color: Theme.muted
                        font.pixelSize: Theme.textCaption
                        font.family: Theme.fontFamily
                    }

                    Symbol {
                        anchors.horizontalCenter: parent.horizontalCenter
                        name: dayItem.modelData.icon
                        size: 18
                        color: Theme.foreground
                    }

                    Row {
                        anchors.horizontalCenter: parent.horizontalCenter
                        spacing: Theme.spaceTiny

                        Text {
                            text: root.degrees(dayItem.modelData.min)
                            color: Theme.muted
                            font.pixelSize: Theme.textCaption
                            font.family: Theme.fontFamily
                        }

                        Text {
                            text: root.degrees(dayItem.modelData.max)
                            color: Theme.foreground
                            font.pixelSize: Theme.textCaption
                            font.family: Theme.fontFamily
                            font.weight: Theme.weightLabel
                        }
                    }
                }
            }
        }
    }
}
