.pragma library

// Brings a ListModel of {key, leaving} rows in line with `keys` by moving
// and inserting rows. Rows that stay keep their delegates, so views keep
// their state and animations instead of reloading. Rows whose key went away
// stay with `leaving` set, so their delegates can animate out and call
// `removeLeft` when done; a key that comes back before then stays put.
function sync(model, keys) {
    for (let i = 0; i < model.count; i++) {
        const leaving = !keys.includes(model.get(i).key);
        if (model.get(i).leaving !== leaving)
            model.setProperty(i, "leaving", leaving);
    }
    // Live rows in the order of `keys`; leaving rows keep their place among
    // them.
    let at = 0;
    for (const key of keys) {
        while (at < model.count && model.get(at).leaving)
            at++;
        let found = -1;
        for (let j = at; j < model.count; j++) {
            if (model.get(j).key === key) {
                found = j;
                break;
            }
        }
        if (found === -1)
            model.insert(at, { key: key, leaving: false });
        else if (found !== at)
            model.move(found, at, 1);
        at++;
    }
}

// Removes the row with `key` if it is still leaving.
function removeLeft(model, key) {
    for (let i = 0; i < model.count; i++) {
        const row = model.get(i);
        if (row.key === key && row.leaving) {
            model.remove(i);
            return;
        }
    }
}

// What makes a bubble the same bubble across snapshots: a keyed replacement
// keeps it, a new view does not.
function bubbleKey(bubble) {
    return `${bubble.module}/${bubble.key ?? "#" + bubble.id}/${bubble.view}`;
}
