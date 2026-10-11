import QtQuick
import qs.island

// Asks for a password polkit needs: an app wants an admin action, like
// pkexec, systemctl or a settings app. It says which app asks and why, and
// for pkexec the command. When several users may allow it, a choice picks
// who types. Enter checks, Escape or Cancel declines. polkit's own helper
// checks the password; a wrong one clears the field and says so.
Item {
    id: root

    property var payload: ({})
    readonly property var users: payload.users ?? []
    property string user: payload.user ?? ""
    readonly property bool checking: payload.checking ?? false
    readonly property string error: payload.error ?? ""
    readonly property bool ready: password.text !== "" && !checking

    implicitWidth: 420
    implicitHeight: column.implicitHeight + Theme.padding * 2

    Component.onCompleted: Qt.callLater(() => input.forceActiveFocus())

    // A wrong password: start again, with the reason above the field.
    onErrorChanged: {
        if (error !== "") {
            input.text = "";
            input.forceActiveFocus();
        }
    }

    function authenticate(): void {
        if (!ready)
            return;
        Daemon.command("auth", "answer", [JSON.stringify({
                "id": root.payload.id,
                "user": root.user,
                "password": input.text
            })]);
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
            icon: "admin_panel_settings"
            title: root.payload.app ? `${root.payload.app} needs an admin password` : "An admin password is needed"
        }

        // Why, as polkit says it.
        Text {
            width: parent.width
            visible: text !== ""
            text: root.payload.message ?? ""
            wrapMode: Text.Wrap
            color: Theme.foreground
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        // What pkexec would run.
        Text {
            width: parent.width
            visible: text !== ""
            text: root.payload.command ?? ""
            wrapMode: Text.WrapAnywhere
            maximumLineCount: 3
            elide: Text.ElideRight
            color: Theme.muted
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
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
                input.forceActiveFocus();
            }
        }

        Rectangle {
            id: password

            property alias text: input.text

            width: parent.width
            height: 40
            radius: height / 2
            color: Theme.surface
            border.width: input.activeFocus ? 2 : 0
            border.color: root.error !== "" ? Theme.danger : Theme.accent
            opacity: root.checking ? 0.6 : 1

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
                id: input

                anchors.left: lock.right
                anchors.leftMargin: Theme.spaceSmall
                anchors.right: reveal.left
                anchors.rightMargin: Theme.spaceSmall
                anchors.verticalCenter: parent.verticalCenter
                enabled: !root.checking
                echoMode: reveal.shown ? TextInput.Normal : TextInput.Password
                color: Theme.foreground
                selectionColor: Theme.accent
                font.pixelSize: Theme.textBody
                font.family: Theme.fontFamily
                clip: true

                Keys.onReturnPressed: root.authenticate()
                Keys.onEnterPressed: root.authenticate()
                Keys.onEscapePressed: Daemon.event("dismiss")

                Text {
                    visible: input.text === ""
                    text: root.user !== "" ? `Password for ${root.user}` : "Password"
                    color: Theme.muted
                    font: input.font
                }
            }

            Button {
                id: reveal

                property bool shown: false

                anchors.right: parent.right
                anchors.rightMargin: Theme.spaceTiny
                anchors.verticalCenter: parent.verticalCenter
                tone: "ghost"
                text: shown ? "Hide" : "Show"
                onClicked: {
                    shown = !shown;
                    input.forceActiveFocus();
                }
            }
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
                width: parent.width - 16 - parent.spacing
                text: {
                    const left = root.payload.triesLeft ?? 0;
                    return left === 1 ? `${root.error} One try left.` : root.error;
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
                text: "Cancel"
                tone: "ghost"
                onClicked: Daemon.event("dismiss")
            }

            Button {
                text: root.checking ? "Checking" : "Authenticate"
                tone: "accent"
                enabled: root.ready
                onClicked: root.authenticate()
            }
        }
    }
}
