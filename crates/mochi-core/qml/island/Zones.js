.pragma library

// Finds time zones by what people type: a city, a region, a country or an
// offset, like "tok", "buenos aires", "japan" or "utc+9". The zones are what
// a module publishes as `timezones`, from mochi_core::zones, west to east:
// {value, label, detail}, like {"value": "Asia/Tokyo", "label": "Tokyo,
// Asia", "detail": "UTC+9 · Japan"}. ZonePicker lists what `rows` gives.

// Lower case, without accents or underscores, so "sao" finds São Paulo and
// "São" finds Sao_Paulo.
function fold(text) {
    return String(text ?? "").normalize("NFD").replace(/[̀-ͯ]/g, "").replace(/_/g, " ").toLowerCase();
}

// How well `zone` matches the typed `words`, all of `whole`: 0 when its
// city starts with the whole query, 1 when a word of its city starts with
// the first word, 2 when each word starts a word of it, 3 when each is
// somewhere in it, and -1 when one isn't.
function rank(zone, words, whole) {
    const city = fold(String(zone.label ?? "").split(",")[0]);
    const text = fold(`${zone.label ?? ""} ${zone.value ?? ""} ${zone.detail ?? ""}`);
    if (!words.every(word => text.includes(word)))
        return -1;
    if (city.startsWith(whole))
        return 0;
    if (city.split(/[\s\-\/]+/).some(part => part.startsWith(words[0])))
        return 1;
    const parts = text.split(/[\s,\/·()\-]+/);
    if (words.every(word => parts.some(part => part.startsWith(word))))
        return 2;
    return 3;
}

// The zones that match `query`, the best first, and west to east among
// those that match as well. Every zone for an empty query.
function search(zones, query) {
    const whole = fold(query).trim();
    const words = whole.split(/\s+/).filter(word => word !== "");
    if (words.length === 0)
        return zones.slice();
    return zones.map((zone, index) => ({
                "zone": zone,
                "index": index,
                "rank": rank(zone, words, whole)
            })).filter(match => match.rank >= 0).sort((a, b) => a.rank - b.rank || a.index - b.index).map(match => match.zone);
}

// What a picker lists, as {kind: "heading", label} and {kind: "zone",
// value, label, detail} rows. With a query: the matches, and with `custom`
// what's typed first, as itself, when it reads like a zone's name and none
// has it. Without: the `chosen` zones, then the `suggested` ones not
// chosen, then every other zone, each under its title in `titles`
// ({chosen, suggested, all}). A chosen zone the list doesn't have, like
// one typed in the settings, comes as its name.
function rows(zones, query, chosen, suggested, titles, custom) {
    const typed = String(query ?? "").trim();
    const byName = {};
    for (const zone of zones)
        byName[zone.value] = zone;
    const row = zone => ({
                "kind": "zone",
                "value": zone.value,
                "label": zone.label,
                "detail": zone.detail ?? ""
            });
    if (typed !== "") {
        const found = search(zones, typed).map(row);
        const named = /^[A-Za-z0-9_+\-]+(\/[A-Za-z0-9_+\-]+)*$/.test(typed);
        if (custom && named && byName[typed] === undefined)
            found.unshift({
                "kind": "zone",
                "value": typed,
                "label": `Use “${typed}”`,
                "detail": "Typed, not in the system's list"
            });
        return found;
    }
    const out = [];
    const heading = label => {
        if (label)
            out.push({
                "kind": "heading",
                "label": label
            });
    };
    const shown = new Set();
    if (chosen.length > 0) {
        heading(titles.chosen);
        for (const value of chosen) {
            shown.add(value);
            out.push(byName[value] ? row(byName[value]) : {
                "kind": "zone",
                "value": value,
                "label": value,
                "detail": "Not in the system's list"
            });
        }
    }
    const more = suggested.filter(value => !shown.has(value) && byName[value]);
    if (more.length > 0) {
        heading(titles.suggested);
        for (const value of more) {
            shown.add(value);
            out.push(row(byName[value]));
        }
    }
    const rest = zones.filter(zone => !shown.has(zone.value));
    if (rest.length > 0) {
        heading(titles.all);
        for (const zone of rest)
            out.push(row(zone));
    }
    return out;
}
