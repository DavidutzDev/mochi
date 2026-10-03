import QtQuick
import Quickshell
import qs.island

ShellRoot {
    // One island per screen. Which screens get an island is an open question
    // in TODO.md; the spike mirrors the same activity on all of them.
    Variants {
        model: Quickshell.screens

        IslandWindow {
            required property ShellScreen modelData
            screen: modelData
        }
    }
}
