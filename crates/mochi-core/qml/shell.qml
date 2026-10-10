import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.island

ShellRoot {
    // Views reload as they change only for `mochid --dev`. Otherwise the
    // daemon says when, once it has written all of them, so a module
    // turned on reloads the shell once and without a popup.
    settings.watchFiles: Quickshell.env("MOCHI_WATCH") === "1"

    Connections {
        target: Quickshell

        function onReloadCompleted(): void {
            Quickshell.inhibitReloadPopup();
        }

        function onReloadFailed(error: string): void {
            console.warn(`mochi: the views didn't reload: ${error}`);
            if (Quickshell.env("MOCHI_WATCH") !== "1")
                Quickshell.inhibitReloadPopup();
        }
    }

    // Panels compiled ahead, so their first open is quick.
    Preloader {}

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
