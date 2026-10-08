import QtQuick
import qs.island

// A module's bubble, with made-up data, for a step of the tour: the tour
// owns the bubble, the module's view draws it.
Item {
    id: root

    property var payload: ({})

    implicitWidth: view.item ? view.item.implicitWidth : 0
    implicitHeight: view.item ? view.item.implicitHeight : 0

    Loader {
        id: view

        anchors.fill: parent
        enabled: false

        Component.onCompleted: {
            if (!Daemon.modules.includes(root.payload.module))
                return;
            setSource(`root:/modules/${root.payload.module}/${root.payload.view}.qml`, {
                "payload": root.payload.payload ?? {}
            });
        }
    }
}
