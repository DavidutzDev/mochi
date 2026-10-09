import QtQuick
import qs.island
import "ColorMath.js" as ColorMath

// The control center page: an interactive color chooser matching the reference
// design, side-by-side with color history. Supports picking from screen,
// choosing via 2D Sat/Val gradient and hue slider, setting custom
// hex/rgb/hsl/named colors, copying in multiple formats (HEX, RGB, CMYK, HSV,
// HSL), and managing saved colors.
Item {
    id: root

    property var payload: null
    readonly property var colors: payload?.history ?? []

    // Active color in HSV (normalized 0.0 .. 1.0)
    // Matches the reference coral/orange (#fb542b)
    property real hue: 0.03285
    property real saturation: 0.8287
    property real value: 0.9843

    // Active color RGB and Hex
    readonly property var currentRgb: ColorMath.hsvToRgb(hue, saturation, value)
    readonly property string currentHex: ColorMath.rgbToHex(currentRgb.r, currentRgb.g, currentRgb.b, false)
    readonly property color currentColor: Qt.hsva(hue, saturation, value, 1.0)

    // Formatted strings matching the reference UI
    readonly property string hexText: currentHex
    readonly property string rgbText: ColorMath.formatRgb(currentRgb.r, currentRgb.g, currentRgb.b)
    readonly property string cmykText: ColorMath.formatCmyk(currentRgb.r, currentRgb.g, currentRgb.b)
    readonly property string hsvText: ColorMath.formatHsv(hue, saturation, value)
    readonly property string hslText: ColorMath.formatHsl(currentRgb.r, currentRgb.g, currentRgb.b)

    // Half width for split view panels
    readonly property real halfWidth: Math.floor((contentArea.width - splitRow.spacing) / 2)

    // Feedback key for copied checkmark
    property string copiedKey: ""
    property string historySearch: ""
    property string inputFeedback: ""

    // Filtered history
    readonly property var filteredColors: {
        const query = root.historySearch.trim().toLowerCase();
        if (query === "")
            return root.colors;
        return root.colors.filter(item => {
            const hex = (item.color ?? "").toLowerCase();
            const text = (item.text ?? "").toLowerCase();
            return hex.includes(query) || text.includes(query);
        });
    }

    implicitHeight: toolbar.height + Theme.spaceMedium + contentArea.implicitHeight

    Timer {
        id: forget

        interval: 1600
        onTriggered: {
            root.copiedKey = "";
            root.inputFeedback = "";
        }
    }

    function copyText(text: string, key: string): void {
        Daemon.command("clipboard", "copy-text", [text]);
        root.copiedKey = key;
        forget.restart();
    }

    function addToHistory(): void {
        Daemon.command("colors", "add", [root.currentHex]);
        root.copiedKey = "ADD";
        forget.restart();
    }

    function selectColor(hex: string): void {
        const rgb = ColorMath.hexToRgb(hex);
        if (rgb) {
            const hsv = ColorMath.rgbToHsv(rgb.r, rgb.g, rgb.b);
            root.hue = hsv.h;
            root.saturation = hsv.s;
            root.value = hsv.v;
        }
    }

    function applyCustomColor(input: string): bool {
        const rgb = ColorMath.parseColorString(input);
        if (rgb) {
            const hsv = ColorMath.rgbToHsv(rgb.r, rgb.g, rgb.b);
            root.hue = hsv.h;
            root.saturation = hsv.s;
            root.value = hsv.v;
            root.inputFeedback = "ok";
            forget.restart();
            return true;
        }
        root.inputFeedback = "error";
        forget.restart();
        return false;
    }

    // A box showing the current color in one format: a click copies it, a
    // double click puts it in the field to edit.
    component Format: Rectangle {
        id: format

        property string label
        property string value
        property bool editable: true
        // For the longer formats.
        property bool small: false

        width: Math.floor((parent.width - Theme.spaceSmall * 2) / 3)
        height: 52
        radius: Theme.radiusControl
        color: formatArea.containsMouse ? Theme.highlight : Theme.surface
        border.color: Theme.border
        border.width: 1

        Column {
            anchors.centerIn: parent
            spacing: 2

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: format.label
                color: Theme.muted
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
                font.weight: Theme.weightTitle
            }

            Row {
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: Theme.spaceTiny

                Text {
                    text: format.value
                    color: Theme.foreground
                    font.pixelSize: format.small ? Theme.textCaption : Theme.textBody
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightLabel
                }

                Symbol {
                    visible: root.copiedKey === format.label
                    anchors.verticalCenter: parent.verticalCenter
                    name: "check"
                    size: 11
                    color: Theme.success
                }
            }
        }

        MouseArea {
            id: formatArea

            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.copyText(format.value, format.label)
            onDoubleClicked: {
                if (!format.editable)
                    return;
                colorInputField.text = format.value;
                colorInputField.forceActiveFocus();
            }
        }
    }

    // A ring around a thumb, light inside dark, so it shows on any color.
    component Ring: Item {
        property real outer
        property real line
        property color fill: "transparent"

        width: outer
        height: outer

        Rectangle {
            anchors.fill: parent
            radius: width / 2
            color: "transparent"
            border.color: "#80000000" // design: a dark edge that shows on any color
            border.width: 1
        }

        Rectangle {
            anchors.centerIn: parent
            width: parent.width - 4
            height: width
            radius: width / 2
            color: parent.fill
            border.color: "#ffffff" // design: a light ring that shows on any color
            border.width: parent.line
        }
    }

    PanelHeader {
        id: toolbar

        width: parent.width
        title: root.colors.length === 1 ? "1 color saved" : `${root.colors.length} colors saved`

        Button {
            tone: "accent"
            icon: "colorize"
            text: "Pick a color"
            onClicked: Daemon.command("colors", "start", [])
        }

        Button {
            visible: root.colors.length > 0
            icon: "trash"
            text: "Clear"
            onClicked: Daemon.command("colors", "clear", [])
        }
    }

    // The chooser on the left, the history on the right.
    Item {
        id: contentArea

        anchors.top: toolbar.bottom
        anchors.topMargin: Theme.spaceMedium
        width: parent.width
        implicitHeight: 390

        Row {
            id: splitRow

            anchors.fill: parent
            spacing: Theme.spaceMedium

            Item {
                width: root.halfWidth
                height: parent.height

                Column {
                    width: parent.width
                    spacing: Theme.spaceSmall

                    // The color, and saturation and value to drag through.
                    Rectangle {
                        id: topBox

                        width: parent.width
                        height: 180
                        radius: Theme.radiusSurface
                        color: Theme.surface
                        clip: true

                        Rectangle {
                            id: swatch

                            anchors.left: parent.left
                            anchors.top: parent.top
                            anchors.bottom: parent.bottom
                            width: Math.round(parent.width * 0.36)
                            color: root.currentColor

                            // Copies the color on a click.
                            Rectangle {
                                anchors.centerIn: parent
                                width: Theme.controlHeight
                                height: Theme.controlHeight
                                radius: height / 2
                                color: Theme.surface
                                opacity: swatchHover.containsMouse ? 0.9 : 0.0

                                Behavior on opacity {
                                    NumberAnimation {
                                        duration: Theme.fast
                                    }
                                }

                                Symbol {
                                    anchors.centerIn: parent
                                    name: root.copiedKey === "SWATCH" ? "check" : "copy"
                                    size: 14
                                    color: root.copiedKey === "SWATCH" ? Theme.success : Theme.foreground
                                }
                            }

                            MouseArea {
                                id: swatchHover

                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: root.copyText(root.currentHex, "SWATCH")
                            }
                        }

                        Rectangle {
                            id: svPicker

                            anchors.left: swatch.right
                            anchors.right: parent.right
                            anchors.top: parent.top
                            anchors.bottom: parent.bottom
                            color: Qt.hsva(root.hue, 1.0, 1.0, 1.0)
                            clip: true

                            // Saturation, from white on the left.
                            Rectangle {
                                anchors.fill: parent
                                gradient: Gradient {
                                    orientation: Gradient.Horizontal
                                    GradientStop { position: 0.0; color: "#ffffff" } // design: saturation's scale
                                    GradientStop { position: 1.0; color: "#00ffffff" } // design: saturation's scale
                                }
                            }

                            // Value, to black at the bottom.
                            Rectangle {
                                anchors.fill: parent
                                gradient: Gradient {
                                    orientation: Gradient.Vertical
                                    GradientStop { position: 0.0; color: "#00000000" } // design: value's scale
                                    GradientStop { position: 1.0; color: "#000000" } // design: value's scale
                                }
                            }

                            Ring {
                                id: svThumb

                                readonly property real margin: 2

                                x: Math.round(margin + root.saturation * (parent.width - width - margin * 2))
                                y: Math.round(margin + (1.0 - root.value) * (parent.height - height - margin * 2))
                                outer: 20
                                line: 2.5
                            }

                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.CrossCursor

                                function updatePos(mouse) {
                                    const rangeX = width - svThumb.width - svThumb.margin * 2;
                                    const rangeY = height - svThumb.height - svThumb.margin * 2;
                                    if (rangeX > 0 && rangeY > 0) {
                                        root.saturation = Math.max(0.0, Math.min(1.0, (mouse.x - svThumb.margin - svThumb.width / 2) / rangeX));
                                        root.value = Math.max(0.0, Math.min(1.0, 1.0 - (mouse.y - svThumb.margin - svThumb.height / 2) / rangeY));
                                    }
                                }

                                onPressed: mouse => updatePos(mouse)
                                onPositionChanged: mouse => {
                                    if (pressed)
                                        updatePos(mouse);
                                }
                            }
                        }
                    }

                    // The hue.
                    Rectangle {
                        id: hueSlider

                        width: parent.width
                        height: 20
                        radius: height / 2

                        gradient: Gradient {
                            orientation: Gradient.Horizontal
                            GradientStop { position: 0.0; color: "#ff0000" } // design: the hues
                            GradientStop { position: 0.167; color: "#ffff00" } // design: the hues
                            GradientStop { position: 0.333; color: "#00ff00" } // design: the hues
                            GradientStop { position: 0.5; color: "#00ffff" } // design: the hues
                            GradientStop { position: 0.667; color: "#0000ff" } // design: the hues
                            GradientStop { position: 0.833; color: "#ff00ff" } // design: the hues
                            GradientStop { position: 1.0; color: "#ff0000" } // design: the hues
                        }

                        Ring {
                            id: hueThumb

                            x: Math.round(root.hue * (parent.width - width))
                            anchors.verticalCenter: parent.verticalCenter
                            outer: 24
                            line: 3
                            fill: Qt.hsva(root.hue, 1.0, 1.0, 1.0)
                        }

                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor

                            function updateHue(mouse) {
                                const range = parent.width - hueThumb.width;
                                if (range > 0)
                                    root.hue = Math.max(0.0, Math.min(1.0, (mouse.x - hueThumb.width / 2) / range));
                            }

                            onPressed: mouse => updateHue(mouse)
                            onPositionChanged: mouse => {
                                if (pressed)
                                    updateHue(mouse);
                            }
                        }
                    }

                    // Any hex, rgb, hsl or named color.
                    Rectangle {
                        id: colorInputBar

                        width: parent.width
                        height: Theme.controlHeight
                        radius: height / 2
                        color: Theme.surface
                        border.color: root.inputFeedback === "error" ? Theme.danger : (root.inputFeedback === "ok" ? Theme.success : Theme.border)
                        border.width: root.inputFeedback !== "" ? 1.5 : 1

                        Behavior on border.color {
                            ColorAnimation {
                                duration: Theme.fast
                            }
                        }

                        Symbol {
                            id: editIcon

                            x: Theme.spaceMedium
                            anchors.verticalCenter: parent.verticalCenter
                            name: "edit"
                            size: 13
                            color: root.inputFeedback === "error" ? Theme.danger : (root.inputFeedback === "ok" ? Theme.success : Theme.muted)
                        }

                        TextInput {
                            id: colorInputField

                            anchors.left: editIcon.right
                            anchors.leftMargin: Theme.spaceSmall
                            anchors.right: applyBtn.left
                            anchors.rightMargin: Theme.spaceSmall
                            anchors.verticalCenter: parent.verticalCenter
                            color: Theme.foreground
                            selectionColor: Theme.accent
                            font.pixelSize: Theme.textBody
                            font.family: Theme.fontFamily
                            clip: true
                            selectByMouse: true
                            onAccepted: {
                                if (text.trim() !== "")
                                    root.applyCustomColor(text);
                            }
                            onEditingFinished: {
                                if (text.trim() !== "")
                                    root.applyCustomColor(text);
                            }

                            Text {
                                visible: colorInputField.text === "" && !colorInputField.activeFocus
                                text: "Set a color, like #fb542b or coral"
                                color: Theme.muted
                                font.pixelSize: Theme.textCaption
                                font.family: Theme.fontFamily
                            }
                        }

                        Rectangle {
                            id: applyBtn

                            anchors.right: parent.right
                            anchors.rightMargin: Theme.spaceTiny
                            anchors.verticalCenter: parent.verticalCenter
                            width: 52
                            height: parent.height - Theme.spaceSmall
                            radius: height / 2
                            color: applyMouse.containsMouse ? Theme.highlight : Theme.raised

                            Row {
                                anchors.centerIn: parent
                                spacing: Theme.spaceTiny

                                Symbol {
                                    anchors.verticalCenter: parent.verticalCenter
                                    name: "check"
                                    size: 11
                                    color: root.inputFeedback === "ok" ? Theme.success : Theme.foreground
                                }

                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: "Set"
                                    color: Theme.foreground
                                    font.pixelSize: Theme.textCaption
                                    font.family: Theme.fontFamily
                                    font.weight: Theme.weightTitle
                                }
                            }

                            MouseArea {
                                id: applyMouse

                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: {
                                    if (colorInputField.text.trim() !== "")
                                        root.applyCustomColor(colorInputField.text);
                                }
                            }
                        }
                    }

                    Row {
                        width: parent.width
                        spacing: Theme.spaceSmall

                        Format {
                            label: "HEX"
                            value: root.hexText
                        }

                        Format {
                            label: "RGB"
                            value: root.rgbText
                        }

                        Format {
                            label: "CMYK"
                            value: root.cmykText
                            small: true
                            editable: false
                        }
                    }

                    Row {
                        width: parent.width
                        spacing: Theme.spaceSmall

                        Format {
                            label: "HSV"
                            value: root.hsvText
                            small: true
                        }

                        Format {
                            label: "HSL"
                            value: root.hslText
                            small: true
                        }

                        // Saves the color to the history.
                        Rectangle {
                            width: Math.floor((parent.width - Theme.spaceSmall * 2) / 3)
                            height: 52
                            radius: Theme.radiusControl
                            color: addArea.containsMouse ? Qt.lighter(Theme.accent, 1.1) : Theme.accent
                            scale: addArea.pressed ? 0.96 : 1.0

                            Behavior on scale {
                                NumberAnimation {
                                    duration: Theme.fast
                                }
                            }

                            Row {
                                anchors.centerIn: parent
                                spacing: Theme.spaceSmall

                                Symbol {
                                    anchors.verticalCenter: parent.verticalCenter
                                    name: root.copiedKey === "ADD" ? "check" : "plus"
                                    size: 13
                                    color: Theme.onAccent
                                }

                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: root.copiedKey === "ADD" ? "Saved" : "Save"
                                    color: Theme.onAccent
                                    font.pixelSize: Theme.textBody
                                    font.family: Theme.fontFamily
                                    font.weight: Theme.weightTitle
                                }
                            }

                            MouseArea {
                                id: addArea

                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: root.addToHistory()
                            }
                        }
                    }
                }
            }

            // The history.
            Item {
                width: root.halfWidth
                height: parent.height

                Column {
                    anchors.fill: parent
                    spacing: Theme.spaceSmall

                    Rectangle {
                        id: searchBox

                        width: parent.width
                        height: Theme.controlHeight
                        radius: height / 2
                        color: Theme.surface
                        border.color: Theme.border
                        border.width: 1

                        Symbol {
                            id: searchIcon

                            x: Theme.spaceMedium
                            anchors.verticalCenter: parent.verticalCenter
                            name: "search"
                            size: 13
                            color: Theme.muted
                        }

                        TextInput {
                            id: searchInput

                            anchors.left: searchIcon.right
                            anchors.leftMargin: Theme.spaceSmall
                            anchors.right: parent.right
                            anchors.rightMargin: Theme.spaceMedium
                            anchors.verticalCenter: parent.verticalCenter
                            color: Theme.foreground
                            selectionColor: Theme.accent
                            font.pixelSize: Theme.textBody
                            font.family: Theme.fontFamily
                            clip: true
                            onTextChanged: root.historySearch = text

                            Text {
                                visible: searchInput.text === ""
                                text: `Search ${root.colors.length} saved colors`
                                color: Theme.muted
                                font.pixelSize: Theme.textBody
                                font.family: Theme.fontFamily
                            }
                        }
                    }

                    Text {
                        visible: root.filteredColors.length === 0
                        width: parent.width
                        topPadding: Theme.spaceLarge
                        horizontalAlignment: Text.AlignHCenter
                        text: root.colors.length === 0 ? "No colors saved yet\nPick or save a color" : "No matches"
                        color: Theme.muted
                        font.pixelSize: Theme.textBody
                        font.family: Theme.fontFamily
                    }

                    ListView {
                        id: history

                        width: parent.width
                        height: parent.height - searchBox.height - parent.spacing
                        clip: true
                        spacing: Theme.spaceSmall
                        model: root.filteredColors

                        ScrollFade {
                            view: history
                        }

                        delegate: Rectangle {
                            id: historyRow

                            required property var modelData

                            width: ListView.view.width
                            height: 48
                            radius: Theme.radiusControl
                            color: rowMouse.containsMouse ? Theme.highlight : Theme.surface
                            border.color: Theme.border
                            border.width: 1

                            MouseArea {
                                id: rowMouse

                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: root.selectColor(historyRow.modelData.color)
                            }

                            Row {
                                anchors.left: parent.left
                                anchors.leftMargin: Theme.spaceSmall
                                anchors.right: actionsRow.left
                                anchors.rightMargin: Theme.spaceSmall
                                anchors.verticalCenter: parent.verticalCenter
                                spacing: Theme.spaceSmall

                                Rectangle {
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: Theme.controlHeight
                                    height: Theme.controlHeight
                                    radius: Theme.radiusControl
                                    color: historyRow.modelData.swatch
                                    border.color: Theme.border
                                    border.width: 1
                                }

                                Column {
                                    anchors.verticalCenter: parent.verticalCenter
                                    spacing: 1

                                    Text {
                                        text: historyRow.modelData.color
                                        color: Theme.foreground
                                        font.pixelSize: Theme.textBody
                                        font.family: Theme.fontFamily
                                        font.weight: Theme.weightTitle
                                    }

                                    Text {
                                        text: {
                                            const rgbFmt = historyRow.modelData.formats?.find(f => f.format === "rgb");
                                            return rgbFmt ? rgbFmt.text : historyRow.modelData.text;
                                        }
                                        color: Theme.muted
                                        font.pixelSize: Theme.textCaption
                                        font.family: Theme.fontFamily
                                    }
                                }
                            }

                            Row {
                                id: actionsRow

                                anchors.right: parent.right
                                anchors.rightMargin: Theme.spaceSmall
                                anchors.verticalCenter: parent.verticalCenter
                                spacing: Theme.spaceTiny

                                IconButton {
                                    icon: root.copiedKey === historyRow.modelData.color ? "check" : "copy"
                                    size: 13
                                    tone: "neutral"
                                    onClicked: root.copyText(historyRow.modelData.color, historyRow.modelData.color)
                                }

                                IconButton {
                                    icon: "trash"
                                    size: 13
                                    tone: "neutral"
                                    onClicked: Daemon.command("colors", "remove", [historyRow.modelData.color])
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
