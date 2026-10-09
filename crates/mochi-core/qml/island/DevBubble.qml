import QtQuick

// The bubble of `mochid --dev`: the revision it runs, so a shell from the
// source tree is never mistaken for the installed one.
Item {
    id: root

    property var payload: ({})
    // What the pointer resting on it shows beside it.
    readonly property string tooltip: {
        const lines = [`Mochi dev, ${payload.revision ?? "unknown revision"}${payload.branch ? ` on ${payload.branch}` : ""}`, `${payload.build ?? "debug"} build, pid ${payload.pid}`];
        if (payload.took_over)
            lines.push(`Mochi ${payload.took_over.version} (pid ${payload.took_over.pid}) starts again when this one stops`);
        return lines.join("\n");
    }

    implicitWidth: row.implicitWidth + Theme.spaceSmall * 2
    implicitHeight: 26

    Row {
        id: row

        anchors.centerIn: parent
        spacing: Theme.spaceTiny

        Symbol {
            anchors.verticalCenter: parent.verticalCenter
            name: "code"
            size: 16
            color: Theme.accent
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.payload.short ?? "dev"
            color: Theme.foreground
            font.family: Theme.fontFamily
            font.pixelSize: Theme.textCaption
            font.weight: Font.DemiBold
        }
    }
}
