import QtQuick
import qs.island

// The connection at a glance: the Wi-Fi's strength, Ethernet, or offline,
// with a lock while a VPN runs. A click opens the hub's Network page.
Item {
    id: root

    property var payload: ({})

    implicitWidth: 26
    implicitHeight: 26

    Symbol {
        anchors.centerIn: parent
        name: root.payload.icon ?? "offline"
        size: 16
        color: root.payload.icon === "offline" ? Theme.muted : Theme.foreground
    }

    Symbol {
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: 1
        visible: (root.payload.vpn ?? null) !== null
        name: "lock"
        size: 10
        color: Theme.accent
    }
}
