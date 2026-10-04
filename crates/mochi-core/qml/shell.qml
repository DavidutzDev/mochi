import QtQuick
import Quickshell
import qs.island

ShellRoot {
    // The same island on every screen, and the space it keeps free.
    Variants {
        model: Quickshell.screens

        Scope {
            id: screen

            required property ShellScreen modelData

            IslandWindow {
                id: island

                screen: screen.modelData
            }

            EdgeReserve {
                screen: screen.modelData
                atBottom: island.atBottom
                zone: island.zone
            }
        }
    }
}
