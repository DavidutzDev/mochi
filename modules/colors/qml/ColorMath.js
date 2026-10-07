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

// Converts HSL to RGB.
// h: 0..360, s: 0.0..1.0, l: 0.0..1.0.
function hslToRgb(h, s, l) {
    let deg = ((h % 360) + 360) % 360;
    let sat = Math.max(0.0, Math.min(1.0, s));
    let lig = Math.max(0.0, Math.min(1.0, l));

    let c = (1.0 - Math.abs(2.0 * lig - 1.0)) * sat;
    let x = c * (1.0 - Math.abs(((deg / 60.0) % 2.0) - 1.0));
    let m = lig - c / 2.0;
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

// Common CSS color names dictionary
const CSS_COLORS = {
    black: [0, 0, 0], white: [255, 255, 255], red: [255, 0, 0],
    green: [0, 128, 0], blue: [0, 0, 255], yellow: [255, 255, 0],
    cyan: [0, 255, 255], magenta: [255, 0, 255], orange: [255, 165, 0],
    purple: [128, 0, 128], pink: [255, 192, 203], coral: [255, 127, 80],
    tomato: [255, 99, 71], gold: [255, 215, 0], gray: [128, 128, 128],
    grey: [128, 128, 128], lime: [0, 255, 0], teal: [0, 128, 128],
    navy: [0, 0, 128], violet: [238, 130, 238], indigo: [75, 0, 130],
    brown: [165, 42, 42], maroon: [128, 0, 0], olive: [128, 128, 0],
    silver: [192, 192, 192], aqua: [0, 255, 255], fuchsia: [255, 0, 255],
    turquoise: [64, 224, 208], salmon: [250, 128, 114], crimson: [220, 20, 60]
};

// Parses any color input string (Hex, RGB, HSL, HSV, or CSS name).
// Returns { r, g, b } or null if invalid.
function parseColorString(str) {
    if (!str || typeof str !== "string") return null;
    let s = str.trim().toLowerCase();
    if (s.length === 0) return null;

    // 1. CSS named colors
    if (CSS_COLORS[s]) {
        let rgb = CSS_COLORS[s];
        return { r: rgb[0], g: rgb[1], b: rgb[2] };
    }

    // 2. Hex formats: #ffffff, #fff, ffffff, fff, with optional alpha
    let hexClean = s.replace(/^#/, "");
    if (/^[0-9a-f]{3}$/i.test(hexClean)) {
        return {
            r: parseInt(hexClean[0] + hexClean[0], 16),
            g: parseInt(hexClean[1] + hexClean[1], 16),
            b: parseInt(hexClean[2] + hexClean[2], 16)
        };
    }
    if (/^[0-9a-f]{6}/i.test(hexClean)) {
        return {
            r: parseInt(hexClean.substring(0, 2), 16),
            g: parseInt(hexClean.substring(2, 4), 16),
            b: parseInt(hexClean.substring(4, 6), 16)
        };
    }

    // 3. RGB: "rgb(251, 84, 43)", "rgb(251 84 43)", "251, 84, 43", "251 84 43"
    let rgbFunc = s.match(/^rgba?\s*\(\s*(\d+)[,\s]+(\d+)[,\s]+(\d+)/i);
    let rgbPlain = s.match(/^(\d{1,3})[,\s]+(\d{1,3})[,\s]+(\d{1,3})$/);
    let rgbMatch = rgbFunc || rgbPlain;
    if (rgbMatch) {
        let r = parseInt(rgbMatch[1], 10);
        let g = parseInt(rgbMatch[2], 10);
        let b = parseInt(rgbMatch[3], 10);
        if (r <= 255 && g <= 255 && b <= 255) {
            return { r: r, g: g, b: b };
        }
    }

    // 4. HSL: "hsl(12, 96%, 58%)", "12°, 96%, 58%", "12, 96%, 58%"
    let hslFunc = s.match(/^hsla?\s*\(\s*(\d+)°?[,\s]+(\d+)%?[,\s]+(\d+)%?/i);
    let hslPlain = s.match(/^(\d+)°?[,\s]+(\d+)%[,\s]+(\d+)%$/);
    let hslMatch = hslFunc || hslPlain;
    if (hslMatch) {
        let h = parseInt(hslMatch[1], 10);
        let sat = parseInt(hslMatch[2], 10) / 100.0;
        let lig = parseInt(hslMatch[3], 10) / 100.0;
        return hslToRgb(h, sat, lig);
    }

    // 5. HSV: "hsv(12, 83%, 98%)", "12°, 83%, 98%"
    let hsvFunc = s.match(/^hsva?\s*\(\s*(\d+)°?[,\s]+(\d+)%?[,\s]+(\d+)%?/i);
    let hsvPlain = s.match(/^(\d+)°[,\s]+(\d+)%[,\s]+(\d+)%$/);
    let hsvMatch = hsvFunc || hsvPlain;
    if (hsvMatch) {
        let h = parseInt(hsvMatch[1], 10);
        let sat = parseInt(hsvMatch[2], 10) / 100.0;
        let val = parseInt(hsvMatch[3], 10) / 100.0;
        return hsvToRgb(h, sat, val);
    }

    return null;
}
