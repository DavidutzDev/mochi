.pragma library

// Color conversion and formatting routines for the color picker.

// Converts HSV to RGB.
// h: 0.0 to 1.0 (or 0 to 360 degrees)
// s: 0.0 to 1.0
// v: 0.0 to 1.0
// Returns { r, g, b } with integers in [0, 255].
function hsvToRgb(h, s, v) {
    let deg = h > 1.0 ? ((h % 360 + 360) % 360) : (((h % 1.0 + 1.0) % 1.0) * 360);
    let sat = Math.max(0.0, Math.min(1.0, s));
    let val = Math.max(0.0, Math.min(1.0, v));

    let c = val * sat;
    let x = c * (1.0 - Math.abs(((deg / 60.0) % 2.0) - 1.0));
    let m = val - c;
    let r1 = 0.0, g1 = 0.0, b1 = 0.0;

    if (deg < 60.0) {
        r1 = c; g1 = x; b1 = 0.0;
    } else if (deg < 120.0) {
        r1 = x; g1 = c; b1 = 0.0;
    } else if (deg < 180.0) {
        r1 = 0.0; g1 = c; b1 = x;
    } else if (deg < 240.0) {
        r1 = 0.0; g1 = x; b1 = c;
    } else if (deg < 300.0) {
        r1 = x; g1 = 0.0; b1 = c;
    } else {
        r1 = c; g1 = 0.0; b1 = x;
    }

    return {
        r: Math.min(255, Math.max(0, Math.round((r1 + m) * 255))),
        g: Math.min(255, Math.max(0, Math.round((g1 + m) * 255))),
        b: Math.min(255, Math.max(0, Math.round((b1 + m) * 255)))
    };
}

// Converts RGB (0..255) to HSV.
// Returns { h: 0.0..1.0, s: 0.0..1.0, v: 0.0..1.0 }.
function rgbToHsv(r, g, b) {
    let rn = Math.max(0, Math.min(255, r)) / 255.0;
    let gn = Math.max(0, Math.min(255, g)) / 255.0;
    let bn = Math.max(0, Math.min(255, b)) / 255.0;

    let max = Math.max(rn, gn, bn);
    let min = Math.min(rn, gn, bn);
    let delta = max - min;

    let h = 0.0;
    if (delta > 0.00001) {
        if (max === rn) {
            h = ((gn - bn) / delta) % 6.0;
        } else if (max === gn) {
            h = (bn - rn) / delta + 2.0;
        } else {
            h = (rn - gn) / delta + 4.0;
        }
        h = h / 6.0;
        if (h < 0.0) h += 1.0;
    }

    let s = max > 0.0 ? delta / max : 0.0;
    let v = max;

    return { h: h, s: s, v: v };
}

// Converts RGB (0..255) to HSL.
// Returns { h: 0..360, s: 0.0..1.0, l: 0.0..1.0 }.
function rgbToHsl(r, g, b) {
    let rn = Math.max(0, Math.min(255, r)) / 255.0;
    let gn = Math.max(0, Math.min(255, g)) / 255.0;
    let bn = Math.max(0, Math.min(255, b)) / 255.0;

    let max = Math.max(rn, gn, bn);
    let min = Math.min(rn, gn, bn);
    let delta = max - min;
    let l = (max + min) / 2.0;

    let h = 0.0;
    if (delta > 0.00001) {
        if (max === rn) {
            h = ((gn - bn) / delta) % 6.0;
        } else if (max === gn) {
            h = (bn - rn) / delta + 2.0;
        } else {
            h = (rn - gn) / delta + 4.0;
        }
        h = h * 60.0;
        if (h < 0.0) h += 360.0;
    }

    let s = 0.0;
    if (delta > 0.00001) {
        let denom = 1.0 - Math.abs(2.0 * l - 1.0);
        s = denom > 0.0 ? delta / denom : 0.0;
    }

    return { h: Math.round(h), s: s, l: l };
}

// Converts RGB (0..255) to CMYK (0..100 integers).
function rgbToCmyk(r, g, b) {
    let rn = Math.max(0, Math.min(255, r)) / 255.0;
    let gn = Math.max(0, Math.min(255, g)) / 255.0;
    let bn = Math.max(0, Math.min(255, b)) / 255.0;

    let max = Math.max(rn, gn, bn);
    let k = 1.0 - max;

    if (k >= 0.9999) {
        return { c: 0, m: 0, y: 0, k: 100 };
    }

    let c = Math.round(((1.0 - rn - k) / (1.0 - k)) * 100.0);
    let m = Math.round(((1.0 - gn - k) / (1.0 - k)) * 100.0);
    let y = Math.round(((1.0 - bn - k) / (1.0 - k)) * 100.0);

    return {
        c: Math.max(0, Math.min(100, c)),
        m: Math.max(0, Math.min(100, m)),
        y: Math.max(0, Math.min(100, y)),
        k: Math.max(0, Math.min(100, Math.round(k * 100.0)))
    };
}

// Converts RGB (0..255) to hex string like "#fb542b".
function rgbToHex(r, g, b, uppercase) {
    let rHex = Math.max(0, Math.min(255, Math.round(r))).toString(16).padStart(2, "0");
    let gHex = Math.max(0, Math.min(255, Math.round(g))).toString(16).padStart(2, "0");
    let bHex = Math.max(0, Math.min(255, Math.round(b))).toString(16).padStart(2, "0");
    let hex = "#" + rHex + gHex + bHex;
    return uppercase ? hex.toUpperCase() : hex.toLowerCase();
}

// Parses hex string ("#rrggbb", "#rgb", "rrggbb") to { r, g, b } or null.
function hexToRgb(hex) {
    if (!hex || typeof hex !== "string") return null;
    let clean = hex.trim().replace(/^#/, "");
    if (clean.length === 3) {
        clean = clean[0] + clean[0] + clean[1] + clean[1] + clean[2] + clean[2];
    }
    if (clean.length !== 6 && clean.length !== 8) return null;
    let num = parseInt(clean.substring(0, 6), 16);
    if (isNaN(num)) return null;
    return {
        r: (num >> 16) & 255,
        g: (num >> 8) & 255,
        b: num & 255
    };
}

function formatHex(r, g, b, uppercase) {
    return rgbToHex(r, g, b, uppercase);
}

function formatRgb(r, g, b) {
    return `${r}, ${g}, ${b}`;
}

function formatCmyk(r, g, b) {
    let c = rgbToCmyk(r, g, b);
    return `${c.c}%, ${c.m}%, ${c.y}%, ${c.k}%`;
}

function formatHsv(h, s, v) {
    let deg = h > 1.0 ? ((h % 360 + 360) % 360) : (((h % 1.0 + 1.0) % 1.0) * 360);
    let sPct = Math.round(Math.max(0, Math.min(1, s)) * 100);
    let vPct = Math.round(Math.max(0, Math.min(1, v)) * 100);
    return `${Math.round(deg)}°, ${sPct}%, ${vPct}%`;
}

function formatHsl(r, g, b) {
    let hsl = rgbToHsl(r, g, b);
    let sPct = Math.round(Math.max(0, Math.min(1, hsl.s)) * 100);
    let lPct = Math.round(Math.max(0, Math.min(1, hsl.l)) * 100);
    return `${hsl.h}°, ${sPct}%, ${lPct}%`;
}
