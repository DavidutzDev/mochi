import QtQuick
import qs.island

// The control center's Weather page. A header with the place, how old the
// forecast is while it's stale, and buttons to fetch it again and change the
// place. Under it, the weather now beside the next 7 days, then the next 24
// hours as a curve. The weather now is the sky in a cookie and the
// temperature, with what it feels like, the humidity, the wind, the UV index,
// and today's sunrise and sunset under them. The days and the hours are the
// widget's forecast and hours looks. Without a forecast, it says why, with
// the button that fixes it.
Item {
    id: root

    property var payload: null
    readonly property var current: payload?.current ?? null
    readonly property var today: payload?.daily?.[0] ?? null
    readonly property var place: payload?.place ?? null
    readonly property string unit: payload?.unit?.temperature ?? "°"
    readonly property string speed: payload?.unit?.speed ?? ""
    // The place's offset from UTC, for its sunrise and sunset.
    readonly property int offset: payload?.utc_offset ?? 0
    readonly property int gap: Theme.spaceMedium
    // A pane's padding, and its heading's height with the space under it,
    // as the home's cards have them.
    readonly property int inset: Theme.spaceMedium
    readonly property int heading: Theme.textCaption + Theme.spaceSmall * 2
    // The details under the weather now, each left out while the forecast
    // has no value for it.
    readonly property var details: {
        const now = current ?? {};
        const list = [];
        if (now.feels_like != null)
            list.push({
                "icon": "thermostat",
                "label": "Feels like",
                "value": degrees(now.feels_like),
                "note": ""
            });
        if (now.humidity != null)
            list.push({
                "icon": "humidity_percentage",
                "label": "Humidity",
                "value": `${Math.round(now.humidity)}%`,
                "note": ""
            });
        if (now.wind_speed != null)
            list.push({
                "icon": "air",
                "label": "Wind",
                "value": `${Math.round(now.wind_speed)} ${speed}`,
                "note": now.wind_direction != null ? `from ${compass(now.wind_direction)}` : ""
            });
        if (now.uv_index != null)
            list.push({
                "icon": "light_mode",
                "label": "UV index",
                "value": `${Math.round(now.uv_index)}`,
                "note": exposure(now.uv_index)
            });
        if (today?.sunrise != null)
            list.push({
                "icon": "wb_twilight",
                "label": "Sunrise",
                "value": clock(today.sunrise),
                "note": ""
            });
        if (today?.sunset != null)
            list.push({
                "icon": "bedtime",
                "label": "Sunset",
                "value": clock(today.sunset),
                "note": ""
            });
        return list;
    }

    function degrees(value: real): string {
        return `${Math.round(value)}°`;
    }

    // Seconds since the epoch as the time at the place, like 07:48.
    function clock(seconds: real): string {
        const at = new Date((seconds + offset) * 1000);
        return `${String(at.getUTCHours()).padStart(2, "0")}:${String(at.getUTCMinutes()).padStart(2, "0")}`;
    }

    // Where the wind comes from, on eight points.
    function compass(direction: real): string {
        return ["N", "NE", "E", "SE", "S", "SW", "W", "NW"][Math.round(((direction % 360) + 360) % 360 / 45) % 8];
    }

    // The WHO's words for a UV index.
    function exposure(index: real): string {
        if (index < 3)
            return "Low";
        if (index < 6)
            return "Moderate";
        if (index < 8)
            return "High";
        if (index < 11)
            return "Very high";
        return "Extreme";
    }

    implicitHeight: column.implicitHeight

    // A surface with an icon and a title at the top, like a card on the
    // home, or without them when the title is empty; what's put inside
    // fills the rest.
    component Pane: Rectangle {
        id: pane

        property string icon: ""
        property string title: ""
        property bool working: false
        default property alias content: holder.data

        radius: Theme.radiusSurface
        color: Theme.surface
        clip: true

        EdgeLight {
            radius: pane.radius
            working: pane.working
        }

        Row {
            x: root.inset
            y: root.inset
            visible: pane.title !== ""
            height: Theme.textCaption + Theme.spaceTiny
            spacing: Theme.spaceTiny

            Symbol {
                anchors.verticalCenter: parent.verticalCenter
                name: pane.icon
                size: Theme.textBody
                color: Theme.muted
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: pane.title
                color: Theme.muted
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
                font.weight: Theme.weightLabel
            }
        }

        Item {
            id: holder

            x: root.inset
            y: root.inset + (pane.title !== "" ? root.heading : 0)
            width: parent.width - root.inset * 2
            height: parent.height - y - root.inset
        }
    }

    Column {
        id: column

        width: parent.width
        spacing: root.gap

        PanelHeader {
            width: parent.width
            icon: "location_on"
            title: {
                if (!root.place)
                    return "Weather";
                return [root.place.name, root.place.country].filter(part => part).join(", ");
            }

            Age {
                anchors.verticalCenter: parent.verticalCenter
                width: implicitWidth
                visible: stale && root.current !== null
                payload: root.payload
            }

            IconButton {
                anchors.verticalCenter: parent.verticalCenter
                visible: root.payload?.configured ?? false
                enabled: !(root.payload?.loading ?? false)
                icon: "refresh"
                onClicked: Daemon.command("weather", "refresh", [])
            }

            IconButton {
                anchors.verticalCenter: parent.verticalCenter
                visible: Daemon.modules.includes("settings")
                icon: "edit_location_alt"
                onClicked: Daemon.command("settings", "open", ["config.module.weather.place"])
            }
        }

        // No place yet, or no forecast yet: the place's pin, or a cloud,
        // in a quiet cookie, and why, with the button that fixes it.
        Pane {
            width: parent.width
            height: 120
            visible: root.current === null
            working: root.payload?.loading ?? false

            Row {
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width
                spacing: Theme.spaceLarge

                ExpressiveShape {
                    id: waiting

                    anchors.verticalCenter: parent.verticalCenter
                    width: 76
                    height: 76
                    shape: "cookie"
                    color: Theme.raised

                    Symbol {
                        anchors.centerIn: parent
                        name: root.payload?.configured ? "cloud" : "location_on"
                        size: 34
                        color: Theme.muted
                    }
                }

                Missing {
                    anchors.verticalCenter: parent.verticalCenter
                    width: Math.min(parent.width - waiting.width - parent.spacing, 420)
                    payload: root.payload
                    room: 96
                }
            }
        }

        Row {
            width: parent.width
            visible: root.current !== null
            spacing: root.gap

            readonly property real half: (width - spacing) / 2

            Pane {
                id: now

                width: parent.half
                height: root.inset * 2 + root.heading + sky.height + Theme.spaceLarge + facts.height
                icon: "partly_cloudy_day"
                title: "Now"
                working: root.payload?.loading ?? false

                Row {
                    id: sky

                    width: parent.width
                    spacing: Theme.spaceLarge

                    ExpressiveShape {
                        id: cookie

                        width: 76
                        height: 76
                        shape: "cookie"

                        Symbol {
                            anchors.centerIn: parent
                            name: root.current?.icon ?? "cloud"
                            size: 38
                            color: Theme.onAccent
                            filled: true
                        }
                    }

                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.width - cookie.width - parent.spacing

                        RollingText {
                            text: `${Math.round(root.current?.temperature ?? 0)}${root.unit}`
                            pixelSize: Theme.textDisplay
                            family: Theme.displayFamily
                            weight: Theme.weightTitle
                        }

                        Text {
                            width: parent.width
                            text: root.current?.text ?? ""
                            elide: Text.ElideRight
                            color: Theme.foreground
                            font.pixelSize: Theme.textBody
                            font.family: Theme.fontFamily
                            font.weight: Theme.weightLabel
                        }

                        Text {
                            width: parent.width
                            visible: root.today !== null
                            text: root.today ? `Today ${root.degrees(root.today.min)} to ${root.degrees(root.today.max)}` : ""
                            elide: Text.ElideRight
                            color: Theme.muted
                            font.pixelSize: Theme.textCaption
                            font.family: Theme.fontFamily
                        }
                    }
                }

                Grid {
                    id: facts

                    y: sky.height + Theme.spaceLarge
                    width: parent.width
                    columns: 3
                    rowSpacing: Theme.spaceMedium
                    columnSpacing: Theme.spaceSmall

                    Repeater {
                        model: root.details

                        Row {
                            id: fact

                            required property var modelData

                            width: (facts.width - facts.columnSpacing * 2) / 3
                            spacing: Theme.spaceSmall

                            Symbol {
                                id: mark

                                anchors.verticalCenter: parent.verticalCenter
                                name: fact.modelData.icon
                                size: 18
                                color: Theme.muted
                            }

                            Column {
                                anchors.verticalCenter: parent.verticalCenter
                                width: fact.width - mark.width - fact.spacing

                                Text {
                                    width: parent.width
                                    text: fact.modelData.label
                                    elide: Text.ElideRight
                                    color: Theme.muted
                                    font.pixelSize: Theme.textCaption
                                    font.family: Theme.fontFamily
                                }

                                // The value, and a word on it, like the
                                // wind's direction.
                                Row {
                                    width: parent.width
                                    spacing: Theme.spaceTiny

                                    Text {
                                        id: value

                                        width: Math.min(implicitWidth, parent.width)
                                        text: fact.modelData.value
                                        elide: Text.ElideRight
                                        color: Theme.foreground
                                        font.pixelSize: Theme.textBody
                                        font.family: Theme.fontFamily
                                        font.weight: Theme.weightLabel
                                    }

                                    Text {
                                        anchors.baseline: value.baseline
                                        width: Math.max(0, parent.width - value.width - parent.spacing)
                                        visible: fact.modelData.note !== ""
                                        text: fact.modelData.note
                                        elide: Text.ElideRight
                                        color: Theme.muted
                                        font.pixelSize: Theme.textCaption
                                        font.family: Theme.fontFamily
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Pane {
                width: parent.half
                height: now.height
                icon: "calendar_month"
                title: "7 days"

                Forecast {
                    anchors.fill: parent
                    payload: root.payload
                    showAge: false
                }
            }
        }

        Pane {
            width: parent.width
            height: root.inset * 2 + root.heading + 150
            visible: root.current !== null
            icon: "schedule"
            title: "Next 24 hours"

            Hours {
                anchors.fill: parent
                payload: root.payload
                count: 24
                showAge: false
            }
        }
    }
}
