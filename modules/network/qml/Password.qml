import QtQuick
import qs.island

// Asks what joining a network takes: its password, a user name too on WPA
// Enterprise, or for a hidden network its name and security first. It also
// asks the password NetworkManager needs for a saved network. Tab moves
// between the fields, Enter joins, Escape cancels.
Item {
    id: root

    property var payload: ({})
    // join, enterprise, hidden or secret.
    readonly property string mode: payload.mode ?? "join"
    readonly property string ssid: payload.ssid ?? ""
    // For a hidden network: open, password or enterprise.
    property string security: "password"
    readonly property bool asksName: mode === "hidden"
    readonly property bool asksIdentity: mode === "enterprise" || (mode === "hidden" && security === "enterprise")
    readonly property bool asksPassword: !(mode === "hidden" && security === "open")
    readonly property bool ready: (!asksName || name.text.trim() !== "") && (!asksIdentity || identity.text !== "") && (!asksPassword || password.text !== "")
    // Sent, until the island moves on.
    property bool joining: false

    implicitWidth: 400
    implicitHeight: column.implicitHeight + Theme.padding * 2

    Component.onCompleted: Qt.callLater(() => (asksName ? name : asksIdentity ? identity : password).focusInput())

    function join(): void {
        if (!ready)
            return;
        Daemon.command("network", "answer", [JSON.stringify({
                "ssid": name.text,
                "identity": identity.text,
                "password": password.text,
                "security": root.security
            })]);
        root.joining = true;
    }

    // One line to type in, in a pill. Enter says `accepted`; an inline
    // component can't reach `root`.
    component Field: Rectangle {
        id: field

        property alias text: input.text
        property string placeholder: ""
        property bool secret: false
        property Item next: null
        signal accepted

        function focusInput(): void {
            input.forceActiveFocus();
        }

        width: parent.width
        height: 40
        radius: height / 2
        color: Theme.surface

        TextInput {
            id: input

            anchors.left: parent.left
            anchors.leftMargin: Theme.spaceLarge
            anchors.right: reveal.visible ? reveal.left : parent.right
            anchors.rightMargin: Theme.spaceSmall
            anchors.verticalCenter: parent.verticalCenter
            echoMode: field.secret && !reveal.shown ? TextInput.Password : TextInput.Normal
            color: Theme.foreground
            selectionColor: Theme.accent
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
            clip: true

            Keys.onReturnPressed: field.accepted()
            Keys.onEnterPressed: field.accepted()
            Keys.onEscapePressed: Daemon.event("dismiss")
            Keys.onTabPressed: {
                if (field.next && field.next.visible)
                    field.next.focusInput();
            }

            Text {
                visible: input.text === ""
                text: field.placeholder
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
            visible: field.secret
            tone: "ghost"
            text: shown ? "Hide" : "Show"
            onClicked: shown = !shown
        }
    }

    EdgeLight {
        radius: Theme.radiusSurface
        working: root.joining
    }

    Column {
        id: column

        x: Theme.padding
        y: Theme.padding
        width: root.width - Theme.padding * 2
        spacing: Theme.spaceMedium

        PanelHeader {
            width: parent.width
            title: {
                switch (root.mode) {
                case "hidden":
                    return "Join a hidden network";
                case "secret":
                    return `Password for ${root.ssid}`;
                default:
                    return `Join ${root.ssid}`;
                }
            }
        }

        // Why NetworkManager asks.
        Text {
            width: parent.width
            visible: text !== ""
            text: {
                if (root.mode !== "secret")
                    return "";
                if (root.payload.retry)
                    return "The password didn't work. Try it again.";
                return root.payload.identity ? `Signing in as ${root.payload.identity}` : "NetworkManager needs it to connect.";
            }
            wrapMode: Text.Wrap
            color: root.payload.retry ? Theme.danger : Theme.muted
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }

        Field {
            id: name

            onAccepted: root.join()

            visible: root.asksName
            placeholder: "Network name"
            next: root.asksIdentity ? identity : password
        }

        Segmented {
            width: parent.width
            height: Theme.controlHeight
            visible: root.asksName
            color: Theme.surface
            options: [
                {
                    "value": "open",
                    "label": "Open",
                    "icon": "lock_open"
                },
                {
                    "value": "password",
                    "label": "Password",
                    "icon": "lock"
                },
                {
                    "value": "enterprise",
                    "label": "Enterprise",
                    "icon": "badge"
                }
            ]
            current: root.security
            onPicked: value => root.security = value
        }

        Field {
            id: identity

            onAccepted: root.join()

            visible: root.asksIdentity
            placeholder: "User name"
            next: password
        }

        Field {
            id: password

            onAccepted: root.join()

            visible: root.asksPassword
            placeholder: "Password"
            secret: true
            next: root.asksName ? name : root.asksIdentity ? identity : null
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
                text: root.mode === "secret" ? "Connect" : "Join"
                tone: "accent"
                enabled: root.ready
                onClicked: root.join()
            }
        }
    }
}
