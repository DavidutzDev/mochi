//! Cuts a picked area out of a frozen frame.
//!
//! The overlay works in logical pixels in the compositor's global layout,
//! the same space as the outputs' positions. Each output's frame is saved at
//! its buffer size, which is larger on a scaled output, so an area maps to
//! frame pixels through that output's own scale.

use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

/// An area in logical pixels, in the global layout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// The smallest area holding both.
    pub fn union(self, other: Self) -> Self {
        let left = self.x.min(other.x);
        let top = self.y.min(other.y);
        let right = (self.x + self.width).max(other.x + other.width);
        let bottom = (self.y + self.height).max(other.y + other.height);
        Self::new(left, top, right - left, bottom - top)
    }

    /// The area both cover, when they overlap.
    pub fn intersection(self, other: Self) -> Option<Self> {
        let left = self.x.max(other.x);
        let top = self.y.max(other.y);
        let right = (self.x + self.width).min(other.x + other.width);
        let bottom = (self.y + self.height).min(other.y + other.height);
        (right > left && bottom > top).then(|| Self::new(left, top, right - left, bottom - top))
    }

    pub fn intersects(&self, other: &Self) -> bool {
        self.intersection(*other).is_some()
    }
}

/// An area in frame pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pixels {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Maps `area` onto a frame of the output at `output`, saved at `size`
/// pixels. Edges round outwards, so a picked window keeps its border, and
/// the result is clipped to the frame. `None` when nothing of `area` is on
/// it.
pub fn to_pixels(area: Rect, output: Rect, size: (u32, u32)) -> Option<Pixels> {
    if output.width <= 0.0 || output.height <= 0.0 {
        return None;
    }
    let scale_x = f64::from(size.0) / output.width;
    let scale_y = f64::from(size.1) / output.height;
    // Shaves float noise off, so 1.25 × 100.8 stays 126 rather than 126.000001.
    let snap = |value: f64| (value * 1000.0).round() / 1000.0;
    let edge = |logical: f64, origin: f64, scale: f64, limit: u32, round: fn(f64) -> f64| {
        round(snap((logical - origin) * scale)).clamp(0.0, f64::from(limit)) as u32
    };
    let left = edge(area.x, output.x, scale_x, size.0, f64::floor);
    let top = edge(area.y, output.y, scale_y, size.1, f64::floor);
    let right = edge(area.x + area.width, output.x, scale_x, size.0, f64::ceil);
    let bottom = edge(area.y + area.height, output.y, scale_y, size.1, f64::ceil);
    (right > left && bottom > top).then_some(Pixels {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    })
}

/// Saves the part of the frame at `frame` under `area` as a PNG at `target`.
/// The overlay saves frames as binary PPM, which costs it next to nothing
/// to write, so the island never stalls; the compression happens here.
pub fn crop(frame: &Path, output: Rect, area: Rect, target: &Path) -> Result<Pixels, String> {
    let bytes = std::fs::read(frame).map_err(|error| format!("cannot read the frame: {error}"))?;
    let (width, height, pixels) = parse_ppm(&bytes).ok_or("the frame isn't a binary PPM")?;

    let cut = to_pixels(area, output, (width, height)).ok_or("the area is outside the screen")?;
    let line = width as usize * 3;
    let mut rows = Vec::with_capacity(cut.width as usize * cut.height as usize * 3);
    for row in cut.y..cut.y + cut.height {
        let start = row as usize * line + cut.x as usize * 3;
        rows.extend_from_slice(&pixels[start..start + cut.width as usize * 3]);
    }

    write_png(target, cut.width, cut.height, png::ColorType::Rgb, &rows)?;
    Ok(cut)
}

/// Saves `area` as one PNG at `target`, from the frames of every screen it
/// touches, each with its place in the global layout. The image takes the
/// finest scale among them, so no screen loses detail; a coarser screen's
/// part is scaled up to match. Where no screen is, as beside a shorter
/// monitor, the image is transparent. Returns its size in pixels.
pub fn join(frames: &[(PathBuf, Rect)], area: Rect, target: &Path) -> Result<(u32, u32), String> {
    let files = frames
        .iter()
        .map(|(path, place)| {
            std::fs::read(path)
                .map(|bytes| (bytes, *place))
                .map_err(|error| format!("cannot read the frame: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut screens = Vec::with_capacity(files.len());
    for (bytes, place) in &files {
        let (width, height, pixels) = parse_ppm(bytes).ok_or("a frame isn't a binary PPM")?;
        screens.push((width, height, pixels, *place));
    }
    let scale = screens
        .iter()
        .map(|(width, _, _, place)| f64::from(*width) / place.width)
        .fold(0.0, f64::max);
    if scale <= 0.0 {
        return Err("no frame to take the screenshot from".into());
    }
    let size = |logical: f64| (logical * scale).round().max(1.0) as u32;
    let (width, height) = (size(area.width), size(area.height));
    let mut canvas = vec![0u8; width as usize * height as usize * 4];

    for (frame_width, frame_height, pixels, place) in screens {
        let Some(part) = area.intersection(place) else {
            continue;
        };
        let Some(source) = to_pixels(part, place, (frame_width, frame_height)) else {
            continue;
        };
        // Where the part lands, in the image's pixels.
        let edge =
            |logical: f64, limit: u32| ((logical * scale).round().max(0.0) as u32).min(limit);
        let (left, right) = (
            edge(part.x - area.x, width),
            edge(part.x + part.width - area.x, width),
        );
        let (top, bottom) = (
            edge(part.y - area.y, height),
            edge(part.y + part.height - area.y, height),
        );
        if right <= left || bottom <= top {
            continue;
        }
        let line = frame_width as usize * 3;
        for y in top..bottom {
            let from_y = source.y
                + (u64::from(y - top) * u64::from(source.height) / u64::from(bottom - top)) as u32;
            for x in left..right {
                let from_x = source.x
                    + (u64::from(x - left) * u64::from(source.width) / u64::from(right - left))
                        as u32;
                let from = from_y as usize * line + from_x as usize * 3;
                let to = (y as usize * width as usize + x as usize) * 4;
                canvas[to..to + 3].copy_from_slice(&pixels[from..from + 3]);
                canvas[to + 3] = 255;
            }
        }
    }

    if canvas
        .as_chunks::<4>()
        .0
        .iter()
        .all(|pixel| pixel[3] == 255)
    {
        let rgb: Vec<u8> = canvas
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
            .collect();
        write_png(target, width, height, png::ColorType::Rgb, &rgb)?;
    } else {
        write_png(target, width, height, png::ColorType::Rgba, &canvas)?;
    }
    Ok((width, height))
}

fn write_png(
    target: &Path,
    width: u32,
    height: u32,
    color: png::ColorType,
    data: &[u8],
) -> Result<(), String> {
    let file = File::create(target)
        .map_err(|error| format!("cannot write {}: {error}", target.display()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(color);
    encoder.set_depth(png::BitDepth::Eight);
    // Screenshots are mostly flat areas, which the fast level squeezes
    // nearly as well, in a fraction of the time.
    encoder.set_compression(png::Compression::Fast);
    let mut writer = encoder
        .write_header()
        .map_err(|error| format!("cannot write the screenshot: {error}"))?;
    writer
        .write_image_data(data)
        .and_then(|()| writer.finish())
        .map_err(|error| format!("cannot write the screenshot: {error}"))
}

/// Reads a binary PPM (`P6`, 8 bits per channel): its size and RGB pixels.
fn parse_ppm(bytes: &[u8]) -> Option<(u32, u32, &[u8])> {
    let mut rest = bytes.strip_prefix(b"P6")?;
    // The header is four whitespace-separated fields; `#` starts a comment.
    let mut field = || -> Option<u32> {
        loop {
            match rest.first()? {
                b'#' => rest = &rest[rest.iter().position(|byte| *byte == b'\n')?..],
                byte if byte.is_ascii_whitespace() => rest = &rest[1..],
                _ => break,
            }
        }
        let digits = rest.iter().take_while(|byte| byte.is_ascii_digit()).count();
        let value = std::str::from_utf8(&rest[..digits]).ok()?.parse().ok()?;
        rest = &rest[digits..];
        Some(value)
    };
    let (width, height, depth) = (field()?, field()?, field()?);
    if depth != 255 {
        return None;
    }
    // Exactly one whitespace byte separates the header from the pixels.
    let pixels = rest.get(1..)?;
    let size = width as usize * height as usize * 3;
    Some((width, height, pixels.get(..size)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEFT: Rect = Rect {
        x: 0.0,
        y: 0.0,
        width: 1920.0,
        height: 1080.0,
    };

    #[test]
    fn maps_areas_on_an_unscaled_output() {
        let area = Rect::new(100.0, 50.0, 300.0, 200.0);
        assert_eq!(
            to_pixels(area, LEFT, (1920, 1080)),
            Some(Pixels {
                x: 100,
                y: 50,
                width: 300,
                height: 200
            })
        );
    }

    #[test]
    fn maps_areas_on_an_output_to_the_right() {
        let right = Rect::new(1920.0, 0.0, 1920.0, 1080.0);
        let area = Rect::new(1942.0, 62.0, 1876.0, 996.0);
        assert_eq!(
            to_pixels(area, right, (1920, 1080)),
            Some(Pixels {
                x: 22,
                y: 62,
                width: 1876,
                height: 996
            })
        );
    }

    #[test]
    fn scales_to_the_frame_rounding_outwards() {
        // A 4K panel at 1.5: 2560 × 1440 logical, 3840 × 2160 pixels.
        let output = Rect::new(0.0, 0.0, 2560.0, 1440.0);
        let area = Rect::new(10.0, 10.0, 101.0, 33.0);
        assert_eq!(
            to_pixels(area, output, (3840, 2160)),
            Some(Pixels {
                x: 15,
                y: 15,
                width: 152,
                height: 50
            })
        );

        // 1.25, where logical edges land between pixels.
        let output = Rect::new(0.0, 0.0, 1536.0, 864.0);
        let area = Rect::new(100.8, 0.0, 10.0, 10.0);
        let cut = to_pixels(area, output, (1920, 1080)).unwrap();
        assert_eq!((cut.x, cut.width), (126, 13));
    }

    #[test]
    fn clips_to_the_frame() {
        let area = Rect::new(-50.0, 1000.0, 100.0, 200.0);
        assert_eq!(
            to_pixels(area, LEFT, (1920, 1080)),
            Some(Pixels {
                x: 0,
                y: 1000,
                width: 50,
                height: 80
            })
        );
        let elsewhere = Rect::new(2000.0, 0.0, 10.0, 10.0);
        assert_eq!(to_pixels(elsewhere, LEFT, (1920, 1080)), None);
        assert_eq!(
            to_pixels(Rect::new(5.0, 5.0, 0.0, 9.0), LEFT, (1920, 1080)),
            None
        );
    }

    #[test]
    fn reads_binary_ppm() {
        let ppm = b"P6\n# made by Qt\n2 1\n255\n\x01\x02\x03\x04\x05\x06";
        assert_eq!(
            parse_ppm(ppm),
            Some((2, 1, &b"\x01\x02\x03\x04\x05\x06"[..]))
        );
        // Too short, the wrong kind, or 16 bits.
        assert_eq!(parse_ppm(b"P6 2 1 255\n\x01"), None);
        assert_eq!(parse_ppm(b"P3 1 1 255\n1 2 3"), None);
        assert_eq!(parse_ppm(b"P6 1 1 65535\n\x00\x00\x00\x00\x00\x00"), None);
    }

    #[test]
    fn joins_screens_at_the_finest_scale() {
        let dir = std::env::temp_dir().join(format!("mochi-join-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Left: 2 × 1 logical at scale 1, pixels red 10 and 20. Right: 1 × 2
        // logical at scale 2, so 2 × 4 pixels, all red 30.
        let left = dir.join("left.ppm");
        let mut ppm = b"P6\n2 1\n255\n".to_vec();
        ppm.extend([10, 0, 0, 20, 0, 0]);
        std::fs::write(&left, ppm).unwrap();
        let right = dir.join("right.ppm");
        let mut ppm = b"P6\n2 4\n255\n".to_vec();
        ppm.extend([30u8, 0, 0].repeat(8));
        std::fs::write(&right, ppm).unwrap();

        let target = dir.join("joined.png");
        let frames = [
            (left, Rect::new(0.0, 0.0, 2.0, 1.0)),
            (right, Rect::new(2.0, 0.0, 1.0, 2.0)),
        ];
        let size = join(&frames, Rect::new(0.0, 0.0, 3.0, 2.0), &target).unwrap();
        assert_eq!(size, (6, 4));

        let file = std::io::BufReader::new(File::open(&target).unwrap());
        let mut reader = png::Decoder::new(file).read_info().unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut pixels).unwrap();
        assert_eq!(info.color_type, png::ColorType::Rgba);
        let pixel = |x: usize, y: usize| {
            let at = (y * 6 + x) * 4;
            (pixels[at], pixels[at + 3])
        };
        // The left screen doubled, then the right one.
        assert_eq!(pixel(0, 0), (10, 255));
        assert_eq!(pixel(1, 1), (10, 255));
        assert_eq!(pixel(2, 0), (20, 255));
        assert_eq!(pixel(4, 3), (30, 255));
        // Under the shorter left screen: nothing.
        assert_eq!(pixel(0, 3).1, 0);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn crops_a_frame_into_a_png() {
        let dir = std::env::temp_dir().join(format!("mochi-crop-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let frame = dir.join("frame.ppm");
        let target = dir.join("cut.png");

        // 4 × 2 pixels, each one's red channel its index.
        let mut ppm = b"P6\n4 2\n255\n".to_vec();
        ppm.extend((0u8..8).flat_map(|index| [index, 0, 0]));
        std::fs::write(&frame, ppm).unwrap();

        // A 2 × 1 logical output saved at 2×: the right half is pixels 2, 3,
        // 6 and 7.
        let output = Rect::new(10.0, 0.0, 2.0, 1.0);
        let cut = crop(&frame, output, Rect::new(11.0, 0.0, 1.0, 1.0), &target).unwrap();
        assert_eq!((cut.width, cut.height), (2, 2));

        let file = std::io::BufReader::new(File::open(&target).unwrap());
        let mut reader = png::Decoder::new(file).read_info().unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut pixels).unwrap();
        assert_eq!((info.width, info.height), (2, 2));
        let reds: Vec<u8> = pixels
            .as_chunks::<3>()
            .0
            .iter()
            .map(|pixel| pixel[0])
            .collect();
        assert_eq!(reds, [2, 3, 6, 7]);

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
