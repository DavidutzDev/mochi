import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.island

ShellRoot {
    // The same island on every screen, and the space it keeps free.
    Variants {
        model: Daemon.screens

        Scope {
            id: screen

            required property ShellScreen modelData

            IslandWindow {
                id: island

                screen: screen.modelData
            }

            // The power module's keep awake: the compositor doesn't go idle
            // while the island is on screen.
            IdleInhibitor {
                window: island
                enabled: Daemon.state("power")?.awake ?? false
            }

            EdgeReserve {
                screen: screen.modelData
                atBottom: island.atBottom
                zone: island.zone
            }

            // Widgets, under the windows.
            DesktopWindow {
                screen: screen.modelData
            }
        }
    }

    // A monitor Mochi made for a module, MOCHI-<MODULE>, shows that module's
    // Screen.qml over all of it, with the module's state as payload. The
    // share module's switchable screen draws its copy this way.
    Variants {
        model: Daemon.virtualScreens

        PanelWindow {
            id: window

            required property ShellScreen modelData
            readonly property string module: (modelData?.name ?? "").slice("MOCHI-".length).toLowerCase()

            screen: modelData
            anchors {
                top: true
                bottom: true
                left: true
                right: true
            }
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.namespace: `mochi-${window.module}`
            color: "black"
            // Nothing to click there.
            mask: Region {}

            Loader {
                id: view

                anchors.fill: parent
                source: Daemon.modules.includes(window.module) ? `root:/modules/${window.module}/Screen.qml` : ""
            }

            Binding {
                target: view.item
                property: "payload"
                value: Daemon.state(window.module)
                when: view.item !== null
            }
        }
    }
}
