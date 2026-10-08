import QtQuick
import qs.island

// Picks a color: saturation and brightness on the square, the hue and the
// opacity on the bars under it, or the hex typed in. `picked` follows the
// pointer; the owner sends it on.
Rectangle {
    id: root

    property string value: "#000000" // design: a value to edit, not a color to draw
    signal picked(string value)

    property real hue: 0
    property real saturation: 0
    property real brightness: 0
    property real alpha: 1
    // Set while the pointer moves, so the daemon's answers don't move the
    // marks back under it.
    property bool dragging: false

    width: 248
    height: column.implicitHeight + Theme.spaceMedium * 2
    radius: Theme.radiusSurface
    color: Theme.surface
    border.width: 1
    border.color: Theme.raised

    onValueChanged: {
        if (!dragging)
            read(value);
    }
    Component.onCompleted: read(value)

    function read(text: string): void {
        const parsed = Qt.color(text);
        // A gray has no hue: keep the one picked.
        if (parsed.hsvHue >= 0)
            hue = parsed.hsvHue;
        saturation = parsed.hsvSaturation;
        brightness = parsed.hsvValue;
        alpha = parsed.a;
    }

    // #rrggbb, or #aarrggbb when it isn't opaque, as theme.toml takes them.
    function hex(): string {
        const picked = Qt.hsva(hue, saturation, brightness, alpha);
        const byte = value => Math.round(value * 255).toString(16).padStart(2, "0");
        const rgb = byte(picked.r) + byte(picked.g) + byte(picked.b);
        return "#" + (alpha < 1 ? byte(alpha) : "") + rgb;
    }

    function update(): void {
        root.picked(hex());
    }

    MouseArea {
        // Clicks inside don't reach the panel, which closes it.
        anchors.fill: parent
    }

    Column {
        id: column

        x: Theme.spaceMedium
        y: Theme.spaceMedium
        width: parent.width - Theme.spaceMedium * 2
        spacing: Theme.spaceMedium

        Rectangle {
            id: square

            width: parent.width
            height: 150
            radius: Theme.radiusControl
            color: Qt.hsva(root.hue, 1, 1, 1)

            Rectangle {
                anchors.fill: parent
                radius: parent.radius
                gradient: Gradient {
                    orientation: Gradient.Horizontal
                    GradientStop {
                        position: 0
                        color: "white" // design: the saturation ramp starts at white
                    }
                    GradientStop {
                        position: 1
                        color: "transparent"
                    }
                }
            }

            Rectangle {
                anchors.fill: parent
                radius: parent.radius
                gradient: Gradient {
                    GradientStop {
                        position: 0
                        color: "transparent"
                    }
                    GradientStop {
                        position: 1
                        color: "black" // design: the brightness ramp ends at black
                    }
                }
            }

            Rectangle {
                x: root.saturation * square.width - width / 2
                y: (1 - root.brightness) * square.height - height / 2
                width: 14
                height: 14
                radius: width / 2
                color: "transparent"
                border.width: 2
                border.color: "white" // design: the mark shows on any color
            }

            MouseArea {
                anchors.fill: parent
                cursorShape: Qt.CrossCursor
                onPressed: mouse => pick(mouse)
                onPositionChanged: mouse => pick(mouse)
                onReleased: root.dragging = false

                function pick(mouse: var): void {
                    root.dragging = true;
                    root.saturation = Math.max(0, Math.min(1, mouse.x / width));
                    root.brightness = Math.max(0, Math.min(1, 1 - mouse.y / height));
                    root.update();
                }
            }
        }

        // The hue bar, then the opacity bar.
        Repeater {
            model: ["hue", "alpha"]

            Rectangle {
                id: bar

                required property string modelData
                readonly property real at: modelData === "hue" ? root.hue : root.alpha

                width: column.width
                height: 14
                radius: height / 2
                color: Theme.raised
                gradient: modelData === "hue" ? hues : fade

                Gradient {
                    id: hues

                    orientation: Gradient.Horizontal
                    GradientStop {
                        position: 0
                        color: Qt.hsva(0, 1, 1, 1)
                    }
                    GradientStop {
                        position: 1 / 6
                        color: Qt.hsva(1 / 6, 1, 1, 1)
                    }
                    GradientStop {
                        position: 2 / 6
                        color: Qt.hsva(2 / 6, 1, 1, 1)
                    }
                    GradientStop {
                        position: 3 / 6
                        color: Qt.hsva(3 / 6, 1, 1, 1)
                    }
                    GradientStop {
                        position: 4 / 6
                        color: Qt.hsva(4 / 6, 1, 1, 1)
                    }
                    GradientStop {
                        position: 5 / 6
                        color: Qt.hsva(5 / 6, 1, 1, 1)
                    }
                    GradientStop {
                        position: 1
                        color: Qt.hsva(1, 1, 1, 1)
                    }
                }

                Gradient {
                    id: fade

                    orientation: Gradient.Horizontal
                    GradientStop {
                        position: 0
                        color: Qt.hsva(root.hue, root.saturation, root.brightness, 0)
                    }
                    GradientStop {
                        position: 1
                        color: Qt.hsva(root.hue, root.saturation, root.brightness, 1)
                    }
                }

                Rectangle {
                    x: bar.at * (bar.width - width)
                    anchors.verticalCenter: parent.verticalCenter
                    width: 18
                    height: 18
                    radius: width / 2
                    color: "transparent"
                    border.width: 2
                    border.color: "white" // design: the mark shows on any color
                }

                MouseArea {
                    anchors.fill: parent
                    anchors.margins: -4
                    cursorShape: Qt.PointingHandCursor
                    onPressed: mouse => pick(mouse)
                    onPositionChanged: mouse => pick(mouse)
                    onReleased: root.dragging = false

                    function pick(mouse: var): void {
                        root.dragging = true;
                        const at = Math.max(0, Math.min(1, (mouse.x - 4) / bar.width));
                        if (bar.modelData === "hue")
                            root.hue = Math.min(at, 0.999);
                        else
                            root.alpha = at;
                        root.update();
                    }
                }
            }
        }

        Row {
            width: parent.width
            spacing: Theme.spaceSmall

            Rectangle {
                width: Theme.controlHeight
                height: Theme.controlHeight
                radius: Theme.radiusControl
                color: root.value
                border.width: 1
                border.color: Theme.highlight
            }

            Entry {
                width: parent.width - Theme.controlHeight - Theme.spaceSmall
                mono: true
                text: root.value
                onAccepted: text => {
                    const typed = text.trim();
                    if (/^#([0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/.test(typed)) {
                        root.read(typed);
                        root.picked(typed.toLowerCase());
                    }
                }
            }
        }
    }
}
