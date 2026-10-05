.pragma library

// Seconds as m:ss.
function format(seconds) {
    const whole = Math.max(0, Math.floor(seconds ?? 0));
    const rest = whole % 60;
    return `${Math.floor(whole / 60)}:${rest < 10 ? "0" : ""}${rest}`;
}
