.pragma library

// Brings a ListModel of {key} rows in line with `keys` by removing, moving
// and inserting rows. Rows that stay keep their delegates, so views keep
// their state and animations instead of reloading.
function sync(model, keys) {
    for (let i = model.count - 1; i >= 0; i--) {
        if (!keys.includes(model.get(i).key))
            model.remove(i);
    }
    for (let i = 0; i < keys.length; i++) {
        let found = -1;
        for (let j = i; j < model.count; j++) {
            if (model.get(j).key === keys[i]) {
                found = j;
                break;
            }
        }
        if (found === -1)
            model.insert(i, { key: keys[i] });
        else if (found !== i)
            model.move(found, i, 1);
    }
}

// What makes a bubble the same bubble across snapshots: a keyed replacement
// keeps it, a new view does not.
function bubbleKey(bubble) {
    return `${bubble.module}/${bubble.key ?? "#" + bubble.id}/${bubble.view}`;
}
