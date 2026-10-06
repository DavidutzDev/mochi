//! The screens' pixels, exactly. When the picker opens, every output is
//! copied once through `wlr-screencopy-unstable-v1` into a shared memory
//! file, on a Wayland connection of the module's own.
//!
//! The overlay shows the frozen screen through Quickshell, but what it
//! draws is scaled to logical pixels, so under fractional scaling it never
//! holds the screen's own bytes. These frames do: the overlay sends the
//! pixel under the pointer, in the frame's pixels, and the module answers
//! from here.

use std::fs::File;
use std::os::fd::AsFd;
use std::os::unix::fs::FileExt;
use std::path::Path;

use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_output::{self, Transform, WlOutput};
use wayland_client::protocol::wl_shm::{self, WlShm};
use wayland_client::protocol::{wl_buffer, wl_registry, wl_shm_pool};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, WEnum};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_frame_v1::{self, ZwlrScreencopyFrameV1},
    zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
};

use crate::color::Color;

/// One output's pixels, as the compositor drew them.
#[derive(Debug)]
pub struct Frame {
    /// The connector, like `DP-3`: what Quickshell calls the screen.
    pub output: String,
    /// The buffer's size. A rotated output's buffer is on its side.
    width: u32,
    height: u32,
    transform: Transform,
    /// Red, green, blue for each buffer pixel, row by row, top first.
    pixels: Vec<u8>,
}

impl Frame {
    /// The size as the screen shows it, in its own pixels.
    pub fn size(&self) -> (u32, u32) {
        if turned(self.transform) {
            (self.height, self.width)
        } else {
            (self.width, self.height)
        }
    }

    /// The color at `x`, `y`, counted as the screen shows it, or `None`
    /// off the screen.
    pub fn pixel(&self, x: i64, y: i64) -> Option<Color> {
        let (shown_width, shown_height) = self.size();
        if x < 0 || y < 0 || x >= i64::from(shown_width) || y >= i64::from(shown_height) {
            return None;
        }
        let (x, y) = to_buffer(self.transform, x as u32, y as u32, self.width, self.height);
        let at = (y as usize * self.width as usize + x as usize) * 3;
        let rgb = self.pixels.get(at..at + 3)?;
        Some(Color::rgb(rgb[0], rgb[1], rgb[2]))
    }
}

/// Whether the transform turns the buffer on its side.
fn turned(transform: Transform) -> bool {
    matches!(
        transform,
        Transform::_90 | Transform::_270 | Transform::Flipped90 | Transform::Flipped270
    )
}

/// Where a pixel the screen shows at `x`, `y` is in a `width` by `height`
/// buffer: this undoes the screen's transform. Transform 90 shows the
/// buffer turned a quarter clockwise; flipped-90 mirrors it left to right
/// and turns it the other way. Checked against grim on Sway.
fn to_buffer(transform: Transform, x: u32, y: u32, width: u32, height: u32) -> (u32, u32) {
    let (right, bottom) = (width - 1, height - 1);
    let (x, y) = match transform {
        Transform::_90 | Transform::Flipped270 => (y, bottom - x),
        Transform::_180 | Transform::Flipped180 => (right - x, bottom - y),
        Transform::_270 | Transform::Flipped90 => (right - y, x),
        _ => (x, y),
    };
    match transform {
        Transform::Flipped
        | Transform::Flipped90
        | Transform::Flipped180
        | Transform::Flipped270 => (right - x, y),
        _ => (x, y),
    }
}

/// Copies every output. `dir` holds the shared memory files for a moment.
/// Blocks until the compositor has copied them all, a frame or two.
pub fn capture(dir: &Path) -> Result<Vec<Frame>, String> {
    let connection = Connection::connect_to_env()
        .map_err(|error| format!("cannot connect to the Wayland display: {error}"))?;
    let (globals, mut queue) = registry_queue_init::<Client>(&connection)
        .map_err(|error| format!("cannot read the Wayland globals: {error}"))?;
    let handle = queue.handle();
    let shm: WlShm = globals
        .bind(&handle, 1..=1, ())
        .map_err(|_| "the compositor has no shared memory".to_owned())?;
    let manager: ZwlrScreencopyManagerV1 = globals
        .bind(&handle, 1..=3, ())
        .map_err(|_| "the compositor doesn't support wlr-screencopy".to_owned())?;
    let mut client = Client {
        shm,
        dir: dir.to_owned(),
        outputs: Vec::new(),
        shots: Vec::new(),
        version: manager.version(),
    };
    let registry = globals.registry();
    globals.contents().with_list(|list| {
        for global in list.iter().filter(|global| global.interface == "wl_output") {
            let index = client.outputs.len();
            let proxy: WlOutput = registry.bind(global.name, global.version.min(4), &handle, index);
            client.outputs.push(Output {
                proxy,
                name: String::new(),
                transform: Transform::Normal,
            });
        }
    });
    // Names and transforms.
    queue
        .roundtrip(&mut client)
        .map_err(|error| error.to_string())?;
    for (index, output) in client.outputs.iter().enumerate() {
        manager.capture_output(0, &output.proxy, &handle, index);
        client.shots.push(Shot::default());
    }
    while !client.shots.iter().all(Shot::finished) {
        queue
            .blocking_dispatch(&mut client)
            .map_err(|error| error.to_string())?;
    }
    let mut frames = Vec::new();
    for (output, shot) in client.outputs.iter().zip(&client.shots) {
        if let Some(error) = &shot.error {
            return Err(format!("cannot copy {}: {error}", output.name));
        }
        let (Some(file), Some(layout)) = (&shot.file, shot.layout) else {
            continue;
        };
        frames.push(Frame {
            output: output.name.clone(),
            width: layout.width,
            height: layout.height,
            transform: output.transform,
            pixels: read(file, layout, shot.y_invert)?,
        });
        if let Some(buffer) = &shot.buffer {
            buffer.destroy();
        }
    }
    Ok(frames)
}

/// The buffer as red, green, blue, top row first.
fn read(file: &File, layout: Layout, y_invert: bool) -> Result<Vec<u8>, String> {
    let size = pixel_size(layout.format)?;
    let mut bytes = vec![0; layout.stride as usize * layout.height as usize];
    file.read_exact_at(&mut bytes, 0)
        .map_err(|error| format!("cannot read the copy: {error}"))?;
    let mut pixels = Vec::with_capacity(layout.width as usize * layout.height as usize * 3);
    for row in 0..layout.height {
        let row = if y_invert {
            layout.height - 1 - row
        } else {
            row
        };
        let start = row as usize * layout.stride as usize;
        let line = bytes
            .get(start..start + layout.width as usize * size)
            .ok_or("the copy is shorter than its size")?;
        for pixel in line.chunks_exact(size) {
            pixels.extend_from_slice(&rgb(layout.format, pixel));
        }
    }
    Ok(pixels)
}

/// Bytes a pixel, for the formats Mochi reads.
fn pixel_size(format: wl_shm::Format) -> Result<usize, String> {
    use wl_shm::Format::*;
    match format {
        Argb8888 | Xrgb8888 | Abgr8888 | Xbgr8888 | Argb2101010 | Xrgb2101010 | Abgr2101010
        | Xbgr2101010 => Ok(4),
        Rgb888 | Bgr888 => Ok(3),
        other => Err(format!(
            "the screen is in a format Mochi can't read: {other:?}"
        )),
    }
}

/// One pixel, its bytes as they are in memory, as 8-bit red, green, blue.
/// The formats name their channels from the high bit of a little-endian
/// word.
fn rgb(format: wl_shm::Format, pixel: &[u8]) -> [u8; 3] {
    use wl_shm::Format::*;
    let mut word = [0; 4];
    word[..pixel.len()].copy_from_slice(pixel);
    let word = u32::from_le_bytes(word);
    let byte = |shift: u32| (word >> shift) as u8;
    // Ten bits a channel, back to eight.
    let ten = |shift: u32| ((((word >> shift) & 0x3ff) * 255 + 511) / 1023) as u8;
    match format {
        Abgr8888 | Xbgr8888 | Bgr888 => [byte(0), byte(8), byte(16)],
        Argb2101010 | Xrgb2101010 => [ten(20), ten(10), ten(0)],
        Abgr2101010 | Xbgr2101010 => [ten(0), ten(10), ten(20)],
        // Argb8888, Xrgb8888 and Rgb888.
        _ => [byte(16), byte(8), byte(0)],
    }
}

#[derive(Debug)]
struct Output {
    proxy: WlOutput,
    name: String,
    transform: Transform,
}

#[derive(Debug, Clone, Copy)]
struct Layout {
    format: wl_shm::Format,
    width: u32,
    height: u32,
    stride: u32,
}

/// One output's copy, as it goes.
#[derive(Debug, Default)]
struct Shot {
    /// The shared memory buffer the compositor offered.
    layout: Option<Layout>,
    file: Option<File>,
    buffer: Option<wl_buffer::WlBuffer>,
    y_invert: bool,
    ready: bool,
    error: Option<String>,
}

impl Shot {
    fn finished(&self) -> bool {
        self.ready || self.error.is_some()
    }
}

#[derive(Debug)]
struct Client {
    shm: WlShm,
    dir: std::path::PathBuf,
    outputs: Vec<Output>,
    shots: Vec<Shot>,
    /// The screencopy manager's version: from 3, the compositor says when
    /// it has offered every buffer.
    version: u32,
}

impl Client {
    /// Makes the buffer the compositor asked for and has it copied into.
    fn copy(
        &mut self,
        frame: &ZwlrScreencopyFrameV1,
        index: usize,
        handle: &QueueHandle<Self>,
    ) -> Result<(), String> {
        let shot = &mut self.shots[index];
        if shot.file.is_some() {
            return Ok(());
        }
        let layout = shot
            .layout
            .ok_or("the compositor offered no shared memory buffer")?;
        let size = layout.stride * layout.height;
        let path = self
            .dir
            .join(format!("copy-{}-{index}", std::process::id()));
        std::fs::create_dir_all(&self.dir).map_err(|error| error.to_string())?;
        let file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .map_err(|error| format!("cannot make {}: {error}", path.display()))?;
        // Only the descriptor is needed from here.
        let _ = std::fs::remove_file(&path);
        file.set_len(u64::from(size))
            .map_err(|error| error.to_string())?;
        let pool = self.shm.create_pool(file.as_fd(), size as i32, handle, ());
        let buffer = pool.create_buffer(
            0,
            layout.width as i32,
            layout.height as i32,
            layout.stride as i32,
            layout.format,
            handle,
            (),
        );
        pool.destroy();
        frame.copy(&buffer);
        shot.file = Some(file);
        shot.buffer = Some(buffer);
        Ok(())
    }
}

impl Dispatch<ZwlrScreencopyFrameV1, usize> for Client {
    fn event(
        client: &mut Self,
        frame: &ZwlrScreencopyFrameV1,
        event: zwlr_screencopy_frame_v1::Event,
        index: &usize,
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        let index = *index;
        let mut copy = false;
        match event {
            zwlr_screencopy_frame_v1::Event::Buffer {
                format: WEnum::Value(format),
                width,
                height,
                stride,
            } => {
                client.shots[index].layout = Some(Layout {
                    format,
                    width,
                    height,
                    stride,
                });
                // Before version 3 this is the only offer.
                copy = client.version < 3;
            }
            zwlr_screencopy_frame_v1::Event::BufferDone => copy = true,
            zwlr_screencopy_frame_v1::Event::Flags {
                flags: WEnum::Value(flags),
            } => {
                client.shots[index].y_invert =
                    flags.contains(zwlr_screencopy_frame_v1::Flags::YInvert);
            }
            zwlr_screencopy_frame_v1::Event::Ready { .. } => {
                client.shots[index].ready = true;
                frame.destroy();
            }
            zwlr_screencopy_frame_v1::Event::Failed => {
                client.shots[index].error = Some("the compositor failed to copy it".into());
                frame.destroy();
            }
            _ => {}
        }
        if copy && let Err(error) = client.copy(frame, index, handle) {
            client.shots[index].error = Some(error);
            frame.destroy();
        }
    }
}

impl Dispatch<WlOutput, usize> for Client {
    fn event(
        client: &mut Self,
        _: &WlOutput,
        event: wl_output::Event,
        index: &usize,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let output = &mut client.outputs[*index];
        match event {
            wl_output::Event::Name { name } => output.name = name,
            wl_output::Event::Geometry {
                transform: WEnum::Value(transform),
                ..
            } => output.transform = transform,
            _ => {}
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Client {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

/// Objects whose events don't matter here.
macro_rules! ignore {
    ($($interface:ty),*) => {
        $(impl Dispatch<$interface, ()> for Client {
            fn event(
                _: &mut Self,
                _: &$interface,
                _: <$interface as Proxy>::Event,
                _: &(),
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
            }
        })*
    };
}

ignore!(
    WlShm,
    wl_shm_pool::WlShmPool,
    wl_buffer::WlBuffer,
    ZwlrScreencopyManagerV1
);

#[cfg(test)]
impl Frame {
    /// A screen of one color, for tests elsewhere.
    pub fn solid(output: &str, width: u32, height: u32, color: Color) -> Self {
        Self {
            output: output.into(),
            width,
            height,
            transform: Transform::Normal,
            pixels: [color.r, color.g, color.b].repeat(width as usize * height as usize),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 3 by 2 buffer whose pixels are numbered by their red channel.
    fn frame(transform: Transform) -> Frame {
        let mut pixels = Vec::new();
        for number in 0..6 {
            pixels.extend_from_slice(&[number, 0, 0]);
        }
        Frame {
            output: "DP-1".into(),
            width: 3,
            height: 2,
            transform,
            pixels,
        }
    }

    /// The red channels, row by row, as the screen shows them.
    fn shown(frame: &Frame) -> Vec<Vec<u8>> {
        let (width, height) = frame.size();
        (0..i64::from(height))
            .map(|y| {
                (0..i64::from(width))
                    .map(|x| frame.pixel(x, y).unwrap().r)
                    .collect()
            })
            .collect()
    }

    #[test]
    fn undoes_the_output_transform() {
        // The buffer:
        //   0 1 2
        //   3 4 5
        assert_eq!(shown(&frame(Transform::Normal)), [[0, 1, 2], [3, 4, 5]]);
        assert_eq!(shown(&frame(Transform::_180)), [[5, 4, 3], [2, 1, 0]]);
        // A quarter turn clockwise.
        assert_eq!(shown(&frame(Transform::_90)), [[3, 0], [4, 1], [5, 2]]);
        assert_eq!(shown(&frame(Transform::_270)), [[2, 5], [1, 4], [0, 3]]);
        // Mirrored left to right first.
        assert_eq!(shown(&frame(Transform::Flipped)), [[2, 1, 0], [5, 4, 3]]);
        assert_eq!(
            shown(&frame(Transform::Flipped90)),
            [[0, 3], [1, 4], [2, 5]]
        );
        assert_eq!(frame(Transform::_90).size(), (2, 3));
        assert_eq!(frame(Transform::Normal).pixel(3, 0), None);
        assert_eq!(frame(Transform::Normal).pixel(0, -1), None);
    }

    #[test]
    fn reads_each_format() {
        let word = u32::to_le_bytes(0x80_1e_2e_3e);
        assert_eq!(rgb(wl_shm::Format::Xrgb8888, &word), [0x1e, 0x2e, 0x3e]);
        assert_eq!(rgb(wl_shm::Format::Abgr8888, &word), [0x3e, 0x2e, 0x1e]);
        // Three bytes: blue, green, red in memory for RGB888.
        assert_eq!(rgb(wl_shm::Format::Rgb888, &[3, 2, 1]), [1, 2, 3]);
        assert_eq!(rgb(wl_shm::Format::Bgr888, &[1, 2, 3]), [1, 2, 3]);
        // Full red, half green, no blue, in ten bits.
        let ten = u32::to_le_bytes((0x3ff << 20) | (0x200 << 10));
        assert_eq!(rgb(wl_shm::Format::Xrgb2101010, &ten), [255, 128, 0]);
        assert_eq!(rgb(wl_shm::Format::Xbgr2101010, &ten), [0, 128, 255]);
        assert_eq!(pixel_size(wl_shm::Format::Bgr888), Ok(3));
        assert!(pixel_size(wl_shm::Format::Rgb565).is_err());
    }

    #[test]
    fn reads_rows_and_flips_them() {
        let dir = std::env::temp_dir().join(format!("mochi-colors-copy-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("frame");
        // Two rows of one pixel each, padded to a stride of 8.
        let bytes = [
            0x03, 0x02, 0x01, 0xff, 0, 0, 0, 0, 0x06, 0x05, 0x04, 0xff, 0, 0, 0, 0,
        ];
        std::fs::write(&path, bytes).unwrap();
        let file = File::open(&path).unwrap();
        let layout = Layout {
            format: wl_shm::Format::Xrgb8888,
            width: 1,
            height: 2,
            stride: 8,
        };
        assert_eq!(read(&file, layout, false).unwrap(), [1, 2, 3, 4, 5, 6]);
        assert_eq!(read(&file, layout, true).unwrap(), [4, 5, 6, 1, 2, 3]);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
