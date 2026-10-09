import QtQuick
import qs.island

// The About page: which Mochi runs, the system and session it runs in, and
// how the daemon and the processes it started are doing. It's read when the
// page shows; Copy puts it on the clipboard as text for a bug report, and
// the links open Mochi's pages in the browser.
Column {
    id: root

    readonly property var settings: Daemon.state("settings") ?? ({})
    readonly property var about: settings.about ?? null
    readonly property bool reading: settings.about_reading === true
    readonly property var mochi: about?.mochi ?? ({})
    readonly property var system: about?.system ?? ({})
    readonly property var session: about?.session ?? ({})
    readonly property var status: about?.status ?? null

    spacing: Theme.spaceLarge

    // Read when the page shows, not when the panel opens.
    onVisibleChanged: {
        if (visible)
            Daemon.command("settings", "about", []);
    }
    Component.onCompleted: {
        if (visible)
            Daemon.command("settings", "about", []);
    }

    function size(bytes: var): string {
        if (typeof bytes !== "number")
            return "";
        const mib = bytes / (1024 * 1024);
        return mib >= 1024 ? `${(mib / 1024).toFixed(1)} GiB` : `${Math.round(mib)} MiB`;
    }

    function duration(seconds: var): string {
        if (typeof seconds !== "number")
            return "";
        const hours = Math.floor(seconds / 3600);
        const minutes = Math.floor(seconds % 3600 / 60);
        if (hours >= 24)
            return `${Math.floor(hours / 24)} d ${hours % 24} h`;
        if (hours > 0)
            return `${hours} h ${minutes} min`;
        return minutes > 0 ? `${minutes} min` : `${seconds} s`;
    }

    // The rows of each group: a label, a value, and "danger" for one that
    // says something is wrong. Empty values are left out.
    readonly property var groups: {
        if (about === null)
            return [];
        // Any release of the series Mochi pins works, like 0.3.x.
        const wanted = (mochi.quickshell_wanted ?? "").split(".").slice(0, 2).join(".");
        const quickshell = mochi.quickshell ?? "Not found";
        const running = (status?.modules ?? []).length;
        const plugins = status?.plugins ?? [];
        const outputs = status?.compositor?.outputs ?? [];
        const out = [
            {
                "title": "Mochi",
                "rows": [["Version", mochi.version ?? ""], ["Build", mochi.revision ? `${mochi.build}, ${mochi.revision}` : mochi.build ?? ""], ["Quickshell", quickshell, quickshell.includes(wanted) ? "" : "danger"], ["Protocol", mochi.api !== undefined ? `API ${mochi.api}` : ""], ["Config", mochi.config ?? ""], ["Socket", mochi.socket ?? ""]]
            },
            {
                "title": "System",
                "rows": [["Distribution", system.os ?? ""], ["Kernel", system.kernel ? `Linux ${system.kernel}` : ""], ["Architecture", system.arch ?? ""], ["Processor", system.cpu ? `${system.cpu}${system.cores ? `, ${system.cores} threads` : ""}` : ""], ["Memory", size(system.memory)]]
            },
            {
                "title": "Session",
                "rows": [["Desktop", session.desktop ?? ""], ["Session", session.type ?? ""], ["Compositor", status?.compositor?.backend ?? ""], ["Screens", outputs.join(", ")], ["Workspaces", status?.compositor?.workspaces !== undefined ? String(status.compositor.workspaces) : ""], ["Interface", status === null ? "" : status.ui_connected ? "Connected" : "Not connected", status?.ui_connected === false ? "danger" : ""], ["Daemon", about.status_error ? `No status: ${about.status_error}` : "", "danger"]]
            },
            {
                "title": "Running",
                "rows": [["mochid", `pid ${mochi.pid}, up ${duration(mochi.uptime)}, ${size(mochi.memory)}`]].concat((about.processes ?? []).map(process => [process.name, `pid ${process.pid}, ${size(process.memory)}`])).concat([["Modules", running > 0 ? `${running}: ${status.modules.join(", ")}` : ""]]).concat(plugins.map(plugin => [plugin.id, plugin.message ? `${plugin.state}: ${plugin.message}` : plugin.state, plugin.state === "failed" || plugin.state === "missing" ? "danger" : ""]))
            }
        ];
        return out.map(group => ({
                    "title": group.title,
                    "rows": group.rows.filter(row => row[1] !== "")
                }));
    }

    // A button the keyboard reaches with Tab and presses with Enter or Space.
    component Action: Item {
        id: action

        property alias text: button.text
        property alias icon: button.icon
        property alias tone: button.tone
        signal activated

        implicitWidth: button.implicitWidth
        implicitHeight: button.implicitHeight
        activeFocusOnTab: enabled
        Keys.onReturnPressed: action.activated()
        Keys.onEnterPressed: action.activated()
        Keys.onSpacePressed: action.activated()

        Button {
            id: button

            anchors.fill: parent
            onClicked: action.activated()
        }

        // The focus ring, around the pill.
        Rectangle {
            visible: action.activeFocus
            anchors.fill: parent
            anchors.margins: -3
            radius: height / 2
            color: "transparent"
            border.width: 2
            border.color: Theme.accent
        }
    }

    // Which Mochi this is, and what to do with it.
    Item {
        width: root.width
        height: Math.max(heading.implicitHeight, actions.implicitHeight)

        Column {
            id: heading

            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - actions.width - Theme.spaceMedium
            spacing: Theme.spaceTiny

            Text {
                width: parent.width
                elide: Text.ElideRight
                text: root.about === null ? "Mochi" : `Mochi ${root.mochi.version ?? ""}`
                color: Theme.foreground
                font.pixelSize: Theme.textTitle
                font.family: Theme.fontFamily
                font.weight: Theme.weightTitle
            }

            // The Build row has the revision; this says where it runs from.
            Text {
                width: parent.width
                elide: Text.ElideRight
                text: root.mochi.build === "debug" ? "A debug build, from the source tree" : root.mochi.build === "release" ? "A release build" : ""
                color: Theme.muted
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }
        }

        Row {
            id: actions

            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.spaceSmall

            Action {
                text: "GitHub"
                icon: "code"
                tone: "ghost"
                onActivated: Daemon.command("settings", "open-link", ["repository"])
            }

            Action {
                text: "Documentation"
                icon: "menu_book"
                tone: "ghost"
                onActivated: Daemon.command("settings", "open-link", ["documentation"])
            }

            Action {
                text: "Report a bug"
                icon: "bug_report"
                tone: "ghost"
                onActivated: Daemon.command("settings", "open-link", ["issue"])
            }

            Action {
                id: copy

                property bool copied: false

                enabled: root.about !== null
                text: copied ? "Copied" : "Copy details"
                icon: copied ? "check" : "content_copy"
                tone: "accent"
                onActivated: {
                    Daemon.command("settings", "about-copy", []);
                    copied = true;
                    copiedTimer.restart();
                }

                Timer {
                    id: copiedTimer

                    interval: 1600
                    onTriggered: copy.copied = false
                }
            }
        }
    }

    // While it's read the first time, or when it couldn't be.
    Row {
        visible: root.about === null
        x: Theme.spaceMedium
        spacing: Theme.spaceSmall

        Symbol {
            anchors.verticalCenter: parent.verticalCenter
            name: root.reading ? "progress_activity" : "error"
            size: Theme.textTitle
            color: root.reading ? Theme.muted : Theme.danger

            RotationAnimation on rotation {
                running: root.reading
                loops: Animation.Infinite
                from: 0
                to: 360
                duration: Theme.duration(1000)
            }
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.reading ? "Reading what runs…" : "Nothing read yet."
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        Action {
            visible: !root.reading
            anchors.verticalCenter: parent.verticalCenter
            text: "Read again"
            icon: "refresh"
            onActivated: Daemon.command("settings", "about", [])
        }
    }

    Repeater {
        model: root.groups

        Column {
            id: group

            required property var modelData

            width: root.width
            spacing: Theme.spaceSmall

            Item {
                width: parent.width
                height: label.implicitHeight

                SectionLabel {
                    id: label

                    x: Theme.spaceMedium
                    text: group.modelData.title
                }

                // The figures change: read them again.
                Text {
                    visible: group.modelData.title === "Running"
                    anchors.right: parent.right
                    anchors.rightMargin: Theme.spaceMedium
                    text: root.reading ? "Reading…" : "Read again"
                    color: refreshArea.containsMouse || refreshArea.activeFocus ? Theme.foreground : Theme.muted
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                    font.underline: refreshArea.activeFocus

                    MouseArea {
                        id: refreshArea

                        anchors.fill: parent
                        anchors.margins: -Theme.spaceTiny
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        activeFocusOnTab: true
                        onClicked: Daemon.command("settings", "about", [])
                        Keys.onReturnPressed: Daemon.command("settings", "about", [])
                        Keys.onSpacePressed: Daemon.command("settings", "about", [])
                    }
                }
            }

            Rectangle {
                width: parent.width
                height: rows.implicitHeight + Theme.spaceSmall * 2
                radius: Theme.radiusField
                color: Theme.surface

                Column {
                    id: rows

                    x: Theme.spaceMedium
                    y: Theme.spaceSmall
                    width: parent.width - Theme.spaceMedium * 2

                    Repeater {
                        model: group.modelData.rows

                        Row {
                            id: row

                            required property var modelData

                            width: rows.width
                            height: Math.max(Theme.controlHeight, value.implicitHeight + Theme.spaceSmall)
                            spacing: Theme.spaceMedium

                            Text {
                                width: 140
                                anchors.verticalCenter: parent.verticalCenter
                                elide: Text.ElideRight
                                text: row.modelData[0]
                                color: Theme.muted
                                font.pixelSize: Theme.textBody
                                font.family: Theme.fontFamily
                            }

                            // Selectable, to copy one value.
                            TextEdit {
                                id: value

                                width: row.width - 140 - Theme.spaceMedium
                                anchors.verticalCenter: parent.verticalCenter
                                readOnly: true
                                selectByMouse: true
                                wrapMode: TextEdit.Wrap
                                text: row.modelData[1]
                                color: row.modelData[2] === "danger" ? Theme.danger : Theme.foreground
                                selectionColor: Theme.accent
                                selectedTextColor: Theme.onAccent
                                font.pixelSize: Theme.textBody
                                font.family: Theme.fontFamily
                            }
                        }
                    }
                }
            }
        }
    }
}
