import QtQuick
import Quickshell
import Quickshell.Wayland

// The desktop layer of one monitor: a surface over the wallpaper and under
// every window, for the widgets module's Desktop.qml. Only the widgets
// take clicks; everywhere else they reach whatever is under the layer.
// While arranging, it comes over the windows, takes every click and the
// keyboard, and the view dims the screen.
PanelWindow {
    id: root

    readonly property Item desktop: view.item
    readonly property bool editing: desktop?.editing ?? false

    anchors {
        top: true
        bottom: true
        left: true
        right: true
    }
    exclusionMode: ExclusionMode.Ignore
    color: "transparent"
    visible: Daemon.modules.includes("widgets")

    WlrLayershell.namespace: "mochi-widgets"
    WlrLayershell.layer: editing ? WlrLayer.Overlay : WlrLayer.Bottom
    // A widget with a text field asks for the keyboard while it's focused.
    WlrLayershell.keyboardFocus: editing ? WlrKeyboardFocus.Exclusive : desktop?.wantsKeyboard ? WlrKeyboardFocus.OnDemand : WlrKeyboardFocus.None

    mask: editing ? everywhere : shapes

    property Region everywhere: Region {
        width: root.width
        height: root.height
    }

    property Region shapes: Region {
        regions: masks.instances
    }

    Variants {
        id: masks

        model: root.desktop?.shapes ?? []

        Region {
            required property Item modelData

            item: modelData
        }
    }

    // Framed widgets blur what's behind them.
    BackgroundEffect.blurRegion: Region {
        regions: blurs.instances
    }

    Variants {
        id: blurs

        model: root.desktop?.framed ?? []

        Region {
            required property Item modelData

            item: modelData
            radius: Theme.radiusSurface
        }
    }

    Loader {
        id: view

        anchors.fill: parent
        source: root.visible ? "root:/modules/widgets/Desktop.qml" : ""
    }

    Binding {
        target: view.item
        property: "output"
        value: root.screen?.name ?? ""
        when: view.item !== null
    }
}
