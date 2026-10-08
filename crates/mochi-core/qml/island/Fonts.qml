pragma Singleton

import QtQuick
import Quickshell

// The font families installed on the system, as Qt finds them through
// fontconfig: the ones a view can draw. Read once, when first used;
// `refresh()` reads them again, like after installing a font.
Singleton {
    id: root

    property var families: read()

    function read(): var {
        // Names starting with a dot are fonts Qt keeps for itself.
        const names = Qt.fontFamilies().filter(name => name !== "" && !name.startsWith("."));
        return [...new Set(names)].sort((a, b) => a.localeCompare(b, undefined, {
                    "sensitivity": "base"
                }));
    }

    function refresh(): void {
        families = read();
    }

    function has(family: string): bool {
        return families.includes(family);
    }

    // The families whose names have every word of `query`, in any case:
    // "mono jet" finds "JetBrains Mono".
    function search(query: string): var {
        const words = query.toLowerCase().split(/\s+/).filter(word => word !== "");
        if (words.length === 0)
            return families;
        return families.filter(family => {
            const name = family.toLowerCase();
            return words.every(word => name.includes(word));
        });
    }
}
