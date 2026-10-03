import QtQuick
import Quickshell
import qs.island

ShellRoot {
    // The same island on every screen.
    Variants {
        model: Quickshell.screens

        IslandWindow {
            required property ShellScreen modelData
            screen: modelData
        }
    }
}
