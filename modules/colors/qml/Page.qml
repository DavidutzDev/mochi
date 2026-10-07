import QtQuick
import qs.island
import "ColorMath.js" as ColorMath

// The hub page: an interactive color chooser matching the reference design,
// side-by-side with color history. Supports picking from screen, choosing via
// 2D Sat/Val gradient and hue slider, direct hex entry, copying in multiple
// formats (HEX, RGB, CMYK, HSV, HSL), and managing saved colors.
Item {
    id: root

    property var payload: null
    readonly property var colors: payload?.history ?? []
    property string viewMode: "split" // "split" | "chooser" | "history"

    // Active color in HSV (normalized 0.0 .. 1.0)
    // Matches the reference screenshot (#fb542b) exactly
    property real hue: 0.03285
    property real saturation: 0.8287
    property real value: 0.9843

    // Active color RGB and Hex
    readonly property var currentRgb: ColorMath.hsvToRgb(hue, saturation, value)
    readonly property string currentHex: ColorMath.rgbToHex(currentRgb.r, currentRgb.g, currentRgb.b, false)
    readonly property color currentColor: Qt.rgba(currentRgb.r / 255.0, currentRgb.g / 255.0, currentRgb.b / 255.0, 1.0)

    // Formatted strings matching the reference UI
    readonly property string hexText: currentHex
    readonly property string rgbText: ColorMath.formatRgb(currentRgb.r, currentRgb.g, currentRgb.b)
    readonly property string cmykText: ColorMath.formatCmyk(currentRgb.r, currentRgb.g, currentRgb.b)
    readonly property string hsvText: ColorMath.formatHsv(hue, saturation, value)
    readonly property string hslText: ColorMath.formatHsl(currentRgb.r, currentRgb.g, currentRgb.b)

    // Feedback key for copied checkmark
    property string copiedKey: ""
    property string historySearch: ""

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

    implicitHeight: toolbar.height + 14 + contentArea.implicitHeight

    Timer {
        id: forget

        interval: 1600
        onTriggered: root.copiedKey = ""
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

    // Top navigation toolbar
    Item {
        id: toolbar

        width: parent.width
        height: 34

        // View mode switch
        Segmented {
            id: viewSwitcher

            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            height: 34
            implicitWidth: 320
            options: [
                { "value": "split", "label": "Split View", "icon": "grid" },
                { "value": "chooser", "label": "Chooser", "icon": "palette" },
                { "value": "history", "label": `History (${root.colors.length})`, "icon": "clock" }
            ]
            current: root.viewMode
            onPicked: value => root.viewMode = value
        }

        // Actions on right
        Row {
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: 8

            Button {
                tone: "accent"
                icon: "palette"
                text: "Pick from Screen"
                onClicked: Daemon.command("colors", "start", [])
            }

            Button {
                visible: root.colors.length > 0 && root.viewMode !== "chooser"
                icon: "trash"
                text: "Clear"
                onClicked: Daemon.command("colors", "clear", [])
            }
        }
    }

    // Main content area
    Item {
        id: contentArea

        anchors.top: toolbar.bottom
        anchors.topMargin: 14
        width: parent.width
        implicitHeight: {
            if (root.viewMode === "split")
                return 390;
            if (root.viewMode === "chooser")
                return chooserComponent.implicitHeight;
            return Math.max(200, historyFullComponent.implicitHeight);
        }

        // 1. SPLIT VIEW: Side-by-side Chooser & History
        Row {
            id: splitRow

            visible: root.viewMode === "split"
            anchors.fill: parent
            spacing: 14

            // Left panel: Color Chooser
            Item {
                width: 440
                height: parent.height

                Column {
                    width: parent.width
                    spacing: 12

                    // Top preview swatch + 2D SV gradient
                    Rectangle {
                        id: topBox

                        width: parent.width
                        height: 160
                        radius: Theme.radiusLarge
                        color: Theme.surface
                        clip: true

                        // Left: Solid preview swatch
                        Rectangle {
                            id: swatch

                            anchors.left: parent.left
                            anchors.top: parent.top
                            anchors.bottom: parent.bottom
                            width: Math.round(parent.width * 0.36)
                            color: root.currentColor

                            // Quick copy icon on hover
                            Rectangle {
                                anchors.centerIn: parent
                                width: 32
                                height: 32
                                radius: 16
                                color: Theme.surface
                                opacity: swatchHover.containsMouse ? 0.9 : 0.0

                                Behavior on opacity {
                                    NumberAnimation { duration: Theme.fast }
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

                        // Right: 2D Saturation / Value Picker
                        Rectangle {
                            id: svPicker

                            anchors.left: swatch.right
                            anchors.right: parent.right
                            anchors.top: parent.top
                            anchors.bottom: parent.bottom
                            color: Qt.hsva(root.hue, 1.0, 1.0, 1.0)
                            clip: true

                            // Horizontal white gradient (Saturation 0 -> 1)
                            Rectangle {
                                anchors.fill: parent
                                gradient: Gradient {
                                    orientation: Gradient.Horizontal
                                    GradientStop { position: 0.0; color: "#ffffff" }
                                    GradientStop { position: 1.0; color: "#00ffffff" }
                                }
                            }

                            // Vertical black gradient (Value 1 -> 0)
                            Rectangle {
                                anchors.fill: parent
                                gradient: Gradient {
                                    orientation: Gradient.Vertical
                                    GradientStop { position: 0.0; color: "#00000000" }
                                    GradientStop { position: 1.0; color: "#000000" }
                                }
                            }

                            // Thumb selector ring (clamped within visible area)
                            Item {
                                id: svThumb

                                readonly property real margin: 2
                                x: Math.round(margin + root.saturation * (parent.width - width - margin * 2))
                                y: Math.round(margin + (1.0 - root.value) * (parent.height - height - margin * 2))
                                width: 20
                                height: 20

                                Rectangle {
                                    anchors.centerIn: parent
                                    width: 20
                                    height: 20
                                    radius: 10
                                    color: "transparent"
                                    border.color: "#80000000"
                                    border.width: 1
                                }

                                Rectangle {
                                    anchors.centerIn: parent
                                    width: 16
                                    height: 16
                                    radius: 8
                                    color: "transparent"
                                    border.color: "#ffffff"
                                    border.width: 2.5
                                }
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

                    // Rainbow Hue Slider
                    Rectangle {
                        id: hueSlider

                        width: parent.width
                        height: 20
                        radius: height / 2
                        clip: false

                        gradient: Gradient {
                            orientation: Gradient.Horizontal
                            GradientStop { position: 0.0;   color: "#ff0000" }
                            GradientStop { position: 0.167; color: "#ffff00" }
                            GradientStop { position: 0.333; color: "#00ff00" }
                            GradientStop { position: 0.5;   color: "#00ffff" }
                            GradientStop { position: 0.667; color: "#0000ff" }
                            GradientStop { position: 0.833; color: "#ff00ff" }
                            GradientStop { position: 1.0;   color: "#ff0000" }
                        }

                        // Hue thumb
                        Item {
                            id: hueThumb

                            x: Math.round(root.hue * (parent.width - width))
                            anchors.verticalCenter: parent.verticalCenter
                            width: 24
                            height: 24

                            Rectangle {
                                anchors.centerIn: parent
                                width: 24
                                height: 24
                                radius: 12
                                color: "transparent"
                                border.color: "#80000000"
                                border.width: 1
                            }

                            Rectangle {
                                anchors.centerIn: parent
                                width: 20
                                height: 20
                                radius: 10
                                color: Qt.hsva(root.hue, 1.0, 1.0, 1.0)
                                border.color: "#ffffff"
                                border.width: 3
                            }
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

                    // Formats Grid: Row 1 (HEX, RGB, CMYK)
                    Row {
                        width: parent.width
                        spacing: 8

                        // HEX Box (editable or click to copy)
                        Rectangle {
                            width: Math.floor((parent.width - 16) / 3)
                            height: 52
                            radius: Theme.radiusSmall
                            color: hexArea.containsMouse ? Theme.highlight : Theme.surface
                            border.color: Theme.border
                            border.width: 1

                            Column {
                                anchors.centerIn: parent
                                spacing: 2

                                Text {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    text: "HEX"
                                    color: Theme.muted
                                    font.pixelSize: Theme.textCaption
                                    font.family: Theme.fontFamily
                                    font.weight: Font.DemiBold
                                }

                                Row {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    spacing: 4

                                    TextInput {
                                        id: hexInput
                                        text: root.hexText
                                        color: Theme.foreground
                                        font.pixelSize: Theme.textBody
                                        font.family: Theme.fontFamily
                                        font.weight: Font.Medium
                                        selectByMouse: true
                                        onEditingFinished: root.selectColor(text)
                                    }

                                    Symbol {
                                        visible: root.copiedKey === "HEX"
                                        anchors.verticalCenter: parent.verticalCenter
                                        name: "check"
                                        size: 11
                                        color: Theme.success
                                    }
                                }
                            }

                            IconButton {
                                anchors.right: parent.right
                                anchors.rightMargin: 4
                                anchors.verticalCenter: parent.verticalCenter
                                icon: root.copiedKey === "HEX" ? "check" : "copy"
                                size: 11
                                tone: "ghost"
                                onClicked: root.copyText(root.hexText, "HEX")
                            }

                            MouseArea {
                                id: hexArea
                                anchors.fill: parent
                                hoverEnabled: true
                                acceptedButtons: Qt.NoButton
                            }
                        }

                        // RGB Box
                        Rectangle {
                            width: Math.floor((parent.width - 16) / 3)
                            height: 52
                            radius: Theme.radiusSmall
                            color: rgbArea.containsMouse ? Theme.highlight : Theme.surface
                            border.color: Theme.border
                            border.width: 1

                            Column {
                                anchors.centerIn: parent
                                spacing: 2

                                Text {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    text: "RGB"
                                    color: Theme.muted
                                    font.pixelSize: Theme.textCaption
                                    font.family: Theme.fontFamily
                                    font.weight: Font.DemiBold
                                }

                                Row {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    spacing: 4

                                    Text {
                                        text: root.rgbText
                                        color: Theme.foreground
                                        font.pixelSize: Theme.textBody
                                        font.family: Theme.fontFamily
                                        font.weight: Font.Medium
                                    }

                                    Symbol {
                                        visible: root.copiedKey === "RGB"
                                        anchors.verticalCenter: parent.verticalCenter
                                        name: "check"
                                        size: 11
                                        color: Theme.success
                                    }
                                }
                            }

                            MouseArea {
                                id: rgbArea
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: root.copyText(root.rgbText, "RGB")
                            }
                        }

                        // CMYK Box
                        Rectangle {
                            width: Math.floor((parent.width - 16) / 3)
                            height: 52
                            radius: Theme.radiusSmall
                            color: cmykArea.containsMouse ? Theme.highlight : Theme.surface
                            border.color: Theme.border
                            border.width: 1

                            Column {
                                anchors.centerIn: parent
                                spacing: 2

                                Text {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    text: "CMYK"
                                    color: Theme.muted
                                    font.pixelSize: Theme.textCaption
                                    font.family: Theme.fontFamily
                                    font.weight: Font.DemiBold
                                }

                                Row {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    spacing: 4

                                    Text {
                                        text: root.cmykText
                                        color: Theme.foreground
                                        font.pixelSize: Theme.textCaption
                                        font.family: Theme.fontFamily
                                        font.weight: Font.Medium
                                    }

                                    Symbol {
                                        visible: root.copiedKey === "CMYK"
                                        anchors.verticalCenter: parent.verticalCenter
                                        name: "check"
                                        size: 11
                                        color: Theme.success
                                    }
                                }
                            }

                            MouseArea {
                                id: cmykArea
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: root.copyText(root.cmykText, "CMYK")
                            }
                        }
                    }

                    // Formats Grid: Row 2 (HSV, HSL, Add to History)
                    Row {
                        width: parent.width
                        spacing: 8

                        // HSV Box
                        Rectangle {
                            width: Math.floor((parent.width - 16) / 3)
                            height: 52
                            radius: Theme.radiusSmall
                            color: hsvArea.containsMouse ? Theme.highlight : Theme.surface
                            border.color: Theme.border
                            border.width: 1

                            Column {
                                anchors.centerIn: parent
                                spacing: 2

                                Text {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    text: "HSV"
                                    color: Theme.muted
                                    font.pixelSize: Theme.textCaption
                                    font.family: Theme.fontFamily
                                    font.weight: Font.DemiBold
                                }

                                Row {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    spacing: 4

                                    Text {
                                        text: root.hsvText
                                        color: Theme.foreground
                                        font.pixelSize: Theme.textCaption
                                        font.family: Theme.fontFamily
                                        font.weight: Font.Medium
                                    }

                                    Symbol {
                                        visible: root.copiedKey === "HSV"
                                        anchors.verticalCenter: parent.verticalCenter
                                        name: "check"
                                        size: 11
                                        color: Theme.success
                                    }
                                }
                            }

                            MouseArea {
                                id: hsvArea
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: root.copyText(root.hsvText, "HSV")
                            }
                        }

                        // HSL Box
                        Rectangle {
                            width: Math.floor((parent.width - 16) / 3)
                            height: 52
                            radius: Theme.radiusSmall
                            color: hslArea.containsMouse ? Theme.highlight : Theme.surface
                            border.color: Theme.border
                            border.width: 1

                            Column {
                                anchors.centerIn: parent
                                spacing: 2

                                Text {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    text: "HSL"
                                    color: Theme.muted
                                    font.pixelSize: Theme.textCaption
                                    font.family: Theme.fontFamily
                                    font.weight: Font.DemiBold
                                }

                                Row {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    spacing: 4

                                    Text {
                                        text: root.hslText
                                        color: Theme.foreground
                                        font.pixelSize: Theme.textCaption
                                        font.family: Theme.fontFamily
                                        font.weight: Font.Medium
                                    }

                                    Symbol {
                                        visible: root.copiedKey === "HSL"
                                        anchors.verticalCenter: parent.verticalCenter
                                        name: "check"
                                        size: 11
                                        color: Theme.success
                                    }
                                }
                            }

                            MouseArea {
                                id: hslArea
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: root.copyText(root.hslText, "HSL")
                            }
                        }

                        // Add to History Action Button
                        Rectangle {
                            width: Math.floor((parent.width - 16) / 3)
                            height: 52
                            radius: Theme.radiusSmall
                            color: addArea.containsMouse ? Qt.lighter(Theme.accent, 1.1) : Theme.accent
                            scale: addArea.pressed ? 0.96 : 1.0

                            Behavior on scale {
                                NumberAnimation { duration: Theme.fast }
                            }

                            Row {
                                anchors.centerIn: parent
                                spacing: 6

                                Symbol {
                                    anchors.verticalCenter: parent.verticalCenter
                                    name: root.copiedKey === "ADD" ? "check" : "plus"
                                    size: 13
                                    color: Theme.onAccent
                                }

                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: root.copiedKey === "ADD" ? "Saved!" : "Save Color"
                                    color: Theme.onAccent
                                    font.pixelSize: Theme.textBody
                                    font.family: Theme.fontFamily
                                    font.weight: Font.DemiBold
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

            // Right panel: Color History
            Item {
                width: parent.width - 440 - splitRow.spacing
                height: parent.height

                Column {
                    anchors.fill: parent
                    spacing: 8

                    // History search input
                    Rectangle {
                        width: parent.width
                        height: 32
                        radius: height / 2
                        color: Theme.surface
                        border.color: Theme.border
                        border.width: 1

                        Symbol {
                            id: searchIcon
                            x: 10
                            anchors.verticalCenter: parent.verticalCenter
                            name: "search"
                            size: 13
                            color: Theme.muted
                        }

                        TextInput {
                            id: searchInput
                            anchors.left: searchIcon.right
                            anchors.leftMargin: 8
                            anchors.right: parent.right
                            anchors.rightMargin: 10
                            anchors.verticalCenter: parent.verticalCenter
                            color: Theme.foreground
                            selectionColor: Theme.accent
                            font.pixelSize: Theme.textBody
                            font.family: Theme.fontFamily
                            clip: true
                            onTextChanged: root.historySearch = text

                            Text {
                                visible: searchInput.text === ""
                                text: `Search ${root.colors.length} saved colors...`
                                color: Theme.muted
                                font.pixelSize: Theme.textBody
                                font.family: Theme.fontFamily
                            }
                        }
                    }

                    // Empty state
                    Text {
                        visible: root.filteredColors.length === 0
                        width: parent.width
                        anchors.topMargin: 20
                        horizontalAlignment: Text.AlignHCenter
                        text: root.colors.length === 0 ? "No colors saved yet\nPick or save a color" : "No matches found"
                        color: Theme.muted
                        font.pixelSize: Theme.textBody
                        font.family: Theme.fontFamily
                    }

                    // Scrollable History ListView
                    ListView {
                        width: parent.width
                        height: parent.height - 40
                        clip: true
                        spacing: 6
                        model: root.filteredColors

                        delegate: Rectangle {
                            id: historyRow

                            required property var modelData

                            width: parent ? parent.width : 0
                            height: 48
                            radius: Theme.radiusSmall
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
                                anchors.leftMargin: 8
                                anchors.right: actionsRow.left
                                anchors.rightMargin: 8
                                anchors.verticalCenter: parent.verticalCenter
                                spacing: 10

                                // Preview swatch
                                Rectangle {
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: 32
                                    height: 32
                                    radius: Theme.radiusSmall - 2
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
                                        font.weight: Font.DemiBold
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
                                anchors.rightMargin: 8
                                anchors.verticalCenter: parent.verticalCenter
                                spacing: 4

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

        // 2. CHOOSER ONLY FULL VIEW
        Item {
            id: chooserComponent

            visible: root.viewMode === "chooser"
            width: parent.width
            implicitHeight: chooserCol.implicitHeight

            Column {
                id: chooserCol

                width: parent.width
                spacing: 16

                // Expansive Preview + 2D Gradient Box
                Rectangle {
                    width: parent.width
                    height: 220
                    radius: Theme.radiusLarge
                    color: Theme.surface
                    clip: true

                    // Swatch
                    Rectangle {
                        id: fullSwatch
                        anchors.left: parent.left
                        anchors.top: parent.top
                        anchors.bottom: parent.bottom
                        width: Math.round(parent.width * 0.38)
                        color: root.currentColor

                        Rectangle {
                            anchors.centerIn: parent
                            width: 40
                            height: 40
                            radius: 20
                            color: Theme.surface
                            opacity: fullSwatchHover.containsMouse ? 0.9 : 0.0

                            Behavior on opacity {
                                NumberAnimation { duration: Theme.fast }
                            }

                            Symbol {
                                anchors.centerIn: parent
                                name: root.copiedKey === "FULL_SWATCH" ? "check" : "copy"
                                size: 16
                                color: root.copiedKey === "FULL_SWATCH" ? Theme.success : Theme.foreground
                            }
                        }

                        MouseArea {
                            id: fullSwatchHover
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.copyText(root.currentHex, "FULL_SWATCH")
                        }
                    }

                    // 2D Gradient
                    Rectangle {
                        anchors.left: fullSwatch.right
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.bottom: parent.bottom
                        color: Qt.hsva(root.hue, 1.0, 1.0, 1.0)
                        clip: true

                        Rectangle {
                            anchors.fill: parent
                            gradient: Gradient {
                                orientation: Gradient.Horizontal
                                GradientStop { position: 0.0; color: "#ffffff" }
                                GradientStop { position: 1.0; color: "#00ffffff" }
                            }
                        }

                        Rectangle {
                            anchors.fill: parent
                            gradient: Gradient {
                                orientation: Gradient.Vertical
                                GradientStop { position: 0.0; color: "#00000000" }
                                GradientStop { position: 1.0; color: "#000000" }
                            }
                        }

                        Item {
                            id: fullSvThumb
                            readonly property real margin: 2
                            x: Math.round(margin + root.saturation * (parent.width - width - margin * 2))
                            y: Math.round(margin + (1.0 - root.value) * (parent.height - height - margin * 2))
                            width: 22
                            height: 22

                            Rectangle {
                                anchors.centerIn: parent
                                width: 22
                                height: 22
                                radius: 11
                                color: "transparent"
                                border.color: "#80000000"
                                border.width: 1
                            }

                            Rectangle {
                                anchors.centerIn: parent
                                width: 18
                                height: 18
                                radius: 9
                                color: "transparent"
                                border.color: "#ffffff"
                                border.width: 3
                            }
                        }

                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.CrossCursor

                            function updateFull(mouse) {
                                const rangeX = width - fullSvThumb.width - fullSvThumb.margin * 2;
                                const rangeY = height - fullSvThumb.height - fullSvThumb.margin * 2;
                                if (rangeX > 0 && rangeY > 0) {
                                    root.saturation = Math.max(0.0, Math.min(1.0, (mouse.x - fullSvThumb.margin - fullSvThumb.width / 2) / rangeX));
                                    root.value = Math.max(0.0, Math.min(1.0, 1.0 - (mouse.y - fullSvThumb.margin - fullSvThumb.height / 2) / rangeY));
                                }
                            }

                            onPressed: mouse => updateFull(mouse)
                            onPositionChanged: mouse => {
                                if (pressed)
                                    updateFull(mouse);
                            }
                        }
                    }
                }

                // Full Hue Slider
                Rectangle {
                    width: parent.width
                    height: 24
                    radius: height / 2
                    clip: false

                    gradient: Gradient {
                        orientation: Gradient.Horizontal
                        GradientStop { position: 0.0;   color: "#ff0000" }
                        GradientStop { position: 0.167; color: "#ffff00" }
                        GradientStop { position: 0.333; color: "#00ff00" }
                        GradientStop { position: 0.5;   color: "#00ffff" }
                        GradientStop { position: 0.667; color: "#0000ff" }
                        GradientStop { position: 0.833; color: "#ff00ff" }
                        GradientStop { position: 1.0;   color: "#ff0000" }
                    }

                    Item {
                        id: fullHueThumb
                        x: Math.round(root.hue * (parent.width - width))
                        anchors.verticalCenter: parent.verticalCenter
                        width: 26
                        height: 26

                        Rectangle {
                            anchors.centerIn: parent
                            width: 26
                            height: 26
                            radius: 13
                            color: "transparent"
                            border.color: "#80000000"
                            border.width: 1
                        }

                        Rectangle {
                            anchors.centerIn: parent
                            width: 22
                            height: 22
                            radius: 11
                            color: Qt.hsva(root.hue, 1.0, 1.0, 1.0)
                            border.color: "#ffffff"
                            border.width: 3
                        }
                    }

                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor

                        function updateHueFull(mouse) {
                            const range = parent.width - fullHueThumb.width;
                            if (range > 0)
                                root.hue = Math.max(0.0, Math.min(1.0, (mouse.x - fullHueThumb.width / 2) / range));
                        }

                        onPressed: mouse => updateHueFull(mouse)
                        onPositionChanged: mouse => {
                            if (pressed)
                                updateHueFull(mouse);
                        }
                    }
                }

                // Format row with 5 formats + Action button
                Row {
                    width: parent.width
                    spacing: 10

                    Repeater {
                        model: [
                            { "label": "HEX", "value": root.hexText },
                            { "label": "RGB", "value": root.rgbText },
                            { "label": "CMYK", "value": root.cmykText },
                            { "label": "HSV", "value": root.hsvText },
                            { "label": "HSL", "value": root.hslText }
                        ]

                        Rectangle {
                            required property var modelData

                            width: Math.floor((chooserCol.width - 50) / 6)
                            height: 56
                            radius: Theme.radiusSmall
                            color: fArea.containsMouse ? Theme.highlight : Theme.surface
                            border.color: Theme.border
                            border.width: 1

                            Column {
                                anchors.centerIn: parent
                                spacing: 3

                                Text {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    text: modelData.label
                                    color: Theme.muted
                                    font.pixelSize: Theme.textCaption
                                    font.family: Theme.fontFamily
                                    font.weight: Font.DemiBold
                                }

                                Row {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    spacing: 4

                                    Text {
                                        text: modelData.value
                                        color: Theme.foreground
                                        font.pixelSize: Theme.textBody
                                        font.family: Theme.fontFamily
                                        font.weight: Font.Medium
                                    }

                                    Symbol {
                                        visible: root.copiedKey === modelData.label
                                        anchors.verticalCenter: parent.verticalCenter
                                        name: "check"
                                        size: 11
                                        color: Theme.success
                                    }
                                }
                            }

                            MouseArea {
                                id: fArea
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: root.copyText(modelData.value, modelData.label)
                            }
                        }
                    }

                    // Save Color Button
                    Rectangle {
                        width: Math.floor((chooserCol.width - 50) / 6)
                        height: 56
                        radius: Theme.radiusSmall
                        color: fullAddArea.containsMouse ? Qt.lighter(Theme.accent, 1.1) : Theme.accent
                        scale: fullAddArea.pressed ? 0.96 : 1.0

                        Behavior on scale {
                            NumberAnimation { duration: Theme.fast }
                        }

                        Row {
                            anchors.centerIn: parent
                            spacing: 6

                            Symbol {
                                anchors.verticalCenter: parent.verticalCenter
                                name: root.copiedKey === "ADD" ? "check" : "plus"
                                size: 14
                                color: Theme.onAccent
                            }

                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                text: root.copiedKey === "ADD" ? "Saved!" : "Save Color"
                                color: Theme.onAccent
                                font.pixelSize: Theme.textBody
                                font.family: Theme.fontFamily
                                font.weight: Font.DemiBold
                            }
                        }

                        MouseArea {
                            id: fullAddArea
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.addToHistory()
                        }
                    }
                }
            }
        }

        // 3. HISTORY FULL VIEW
        Item {
            id: historyFullComponent

            visible: root.viewMode === "history"
            width: parent.width
            implicitHeight: historyCol.implicitHeight

            Column {
                id: historyCol

                width: parent.width
                spacing: 12

                // Search bar
                Rectangle {
                    width: parent.width
                    height: 36
                    radius: height / 2
                    color: Theme.surface
                    border.color: Theme.border
                    border.width: 1

                    Symbol {
                        id: fullSearchIcon
                        x: 12
                        anchors.verticalCenter: parent.verticalCenter
                        name: "search"
                        size: 14
                        color: Theme.muted
                    }

                    TextInput {
                        anchors.left: fullSearchIcon.right
                        anchors.leftMargin: 10
                        anchors.right: parent.right
                        anchors.rightMargin: 12
                        anchors.verticalCenter: parent.verticalCenter
                        color: Theme.foreground
                        selectionColor: Theme.accent
                        font.pixelSize: Theme.textBody
                        font.family: Theme.fontFamily
                        clip: true
                        text: root.historySearch
                        onTextChanged: root.historySearch = text

                        Text {
                            visible: parent.text === ""
                            text: `Search ${root.colors.length} saved colors...`
                            color: Theme.muted
                            font.pixelSize: Theme.textBody
                            font.family: Theme.fontFamily
                        }
                    }
                }

                // Empty message
                Text {
                    visible: root.filteredColors.length === 0
                    width: parent.width
                    anchors.topMargin: 30
                    horizontalAlignment: Text.AlignHCenter
                    text: root.colors.length === 0 ? "No colors saved yet" : "No matches found"
                    color: Theme.muted
                    font.pixelSize: Theme.textSubtitle
                    font.family: Theme.fontFamily
                }

                // Cards with all formats
                Repeater {
                    model: root.filteredColors

                    Rectangle {
                        id: fullHistoryRow

                        required property var modelData

                        width: historyCol.width
                        height: 56
                        radius: Theme.radiusMedium
                        color: Theme.surface
                        border.color: Theme.border
                        border.width: 1

                        // Swatch: clicking loads it into chooser
                        Rectangle {
                            id: fullSwatchItem

                            x: 12
                            anchors.verticalCenter: parent.verticalCenter
                            width: 36
                            height: 36
                            radius: Theme.radiusSmall
                            color: fullHistoryRow.modelData.swatch
                            border.color: Theme.border
                            border.width: 1

                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                onClicked: {
                                    root.selectColor(fullHistoryRow.modelData.color);
                                    root.viewMode = "split";
                                }
                            }
                        }

                        // Formats pills
                        Flow {
                            anchors.left: fullSwatchItem.right
                            anchors.leftMargin: 12
                            anchors.right: fullTrash.left
                            anchors.rightMargin: 10
                            anchors.verticalCenter: parent.verticalCenter
                            spacing: 8

                            Repeater {
                                model: fullHistoryRow.modelData.formats ?? []

                                Button {
                                    required property var modelData
                                    readonly property string key: `${fullHistoryRow.modelData.color}/${modelData.format}`

                                    text: modelData.text
                                    icon: root.copiedKey === key ? "check" : ""
                                    iconSize: 12
                                    onClicked: {
                                        Daemon.command("colors", "copy", [fullHistoryRow.modelData.color, modelData.format]);
                                        root.copiedKey = key;
                                        forget.restart();
                                    }
                                }
                            }
                        }

                        IconButton {
                            id: fullTrash

                            anchors.right: parent.right
                            anchors.rightMargin: 10
                            anchors.verticalCenter: parent.verticalCenter
                            icon: "trash"
                            size: 14
                            tone: "neutral"
                            onClicked: Daemon.command("colors", "remove", [fullHistoryRow.modelData.color])
                        }
                    }
                }
            }
        }
    }
}
