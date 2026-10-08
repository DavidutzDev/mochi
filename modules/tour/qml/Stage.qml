import QtQuick
import qs.island

// The island during the tour: the step's view, a module's real one with
// made-up data, or a card of the tour's own. Nothing in it takes clicks or
// keys: the dim layer has them. A view whose module isn't running, so the
// shell doesn't have it, shows as a card.
Item {
    id: root

    property var payload: ({})
    readonly property var step: payload.step ?? null
    readonly property string place: step?.place ?? "text"
    readonly property bool framed: place === "card" || place === "widget"
    readonly property bool hosted: step !== null && step.view !== "" && Daemon.modules.includes(step.module) && place !== "bubble"
    readonly property bool loaded: hosted && view.status === Loader.Ready

    implicitWidth: loaded ? (framed ? frame.width + Theme.padding * 2 : view.item.implicitWidth) : card.implicitWidth
    implicitHeight: loaded ? (framed ? frame.height + Theme.padding * 2 : view.item.implicitHeight) : card.implicitHeight

    // A card's or a widget's frame, as the hub and the desktop draw them.
    Rectangle {
        id: frame

        visible: root.loaded && root.framed
        anchors.centerIn: parent
        width: root.step?.size?.[0] ?? 280
        // As tall as the view needs, when that's more.
        height: Math.max(root.step?.size?.[1] ?? Theme.tileHeight, (view.item?.implicitHeight ?? 0) + Theme.spaceMedium * 2)
        radius: Theme.radiusSurface
        color: Theme.surface
    }

    // The module's view, built afresh for each step.
    Loader {
        id: view

        anchors.fill: root.framed ? frame : parent
        anchors.margins: root.framed ? Theme.spaceMedium : 0
        enabled: false
        active: root.hosted

        function build(): void {
            if (!root.hosted)
                return;
            const url = `root:/modules/${root.step.module}/${root.step.view}.qml`;
            setSource(url, Object.assign({}, root.step.properties ?? {}, {
                "payload": root.step.payload ?? {}
            }));
            if (status === Loader.Error)
                console.warn(`mochi: the tour could not load ${url}`);
        }

        Component.onCompleted: build()

        Connections {
            target: root

            function onStepChanged(): void {
                view.build();
            }
        }
    }

    // A card of the tour's own: a welcome, a look, a module that's off.
    Item {
        id: card

        visible: !root.loaded
        anchors.fill: parent
        implicitWidth: Math.max(360, Math.min(520, texts.implicitWidth + 44 + Theme.padding * 3))
        implicitHeight: texts.implicitHeight + Theme.padding * 2

        Rectangle {
            id: badge

            x: Theme.padding
            anchors.verticalCenter: parent.verticalCenter
            width: 44
            height: 44
            radius: width / 2
            color: root.place === "off" ? Theme.raised : Theme.accent

            Symbol {
                anchors.centerIn: parent
                name: root.step?.icon || "auto_awesome"
                size: Theme.textHeadline
                color: root.place === "off" ? Theme.muted : Theme.onAccent
            }
        }

        Column {
            id: texts

            anchors.left: badge.right
            anchors.leftMargin: Theme.padding
            anchors.right: parent.right
            anchors.rightMargin: Theme.padding
            anchors.verticalCenter: parent.verticalCenter
            spacing: 2

            Text {
                text: root.step?.title ?? ""
                color: Theme.foreground
                font.pixelSize: Theme.textHeadline
                font.family: Theme.fontFamily
                font.weight: Theme.weightTitle
            }

            Text {
                visible: root.place === "off"
                text: "Off"
                color: Theme.muted
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
                font.weight: Theme.weightLabel
            }
        }
    }
}
