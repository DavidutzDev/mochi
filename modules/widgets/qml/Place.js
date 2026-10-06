.pragma library

// Where widgets go. A widget's anchor point, like its bottom-right corner,
// sits at the same point of the screen, moved by x and y grid cells.

// How far across and down an anchor's point is: 0, 0.5 or 1.
function factors(anchor) {
    const across = anchor.endsWith("left") ? 0 : anchor.endsWith("right") ? 1 : 0.5;
    const down = anchor.startsWith("top") ? 0 : anchor.startsWith("bottom") ? 1 : 0.5;
    return [across, down];
}

// A placed widget's rectangle in pixels on a screen `width` by `height`.
function rect(widget, cell, width, height) {
    const [across, down] = factors(widget.anchor ?? "top-left");
    const w = widget.width * cell;
    const h = widget.height * cell;
    return {
        x: across * (width - w) + widget.x * cell,
        y: down * (height - h) + widget.y * cell,
        width: w,
        height: h
    };
}

// The anchor and offsets for a widget whose top-left corner is at x, y: the
// anchor of the third of the screen its middle is in, so it keeps to that
// side when the screen changes size.
function place(x, y, w, h, cell, width, height) {
    const middleX = x + w / 2;
    const middleY = y + h / 2;
    const across = middleX < width / 3 ? "left" : middleX > width * 2 / 3 ? "right" : "";
    const down = middleY < height / 3 ? "top" : middleY > height * 2 / 3 ? "bottom" : "";
    const anchor = across && down ? `${down}-${across}` : (down || across || "center");
    const [fx, fy] = factors(anchor);
    return {
        anchor: anchor,
        x: Math.round((x - fx * (width - w)) / cell),
        y: Math.round((y - fy * (height - h)) / cell)
    };
}

// To the nearest cell.
function snap(value, cell) {
    return Math.round(value / cell) * cell;
}

// Brings a ListModel of {key} rows in line with `keys`, keeping the rows
// that stay, so their views keep running instead of loading again.
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
