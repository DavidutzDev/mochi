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

// Where a widget whose top-left corner is near x, y ends up once saved: on
// the grid of whole cells counted from its anchor. That is the screen's
// grid on the left and top, but counts from the far edge on the right and
// bottom, and from the middle in between, when the screen isn't a whole
// number of cells.
function saved(x, y, w, h, cell, width, height) {
    const spot = place(x, y, w, h, cell, width, height);
    return rect({
        anchor: spot.anchor,
        x: spot.x,
        y: spot.y,
        width: w / cell,
        height: h / cell
    }, cell, width, height);
}

// Whether a widget whose top-left corner is at x, y is saved at exactly
// that spot, across and down. Offsets are whole cells from the anchor, so a
// spot only fits when it is a whole number of cells from where its anchor
// puts the widget.
function fits(x, y, w, h, cell, width, height) {
    const back = saved(x, y, w, h, cell, width, height);
    return [Math.abs(back.x - x) < 0.5, Math.abs(back.y - y) < 0.5];
}

// A rectangle's start, middle and end across (`axis` 0) or down (1).
function lines(r, axis) {
    return axis === 0 ? [r.x, r.x + r.width / 2, r.x + r.width] : [r.y, r.y + r.height / 2, r.y + r.height];
}

// What a widget lines up with on one axis: the other widgets' edges and
// middles, and the screen's middle.
function targets(others, axis, size) {
    const all = [size / 2];
    for (const other of others)
        all.push(...lines(other, axis));
    return all;
}

// For a widget being moved: how far to shift it along `axis` so one of its
// lines meets a target, the nearest within `reach` pixels that `ok` takes,
// or null.
function guideShift(r, axis, others, size, reach, ok) {
    let best = null;
    for (const target of targets(others, axis, size)) {
        for (const line of lines(r, axis)) {
            const shift = target - line;
            if (Math.abs(shift) > reach || (best !== null && Math.abs(shift) >= Math.abs(best)))
                continue;
            if (ok(shift))
                best = shift;
        }
    }
    return best;
}

// For a widget being resized from its far corner, which keeps its start:
// the length in whole cells that puts its end or middle on a target within
// `reach` pixels of where the pointer has it, and that `ok` takes, or null.
function guideLength(start, length, others, axis, size, cell, reach, ok) {
    let best = null;
    let distance = reach;
    for (const target of targets(others, axis, size)) {
        // The end on the target, then the middle.
        const options = [[target - start, start + length - target], [2 * (target - start), start + length / 2 - target]];
        for (const [candidate, off] of options) {
            if (Math.abs(off) > distance || candidate <= 0 || Math.abs(candidate / cell - Math.round(candidate / cell)) > 0.001)
                continue;
            if (ok(Math.round(candidate / cell) * cell)) {
                best = Math.round(candidate / cell) * cell;
                distance = Math.abs(off);
            }
        }
    }
    return best;
}

// The guides to draw for a widget at `r`: a line for each of its edges and
// middles that meets another widget's, from one to the other, and one
// across the whole screen for its middle lines.
function guides(r, others, width, height) {
    const found = [];
    const meets = (a, b) => Math.abs(a - b) < 0.5;
    for (const axis of [0, 1]) {
        const size = axis === 0 ? width : height;
        for (const line of lines(r, axis)) {
            if (meets(line, size / 2))
                found.push(axis === 0 ? { x: line, y: 0, length: height, axis } : { x: 0, y: line, length: width, axis });
            for (const other of others) {
                if (!lines(other, axis).some(theirs => meets(theirs, line)))
                    continue;
                if (axis === 0) {
                    const top = Math.min(r.y, other.y);
                    found.push({ x: line, y: top, length: Math.max(r.y + r.height, other.y + other.height) - top, axis });
                } else {
                    const left = Math.min(r.x, other.x);
                    found.push({ x: left, y: line, length: Math.max(r.x + r.width, other.x + other.width) - left, axis });
                }
            }
        }
    }
    return found;
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
