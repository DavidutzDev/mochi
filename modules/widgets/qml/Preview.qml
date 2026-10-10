import QtQuick
import qs.island

// A widget as it looks on the desktop, for the drawer: its module's real
// view with the module's real state and the default settings, at the size
// it's added at, scaled down to fit, and taking no clicks. `entry` is a
// widget of the catalog and `variant` one of its looks, or null.
Item {
    id: root

    required property var entry
    property var variant: null
    property real cell: 16

    readonly property var size: variant?.size ?? entry.size
    // Whether it has a card: its look's, which can go without.
    readonly property bool framed: variant?.frame ?? entry.frame
    readonly property real fullWidth: size[0] * cell
    readonly property real fullHeight: size[1] * cell
    readonly property real ratio: Math.min(1, width / fullWidth, height / fullHeight)
    // A view with nothing to show steps aside, like the battery's on a
    // computer without one.
    readonly property bool hidden: content.item?.hidden ?? false
    // The look's own defaults first.
    readonly property var defaults: {
        const settings = {};
        for (const setting of entry.settings ?? [])
            settings[setting.name] = variant?.defaults?.[setting.name] ?? setting.default;
        return settings;
    }

    Item {
        id: widget

        anchors.centerIn: parent
        width: root.fullWidth
        height: root.fullHeight
        scale: root.ratio
        opacity: root.hidden ? 0.5 : 1

        Rectangle {
            anchors.fill: parent
            visible: root.framed
            radius: Theme.radiusSurface
            color: Theme.background
            border.width: 1
            border.color: Theme.border
        }

        Loader {
            id: content

            anchors.fill: parent
            anchors.margins: root.framed ? Theme.padding : 0
            enabled: false
            Component.onCompleted: setSource(`root:/modules/${root.entry.module}/${root.variant?.view ?? root.entry.view}.qml`, {
                payload: Daemon.state(root.entry.module)
            })
        }

        // The defaults and the id, to views that declare them.
        Binding {
            target: content.item
            property: "settings"
            value: root.defaults
            when: content.item !== null && "settings" in content.item
        }

        Binding {
            target: content.item
            property: "instance"
            value: "preview"
            when: content.item !== null && "instance" in content.item
        }

        Binding {
            target: content.item
            property: "payload"
            value: Daemon.state(root.entry.module)
            when: content.item !== null
        }

        // For a view that has looks: which one.
        Binding {
            target: content.item
            property: "variant"
            value: root.variant?.id ?? ""
            when: content.item !== null && "variant" in content.item
        }
    }

    Text {
        anchors.centerIn: parent
        width: parent.width
        visible: root.hidden
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.Wrap
        text: "Nothing to show right now"
        color: Theme.foreground
        font.pixelSize: Theme.textCaption
        font.family: Theme.fontFamily
        font.weight: Theme.weightLabel
    }
}
