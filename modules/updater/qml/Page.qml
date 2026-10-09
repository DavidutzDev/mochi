import QtQuick
import qs.island

// The Updates page, at the top of the module's page in the settings: whether
// this Mochi is the latest, how it updates, which depends on how it was
// installed, and the changelog of each newer release.
Column {
    id: root

    property var payload: ({})
    readonly property var newer: payload.newer ?? []
    readonly property bool outdated: newer.length > 0
    readonly property bool checking: payload.checking === true
    readonly property var job: payload.job ?? ({})
    readonly property string kind: payload.method?.kind ?? "unknown"
    // The install script updates in place; anything else runs a command.
    readonly property bool inPlace: kind === "script" && payload.custom !== true

    spacing: Theme.spaceLarge

    function ago(seconds: var): string {
        if (typeof seconds !== "number")
            return "";
        const minutes = Math.floor((Date.now() / 1000 - seconds) / 60);
        if (minutes < 1)
            return "just now";
        if (minutes < 60)
            return `${minutes} min ago`;
        const hours = Math.floor(minutes / 60);
        return hours < 24 ? `${hours} h ago` : `${Math.floor(hours / 24)} d ago`;
    }

    // Whether there's a new release.
    Rectangle {
        width: root.width
        height: status.implicitHeight + Theme.spaceMedium * 2
        radius: Theme.radiusField
        color: Theme.surface

        Row {
            id: status

            x: Theme.spaceMedium
            y: Theme.spaceMedium
            width: parent.width - Theme.spaceMedium * 2
            spacing: Theme.spaceMedium

            Symbol {
                id: mark

                anchors.verticalCenter: parent.verticalCenter
                name: root.checking ? "progress_activity" : root.outdated ? "system_update" : root.payload.error ? "cloud_off" : "check_circle"
                size: Theme.textHeadline
                color: root.outdated ? Theme.accent : root.payload.error && !root.checking ? Theme.danger : Theme.muted

                RotationAnimation on rotation {
                    running: root.checking
                    loops: Animation.Infinite
                    from: 0
                    to: 360
                    duration: Theme.duration(1000)
                }
            }

            Column {
                width: parent.width - mark.width - checkNow.width - Theme.spaceMedium * 2
                anchors.verticalCenter: parent.verticalCenter
                spacing: 2

                Text {
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: {
                        if (root.outdated)
                            return `Mochi ${root.payload.latest} is out`;
                        if (root.checking && root.payload.checked_at === null)
                            return "Looking for a new release…";
                        if (root.payload.error)
                            return "Couldn't look for a new release";
                        if (root.payload.checked_at === null || root.payload.checked_at === undefined)
                            return `Mochi ${root.payload.current ?? ""}`;
                        return `Mochi ${root.payload.current} is the latest`;
                    }
                    color: Theme.foreground
                    font.pixelSize: Theme.textTitle
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightTitle
                }

                Text {
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: {
                        const parts = [];
                        if (root.outdated)
                            parts.push(`You have ${root.payload.current}${root.newer.length > 1 ? `, ${root.newer.length} releases behind` : ""}.`);
                        if (root.payload.error && !root.checking)
                            parts.push(`${root.payload.error}.`);
                        if (root.payload.checked_at)
                            parts.push(`Checked ${root.ago(root.payload.checked_at)}.`);
                        else if (root.payload.check === false)
                            parts.push("Checking at start is off.");
                        return parts.join(" ");
                    }
                    visible: text !== ""
                    color: root.payload.error && !root.checking && !root.outdated ? Theme.danger : Theme.muted
                    font.pixelSize: Theme.textBody
                    font.family: Theme.fontFamily
                }
            }

            ActionButton {
                id: checkNow

                anchors.verticalCenter: parent.verticalCenter
                enabled: !root.checking
                text: root.checking ? "Checking…" : "Check now"
                icon: "refresh"
                tone: "ghost"
                onClicked: Daemon.command("updater", "check", [])
            }
        }
    }

    // How this Mochi updates.
    Column {
        width: root.width
        spacing: Theme.spaceSmall

        SectionLabel {
            x: Theme.spaceMedium
            text: "How to update"
        }

        Rectangle {
            width: parent.width
            height: how.implicitHeight + Theme.spaceMedium * 2
            radius: Theme.radiusField
            color: Theme.surface

            Column {
                id: how

                x: Theme.spaceMedium
                y: Theme.spaceMedium
                width: parent.width - Theme.spaceMedium * 2
                spacing: Theme.spaceMedium

                Text {
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: root.payload.custom === true ? "With your own command, from the settings below." : root.payload.about ?? ""
                    color: Theme.foreground
                    font.pixelSize: Theme.textBody
                    font.family: Theme.fontFamily
                }

                // The command, to select or copy.
                Rectangle {
                    visible: !root.inPlace
                    width: parent.width
                    height: commandText.implicitHeight + Theme.spaceSmall * 2
                    radius: Theme.radiusField
                    color: Theme.raised

                    TextEdit {
                        id: commandText

                        x: Theme.spaceSmall
                        y: Theme.spaceSmall
                        width: parent.width - Theme.spaceSmall * 2
                        readOnly: true
                        selectByMouse: true
                        wrapMode: TextEdit.WrapAnywhere
                        text: root.payload.command ?? ""
                        color: Theme.foreground
                        selectionColor: Theme.accent
                        selectedTextColor: Theme.onAccent
                        font.pixelSize: Theme.textBody
                        font.family: "monospace"
                    }
                }

                // What the installer said, while it runs or when it failed.
                Text {
                    visible: root.inPlace && (root.job.running === true || (root.job.error ?? null) !== null)
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: root.job.running ? "Updating… Mochi restarts once it's done." : `The update failed: ${root.job.error ?? ""}`
                    color: root.job.running ? Theme.muted : Theme.danger
                    font.pixelSize: Theme.textBody
                    font.family: Theme.fontFamily
                }

                Row {
                    anchors.right: parent.right
                    spacing: Theme.spaceSmall

                    ActionButton {
                        visible: !root.inPlace
                        text: "Copy"
                        icon: "content_copy"
                        tone: "ghost"
                        onClicked: Daemon.command("updater", "copy", [])
                    }

                    ActionButton {
                        visible: !root.inPlace
                        enabled: root.payload.terminal === true
                        text: "Run in a terminal"
                        icon: "terminal"
                        tone: root.outdated ? "accent" : "neutral"
                        onClicked: Daemon.command("updater", "run", [])
                    }

                    ActionButton {
                        visible: root.inPlace
                        enabled: root.outdated && root.job.running !== true
                        text: root.outdated ? `Update to ${root.payload.latest}` : "Up to date"
                        icon: "system_update"
                        tone: "accent"
                        onClicked: Daemon.command("updater", "update", [])
                    }
                }

                Text {
                    visible: !root.inPlace && root.payload.terminal !== true
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: "No terminal found to run it: set one below, or install xdg-terminal-exec."
                    color: Theme.muted
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                }
            }
        }
    }

    // What each newer release brings.
    Column {
        visible: root.outdated
        width: root.width
        spacing: Theme.spaceSmall

        SectionLabel {
            x: Theme.spaceMedium
            text: "What's new"
        }

        Repeater {
            model: root.newer

            Rectangle {
                id: release

                required property var modelData

                width: root.width
                height: notes.y + notes.implicitHeight + Theme.spaceMedium
                radius: Theme.radiusField
                color: Theme.surface

                Row {
                    id: title

                    x: Theme.spaceMedium
                    y: Theme.spaceMedium
                    spacing: Theme.spaceSmall

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: `Mochi ${release.modelData.version}`
                        color: Theme.foreground
                        font.pixelSize: Theme.textTitle
                        font.family: Theme.fontFamily
                        font.weight: Theme.weightTitle
                    }

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        visible: (release.modelData.date ?? "") !== ""
                        text: release.modelData.date ?? ""
                        color: Theme.muted
                        font.pixelSize: Theme.textCaption
                        font.family: Theme.fontFamily
                    }
                }

                ActionButton {
                    visible: (release.modelData.url ?? "") !== ""
                    anchors.right: parent.right
                    anchors.rightMargin: Theme.spaceSmall
                    anchors.verticalCenter: title.verticalCenter
                    text: "On GitHub"
                    icon: "open_in_new"
                    tone: "ghost"
                    onClicked: Daemon.command("updater", "open-release", [release.modelData.version])
                }

                Text {
                    id: notes

                    x: Theme.spaceMedium
                    y: title.y + title.height + Theme.spaceSmall
                    width: parent.width - Theme.spaceMedium * 2
                    wrapMode: Text.Wrap
                    textFormat: Text.MarkdownText
                    text: (release.modelData.notes ?? "") !== "" ? release.modelData.notes : "No notes for this release."
                    color: Theme.foreground
                    linkColor: Theme.accent
                    font.pixelSize: Theme.textBody
                    font.family: Theme.fontFamily
                }
            }
        }
    }
}
