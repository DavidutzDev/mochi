// Times and dates as the clock's tabs write them. Not a library, so each
// view that imports it has Qt's Locale at hand.

function pad(value) {
    return `${value}`.padStart(2, "0");
}

// "19:30", or "7:30 PM" on a 12-hour clock.
function time(date, twelve) {
    const hours = date.getHours();
    if (!twelve)
        return `${pad(hours)}:${pad(date.getMinutes())}`;
    return `${hours % 12 === 0 ? 12 : hours % 12}:${pad(date.getMinutes())} ${hours < 12 ? "AM" : "PM"}`;
}

// The hour of the day as a clock in the forecast shows it: "14:00" or
// "2 PM".
function hour(value, twelve) {
    if (!twelve)
        return `${pad(value)}:00`;
    return `${value % 12 === 0 ? 12 : value % 12} ${value < 12 ? "AM" : "PM"}`;
}

// Midnight starting `date`'s day.
function day(date) {
    return new Date(date.getFullYear(), date.getMonth(), date.getDate());
}

// Whole days from `from`'s date to `to`'s.
function daysBetween(from, to) {
    return Math.round((day(to) - day(from)) / 86400000);
}

function sameDay(a, b) {
    return daysBetween(a, b) === 0;
}

// How far `date` is from `now`: "in 5 min", "in 3 h", "tomorrow",
// "in 4 days", or the same into the past.
function relative(date, now) {
    const minutes = Math.round((date - now) / 60000);
    const days = daysBetween(now, date);
    if (minutes === 0)
        return "now";
    if (minutes > 0) {
        if (minutes < 60)
            return `in ${minutes} min`;
        if (days === 0)
            return `in ${Math.round(minutes / 60)} h`;
        return days === 1 ? "tomorrow" : `in ${days} days`;
    }
    if (-minutes < 60)
        return `${-minutes} min ago`;
    if (days === 0)
        return `${Math.round(-minutes / 60)} h ago`;
    return days === -1 ? "yesterday" : `${-days} days ago`;
}

// "Wed 21 Oct", in the locale's words.
function shortDate(date) {
    const locale = Qt.locale();
    return `${locale.dayName(date.getDay(), Locale.ShortFormat)} ${date.getDate()} ${locale.monthName(date.getMonth(), Locale.ShortFormat)}`;
}

// "Wednesday 21 October".
function longDate(date) {
    const locale = Qt.locale();
    return `${locale.dayName(date.getDay(), Locale.LongFormat)} ${date.getDate()} ${locale.monthName(date.getMonth(), Locale.LongFormat)}`;
}

// "2026-10-21", as the clock module's actions take it.
function isoDate(date) {
    return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

// "18:30", "6:30 pm" or "6 PM" as [hours, minutes] on a 24-hour clock, or
// null. An hour alone needs AM or PM.
function parseTime(text) {
    const match = /^\s*(\d{1,2})(?::(\d{2}))?\s*([AaPp][Mm])?\s*$/.exec(text);
    if (!match || (match[2] === undefined && match[3] === undefined))
        return null;
    let hours = Number(match[1]);
    const minutes = Number(match[2] ?? 0);
    if (match[3] !== undefined) {
        if (hours < 1 || hours > 12)
            return null;
        hours = hours % 12 + (match[3].toLowerCase() === "pm" ? 12 : 0);
    }
    if (hours > 23 || minutes > 59)
        return null;
    return [hours, minutes];
}
