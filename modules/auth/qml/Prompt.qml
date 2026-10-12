import QtQuick
import qs.island

// Asks for a password the session needs, for one of two:
//
// - polkit, when an app wants an admin action, like pkexec, systemctl or
//   a settings app. It says which app asks and why, and for pkexec the
//   command. When several users may allow it, a choice picks who types.
//   polkit's own helper checks the password; a wrong one clears the field
//   and says so.
// - gnome-keyring, to unlock a keyring, to set a new one's password, typed
//   twice, or for an SSH key's passphrase gcr's agent needs. It shows what
//   the keyring says, with its choice to tick, like unlocking at login; a
//   question without a password is a yes or no.
// - sudo -A or ssh, through `mochi askpass`: what they ask, in their words.
//   For an SSH key's passphrase, a choice of how long the key stays in the
//   agent. ssh's yes or no questions are such, and its notices, like
//   touching a security key, only show.
//
// Enter answers, Escape or Cancel declines.
Item {
    id: root

    property var payload: ({})
    readonly property string kind: payload.kind ?? "polkit"
    readonly property bool keyring: kind === "keyring"
    readonly property bool askpass: kind === "askpass"
    readonly property bool notice: kind === "notice"
    readonly property bool question: notice || ((keyring || askpass) && (payload.confirm ?? false))
    // For an SSH key: how long it stays in the agent.
    property string keep: payload.keep ?? ""
    readonly property bool twice: keyring && !question && (payload.passwordNew ?? false)
    readonly property var users: payload.users ?? []
    property string user: payload.user ?? ""
    property bool choice: payload.choice ?? false
    readonly property bool checking: payload.checking ?? false
    readonly property string error: payload.error ?? ""
    readonly property bool matching: !twice || field.input.text === confirmField.input.text
    readonly property bool ready: !checking && (question || (field.input.text !== "" && matching))

    implicitWidth: 420
    implicitHeight: column.implicitHeight + Theme.padding * 2

    // Each prompt, also the next in the same view, like the keyring's after
    // a wrong password, starts empty with the keyboard in its field.
    readonly property var promptId: payload.id ?? 0
    onPromptIdChanged: {
        field.input.text = "";
        confirmField.input.text = "";
        Qt.callLater(root.focusField);
    }
    Component.onCompleted: Qt.callLater(root.focusField)

    function focusField(): void {
        if (question)
            root.forceActiveFocus();
        else
            field.input.forceActiveFocus();
    }

    // A wrong password: start again, with the reason above the field.
    onErrorChanged: {
        if (error !== "" && !keyring) {
            field.input.text = "";
            field.input.forceActiveFocus();
        }
    }

    Keys.onReturnPressed: root.answer()
    Keys.onEnterPressed: root.answer()
    Keys.onEscapePressed: Daemon.event("dismiss")

    function answer(): void {
        if (!ready)
            return;
        if (notice) {
            Daemon.event("dismiss");
            return;
        }
        Daemon.command("auth", "answer", [JSON.stringify({
                "id": root.payload.id,
                "user": root.user,
                "password": root.question ? "" : field.input.text,
                "choice": root.choice,
                "keep": root.keep,
                "accept": true
            })]);
    }

    // A program's prompt, without the colon it ends on.
    function asked(text: string): string {
        return text.trim().replace(/:$/, "");
    }

    EdgeLight {
        radius: Theme.radiusSurface
        working: root.checking
    }

    Column {
        id: column

        x: Theme.padding
        y: Theme.padding
        width: root.width - Theme.padding * 2
        spacing: Theme.spaceMedium

        PanelHeader {
            width: parent.width
            icon: {
                if (root.keyring || (root.askpass && root.keep !== ""))
                    return "key";
                if (root.notice)
                    return "fingerprint";
                return root.askpass ? "password" : "admin_panel_settings";
            }
            title: {
                const app = root.payload.app ?? "";
                if (root.keyring)
                    return root.payload.title ?? root.payload.message ?? "The keyring needs a password";
                if (root.notice)
                    return app !== "" ? app : "A notice";
                if (root.askpass && root.question)
                    return app !== "" ? `${app} asks` : "A question";
                if (root.askpass && root.keep !== "")
                    return app !== "" ? `${app} needs the key's passphrase` : "A key's passphrase is needed";
                if (root.askpass)
                    return app !== "" ? `${app} needs a password` : "A password is needed";
                return app !== "" ? `${app} needs an admin password` : "An admin password is needed";
            }
        }

        // Why: polkit's reason, the keyring's message under its title, or
        // what sudo or ssh asks.
        Text {
            textFormat: Text.PlainText
            width: parent.width
            visible: text !== ""
            text: {
                if (root.keyring)
                    return root.payload.title ? root.payload.message ?? "" : "";
                if (root.askpass || root.notice)
                    return root.asked(root.payload.message ?? "");
                return root.payload.message ?? "";
            }
            wrapMode: Text.Wrap
            color: Theme.foreground
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        // More: the command that asks, as the kernel says it, like what
        // pkexec or sudo would run, or the keyring's description.
        Text {
            textFormat: Text.PlainText
            width: parent.width
            visible: text !== ""
            text: {
                if (root.keyring)
                    return root.payload.description ?? "";
                const command = root.payload.command ?? "";
                return command !== "" && root.payload.privileged ? `${command} · runs as root` : command;
            }
            wrapMode: root.keyring ? Text.Wrap : Text.WrapAnywhere
            maximumLineCount: 4
            elide: Text.ElideRight
            color: Theme.muted
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }
        // Why to look twice: the asker runs from a folder the user can
        // write to, or asked Mochi directly, or couldn't be told.
        Row {
            width: parent.width
            visible: (root.payload.doubt ?? "") !== ""
            spacing: Theme.spaceSmall

            Symbol {
                anchors.verticalCenter: parent.verticalCenter
                name: "warning"
                size: 16
                color: Theme.danger
            }

            Text {
                textFormat: Text.PlainText
                width: parent.width - 16 - parent.spacing
                text: root.payload.doubt ?? ""
                wrapMode: Text.Wrap
                color: Theme.danger
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }
        }

        Segmented {
            width: parent.width
            height: Theme.controlHeight
            visible: root.users.length > 1
            color: Theme.surface
            options: root.users.map(name => ({
                        "value": name,
                        "label": name,
                        "icon": "person"
                    }))
            current: root.user
            onPicked: value => {
                root.user = value;
                field.input.forceActiveFocus();
            }
        }

        Secret {
            id: field

            onAccepted: root.answer()

            next: confirmField

            visible: !root.question
            enabled: !root.checking
            shown: reveal.shown
            placeholder: {
                if (root.twice)
                    return "New password";
                if (root.keyring)
                    return "Password";
                return root.user !== "" ? `Password for ${root.user}` : "Password";
            }
        }

        Secret {
            id: confirmField

            onAccepted: root.answer()

            next: field

            visible: root.twice
            enabled: !root.checking
            shown: reveal.shown
            placeholder: "The same again"
        }

        Text {
            textFormat: Text.PlainText
            width: parent.width
            visible: root.twice && confirmField.input.text !== "" && !root.matching
            text: "The two passwords aren't the same."
            wrapMode: Text.Wrap
            color: Theme.danger
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }

        // How long an SSH key stays in the agent once added: until the
        // screen locks, a time, or until logging out.
        Column {
            width: parent.width
            visible: root.askpass && root.keep !== ""
            spacing: Theme.spaceSmall

            Text {
                textFormat: Text.PlainText
                text: "Keep the key"
                color: Theme.muted
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }

            Segmented {
                width: parent.width
                height: Theme.controlHeight
                color: Theme.surface
                options: [
                    {
                        "value": "until-locked",
                        "label": "Until locked"
                    },
                    {
                        "value": "1h",
                        "label": "1 h"
                    },
                    {
                        "value": "4h",
                        "label": "4 h"
                    },
                    {
                        "value": "8h",
                        "label": "8 h"
                    },
                    {
                        "value": "until-logout",
                        "label": "Logout"
                    }
                ]
                current: root.keep
                onPicked: value => {
                    root.keep = value;
                    field.input.forceActiveFocus();
                }
            }
        }

        SwitchRow {
            width: parent.width
            visible: root.keyring && (root.payload.choiceLabel ?? "") !== ""
            icon: "lock_open"
            title: root.payload.choiceLabel ?? ""
            checked: root.choice
            onToggled: checked => root.choice = checked
        }

        // What went wrong, in words, with what's left.
        Row {
            width: parent.width
            visible: root.error !== ""
            spacing: Theme.spaceSmall

            Symbol {
                anchors.verticalCenter: parent.verticalCenter
                name: "error"
                size: 16
                color: Theme.danger
            }

            Text {
                textFormat: Text.PlainText
                width: parent.width - 16 - parent.spacing
                text: {
                    const left = root.payload.triesLeft ?? 0;
                    return !root.keyring && left === 1 ? `${root.error} One try left.` : root.error;
                }
                wrapMode: Text.Wrap
                color: Theme.danger
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }
        }

        Row {
            anchors.right: parent.right
            spacing: Theme.spaceSmall

            Button {
                id: reveal

                property bool shown: false

                visible: !root.question
                text: shown ? "Hide" : "Show"
                tone: "ghost"
                onClicked: {
                    shown = !shown;
                    field.input.forceActiveFocus();
                }
            }

            Button {
                visible: !root.notice
                text: root.askpass && root.question ? "No" : root.payload.cancelLabel ?? "Cancel"
                tone: "ghost"
                onClicked: Daemon.event("dismiss")
            }

            Button {
                text: {
                    if (root.checking)
                        return "Checking";
                    if (root.notice)
                        return "OK";
                    if (root.keyring)
                        return root.payload.continueLabel ?? (root.question ? "Continue" : "Unlock");
                    if (root.askpass)
                        return root.question ? "Yes" : "Continue";
                    return "Authenticate";
                }
                tone: "accent"
                enabled: root.ready
                onClicked: root.answer()
            }
        }
    }

    // One line to type a secret in, in a pill.
    component Secret: Rectangle {
        id: secret

        property alias input: entry
        property string placeholder: ""
        property bool shown: false
        // Where Tab goes.
        property Item next: null
        signal accepted

        width: parent.width
        height: 40
        radius: height / 2
        color: Theme.surface
        border.width: entry.activeFocus ? 2 : 0
        border.color: Theme.accent
        opacity: enabled ? 1 : 0.6

        Symbol {
            id: lock

            anchors.left: parent.left
            anchors.leftMargin: Theme.spaceMedium
            anchors.verticalCenter: parent.verticalCenter
            name: "lock"
            size: 18
            color: Theme.muted
        }

        TextInput {
            id: entry

            anchors.left: lock.right
            anchors.leftMargin: Theme.spaceSmall
            anchors.right: parent.right
            anchors.rightMargin: Theme.spaceLarge
            anchors.verticalCenter: parent.verticalCenter
            echoMode: secret.shown ? TextInput.Normal : TextInput.Password
            color: Theme.foreground
            selectionColor: Theme.accent
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
            clip: true

            // A shown password isn't copied: the clipboard's history would
            // keep it.
            Keys.onPressed: event => {
                if (event.matches(StandardKey.Copy) || event.matches(StandardKey.Cut))
                    event.accepted = true;
            }
            Keys.onReturnPressed: secret.accepted()
            Keys.onEnterPressed: secret.accepted()
            Keys.onEscapePressed: Daemon.event("dismiss")
            Keys.onTabPressed: {
                if (secret.next && secret.next.visible)
                    secret.next.input.forceActiveFocus();
            }

            Text {
                textFormat: Text.PlainText
                visible: entry.text === ""
                text: secret.placeholder
                color: Theme.muted
                font: entry.font
            }
        }
    }
}
