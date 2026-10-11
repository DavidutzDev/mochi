//! The face the lock draws itself: the clock and the date, a field with a
//! dot for each character typed, and what went wrong. It's what shows when
//! nothing else can, so it needs nothing but this process: no Quickshell,
//! no daemon, and no font even, which only leaves the text out.

use std::time::Instant;

use ab_glyph::{Font, FontVec, PxScale, ScaleFont, point};
use tiny_skia::{
    Color, FillRule, Paint, PathBuilder, Pixmap, PremultipliedColorU8, Rect, Stroke, Transform,
};

use crate::config::{Config, rgba};
use crate::state::{Phase, State};

/// Colors as the face draws them.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub background: [u8; 4],
    pub raised: [u8; 4],
    pub foreground: [u8; 4],
    pub muted: [u8; 4],
    pub accent: [u8; 4],
    pub danger: [u8; 4],
}

impl Palette {
    pub fn from_config(config: &Config) -> Self {
        let colors = &config.colors;
        let mut background = rgba(&colors.background, [0, 0, 0, 255]);
        // Never see-through: nothing of the desktop shows on a lock screen.
        background[3] = 255;
        Self {
            background,
            raised: rgba(&colors.raised, [44, 44, 46, 255]),
            foreground: rgba(&colors.foreground, [255, 255, 255, 255]),
            muted: rgba(&colors.muted, [142, 142, 147, 255]),
            accent: rgba(&colors.accent, [255, 159, 10, 255]),
            danger: rgba(&colors.danger, [255, 69, 58, 255]),
        }
    }
}

/// A monitor in pixels, and its scale, which the face's sizes follow.
#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub width: u32,
    pub height: u32,
    pub scale: f32,
}

/// What the face shows, besides the state.
#[derive(Debug, Clone)]
pub struct Clock {
    pub time: String,
    pub date: String,
}

pub struct Face {
    pub palette: Palette,
    font: Option<FontVec>,
}

impl std::fmt::Debug for Face {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Face")
            .field("font", &self.font.is_some())
            .finish_non_exhaustive()
    }
}

impl Face {
    pub fn new(config: &Config) -> Self {
        Self {
            palette: Palette::from_config(config),
            font: config
                .font_bytes()
                .and_then(|bytes| FontVec::try_from_vec(bytes).ok()),
        }
    }

    /// What's under everything on a monitor of `size`: the theme's
    /// background, and over it the blurred image, filling the monitor and
    /// dimmed as the lock screen dims it. Scaling the image is the slow
    /// part of drawing, so the lock makes this once and keeps it.
    pub fn scene(&self, size: Size, backdrop: Option<&Pixmap>) -> Option<Pixmap> {
        let mut pixmap = Pixmap::new(size.width.max(1), size.height.max(1))?;
        pixmap.fill(color(self.palette.background));
        let Some(image) = backdrop else {
            return Some(pixmap);
        };
        let (width, height) = (pixmap.width() as f32, pixmap.height() as f32);
        let scale = (width / image.width() as f32).max(height / image.height() as f32);
        let x = (width - image.width() as f32 * scale) / 2.0;
        let y = (height - image.height() as f32 * scale) / 2.0;
        let smooth = tiny_skia::PixmapPaint {
            quality: tiny_skia::FilterQuality::Bilinear,
            ..tiny_skia::PixmapPaint::default()
        };
        pixmap.draw_pixmap(
            0,
            0,
            image.as_ref(),
            &smooth,
            Transform::from_row(scale, 0.0, 0.0, scale, x, y),
            None,
        );
        // As LockScreen.qml dims it.
        let mut dim = self.palette.background;
        dim[3] = (255.0 * DIM).round() as u8;
        if let Some(rect) = Rect::from_xywh(0.0, 0.0, width, height) {
            pixmap.fill_rect(rect, &paint(dim), Transform::identity(), None);
        }
        Some(pixmap)
    }

    /// A scene in BGRA bytes, as `wl_shm` wants them: what shows while the
    /// renderer gets ready, copied as it is.
    pub fn bgra(scene: &Pixmap) -> Vec<u8> {
        let mut out = vec![0; scene.data().len()];
        to_bgra(scene, &mut out);
        out
    }

    /// Draws a monitor of `size` into BGRA bytes, as `wl_shm`'s ARGB8888
    /// wants them.
    pub fn draw(
        &self,
        size: Size,
        state: &State,
        clock: &Clock,
        now: Instant,
        scene: Option<&Pixmap>,
        out: &mut [u8],
    ) {
        let Size {
            width,
            height,
            scale,
        } = size;
        let scene = scene.filter(|scene| (scene.width(), scene.height()) == (width, height));
        let Some(mut pixmap) = scene.cloned().or_else(|| self.scene(size, None)) else {
            return;
        };
        let palette = self.palette;
        let (width, height) = (width as f32, height as f32);
        let center = width / 2.0;

        // The clock, big, and the date under it, above the middle.
        let clock_size = (height * 0.14).clamp(48.0 * scale, 160.0 * scale);
        let clock_baseline = height * 0.42;
        self.text(
            &mut pixmap,
            &clock.time,
            clock_size,
            center,
            clock_baseline,
            palette.foreground,
        );
        self.text(
            &mut pixmap,
            &clock.date,
            18.0 * scale,
            center,
            clock_baseline + 36.0 * scale,
            palette.muted,
        );

        // The field: a pill with the dots, or what's going on.
        let field_width = 300.0 * scale;
        let field_height = 48.0 * scale;
        let field_top = clock_baseline + 72.0 * scale;
        let field = Rect::from_xywh(
            center - field_width / 2.0,
            field_top,
            field_width,
            field_height,
        );
        let checking = state.phase == Phase::Checking;
        if let Some(field) = field {
            fill_rounded(&mut pixmap, field, field_height / 2.0, palette.raised);
            if state.message.is_some() && state.typed() == 0 && !checking {
                stroke_rounded(
                    &mut pixmap,
                    field,
                    field_height / 2.0,
                    palette.danger,
                    1.5 * scale,
                );
            }
            lock_icon(
                &mut pixmap,
                field.left() + 26.0 * scale,
                field.top() + field_height / 2.0,
                scale,
                palette.muted,
            );
        }
        let middle = field_top + field_height / 2.0;
        let typed = state.typed();
        if typed > 0 && !checking {
            // As many dots as fit; past that, the field stays full.
            let step = 16.0 * scale;
            let room = ((field_width - 96.0 * scale) / step).floor().max(1.0) as usize;
            let shown = typed.min(room);
            let start = center - (shown as f32 - 1.0) * step / 2.0;
            for index in 0..shown {
                dot(
                    &mut pixmap,
                    start + index as f32 * step,
                    middle,
                    4.5 * scale,
                    palette.foreground,
                );
            }
        } else {
            let hint = match state.phase {
                Phase::Checking => "Checking…".to_owned(),
                Phase::Waiting(until) => {
                    let left = until.saturating_duration_since(now).as_secs() + 1;
                    format!("Try again in {left} s")
                }
                Phase::Typing => "Type your password".to_owned(),
            };
            self.text(
                &mut pixmap,
                &hint,
                15.0 * scale,
                center,
                middle + 5.0 * scale,
                palette.muted,
            );
        }

        // What went wrong, or Caps Lock.
        let below = field_top + field_height + 30.0 * scale;
        if let Some(message) = &state.message {
            self.text(
                &mut pixmap,
                message,
                14.0 * scale,
                center,
                below,
                palette.danger,
            );
        } else if state.caps_lock {
            self.text(
                &mut pixmap,
                "Caps Lock is on",
                14.0 * scale,
                center,
                below,
                palette.accent,
            );
        }

        to_bgra(&pixmap, out);
    }

    /// Draws `text` centred on `x`, its baseline at `y`.
    fn text(&self, pixmap: &mut Pixmap, text: &str, size: f32, x: f32, y: f32, rgba: [u8; 4]) {
        let Some(font) = &self.font else {
            return;
        };
        let scaled = font.as_scaled(PxScale::from(size));
        let mut glyphs = Vec::new();
        let mut caret = 0.0;
        let mut previous = None;
        for character in text.chars() {
            let id = scaled.glyph_id(character);
            if let Some(previous) = previous {
                caret += scaled.kern(previous, id);
            }
            glyphs.push(id.with_scale_and_position(size, point(caret, 0.0)));
            caret += scaled.h_advance(id);
            previous = Some(id);
        }
        let left = x - caret / 2.0;
        let (width, height) = (pixmap.width() as i32, pixmap.height() as i32);
        let pixels = pixmap.pixels_mut();
        for mut glyph in glyphs {
            glyph.position = point(left + glyph.position.x, y);
            let Some(outline) = font.outline_glyph(glyph) else {
                continue;
            };
            let bounds = outline.px_bounds();
            outline.draw(|gx, gy, coverage| {
                let px = bounds.min.x as i32 + gx as i32;
                let py = bounds.min.y as i32 + gy as i32;
                if px < 0 || py < 0 || px >= width || py >= height {
                    return;
                }
                let index = (py * width + px) as usize;
                pixels[index] = blend(pixels[index], rgba, coverage);
            });
        }
    }
}

fn color(rgba: [u8; 4]) -> Color {
    Color::from_rgba8(rgba[0], rgba[1], rgba[2], rgba[3])
}

/// How much the background's color darkens the image behind the lock
/// screen, as in LockScreen.qml.
const DIM: f32 = 0.45;

fn paint(rgba: [u8; 4]) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color(color(rgba));
    paint.anti_alias = true;
    paint
}

/// `rgba` over `under`, `coverage` of it.
fn blend(under: PremultipliedColorU8, rgba: [u8; 4], coverage: f32) -> PremultipliedColorU8 {
    let alpha = coverage.clamp(0.0, 1.0) * f32::from(rgba[3]) / 255.0;
    let mix = |over: u8, under: u8| {
        (f32::from(over) * alpha + f32::from(under) * (1.0 - alpha)).round() as u8
    };
    let out_alpha = (alpha * 255.0 + f32::from(under.alpha()) * (1.0 - alpha)).round() as u8;
    PremultipliedColorU8::from_rgba(
        mix(rgba[0], under.red()).min(out_alpha),
        mix(rgba[1], under.green()).min(out_alpha),
        mix(rgba[2], under.blue()).min(out_alpha),
        out_alpha,
    )
    .unwrap_or(under)
}

fn rounded(rect: Rect, radius: f32) -> Option<tiny_skia::Path> {
    let radius = radius.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let (left, top, right, bottom) = (rect.left(), rect.top(), rect.right(), rect.bottom());
    // A quarter circle as a cubic: the handles at 0.5523 of the radius.
    let k = radius * 0.552_284_8;
    let mut path = PathBuilder::new();
    path.move_to(left + radius, top);
    path.line_to(right - radius, top);
    path.cubic_to(
        right - radius + k,
        top,
        right,
        top + radius - k,
        right,
        top + radius,
    );
    path.line_to(right, bottom - radius);
    path.cubic_to(
        right,
        bottom - radius + k,
        right - radius + k,
        bottom,
        right - radius,
        bottom,
    );
    path.line_to(left + radius, bottom);
    path.cubic_to(
        left + radius - k,
        bottom,
        left,
        bottom - radius + k,
        left,
        bottom - radius,
    );
    path.line_to(left, top + radius);
    path.cubic_to(
        left,
        top + radius - k,
        left + radius - k,
        top,
        left + radius,
        top,
    );
    path.close();
    path.finish()
}

fn fill_rounded(pixmap: &mut Pixmap, rect: Rect, radius: f32, rgba: [u8; 4]) {
    if let Some(path) = rounded(rect, radius) {
        pixmap.fill_path(
            &path,
            &paint(rgba),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn stroke_rounded(pixmap: &mut Pixmap, rect: Rect, radius: f32, rgba: [u8; 4], width: f32) {
    if let Some(path) = rounded(rect, radius) {
        let stroke = Stroke {
            width,
            ..Stroke::default()
        };
        pixmap.stroke_path(&path, &paint(rgba), &stroke, Transform::identity(), None);
    }
}

fn dot(pixmap: &mut Pixmap, x: f32, y: f32, radius: f32, rgba: [u8; 4]) {
    if let Some(path) = PathBuilder::from_circle(x, y, radius) {
        pixmap.fill_path(
            &path,
            &paint(rgba),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

/// A padlock, centred on (x, y): a rounded body and its shackle.
fn lock_icon(pixmap: &mut Pixmap, x: f32, y: f32, scale: f32, rgba: [u8; 4]) {
    let body = Rect::from_xywh(x - 7.0 * scale, y - 1.0 * scale, 14.0 * scale, 10.0 * scale);
    if let Some(body) = body {
        fill_rounded(pixmap, body, 2.5 * scale, rgba);
    }
    let mut shackle = PathBuilder::new();
    let (left, right, top, foot) = (
        x - 4.5 * scale,
        x + 4.5 * scale,
        y - 9.0 * scale,
        y - 1.0 * scale,
    );
    shackle.move_to(left, foot);
    shackle.line_to(left, top + 4.5 * scale);
    shackle.cubic_to(
        left,
        top - 1.5 * scale,
        right,
        top - 1.5 * scale,
        right,
        top + 4.5 * scale,
    );
    shackle.line_to(right, foot);
    if let Some(path) = shackle.finish() {
        let stroke = Stroke {
            width: 2.0 * scale,
            ..Stroke::default()
        };
        pixmap.stroke_path(&path, &paint(rgba), &stroke, Transform::identity(), None);
    }
}

/// tiny-skia's premultiplied RGBA to the BGRA bytes of ARGB8888.
fn to_bgra(pixmap: &Pixmap, out: &mut [u8]) {
    for (from, to) in pixmap
        .data()
        .as_chunks::<4>()
        .0
        .iter()
        .zip(out.as_chunks_mut::<4>().0)
    {
        to[0] = from[2];
        to[1] = from[1];
        to[2] = from[0];
        to[3] = from[3];
    }
}

/// The time and the date now, in the system's locale.
pub fn clock(hours: &str) -> Clock {
    let format = |pattern: &str| -> String {
        // SAFETY: localtime_r fills `tm` from `now`; strftime writes at most
        // the buffer's length and returns how much it wrote.
        unsafe {
            let now = libc::time(std::ptr::null_mut());
            let mut tm: libc::tm = std::mem::zeroed();
            if libc::localtime_r(&raw const now, &raw mut tm).is_null() {
                return String::new();
            }
            let Ok(pattern) = std::ffi::CString::new(pattern) else {
                return String::new();
            };
            let mut buffer = [0u8; 128];
            let written = libc::strftime(
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                pattern.as_ptr(),
                &raw const tm,
            );
            String::from_utf8_lossy(&buffer[..written]).into_owned()
        }
    };
    let time = if hours == "12" {
        format("%I:%M").trim_start_matches('0').to_owned()
    } else {
        format("%H:%M")
    };
    Clock {
        time,
        date: format("%A %d %B")
            .split(' ')
            .map(|word| word.trim_start_matches('0'))
            .collect::<Vec<_>>()
            .join(" "),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Key;

    fn drawn(face: &Face, state: &State) -> Vec<u8> {
        let mut out = vec![0; 400 * 300 * 4];
        let clock = Clock {
            time: "12:34".into(),
            date: "Sunday 11 October".into(),
        };
        let size = Size {
            width: 400,
            height: 300,
            scale: 1.0,
        };
        face.draw(size, state, &clock, Instant::now(), None, &mut out);
        out
    }

    #[test]
    fn opaque_and_the_dots_follow_what_is_typed() {
        let mut config = Config::default();
        config.colors.background = "#00112233".into();
        let face = Face::new(&config);
        let mut state = State::default();
        let empty = drawn(&face, &state);
        assert!(
            empty.as_chunks::<4>().0.iter().all(|pixel| pixel[3] == 255),
            "never see-through"
        );
        state.key(Key::Text("a".into()), Instant::now());
        let one = drawn(&face, &state);
        state.key(Key::Text("b".into()), Instant::now());
        let two = drawn(&face, &state);
        assert_ne!(one, two);
        assert_ne!(empty, one);
    }

    #[test]
    fn the_clock_reads() {
        let clock = clock("24");
        assert_eq!(clock.time.len(), 5, "{}", clock.time);
        assert!(!clock.date.is_empty());
    }
}
