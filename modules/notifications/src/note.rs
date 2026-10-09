//! One notification as an app sent it, read from the arguments of `Notify`.
//!
//! Apps differ in what they send, so everything here is forgiving: an odd or
//! missing hint gives a neutral value instead of an error.

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use zbus::zvariant::{OwnedValue, Value};

use mochi_core::sound::Sound;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Urgency {
    Low,
    Normal,
    Critical,
}

impl Urgency {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::Critical => "critical",
        }
    }
}

/// The action that asks for a reply instead of a button: its label goes on
/// the Reply button, and the text typed goes back in `NotificationReplied`.
pub const REPLY: &str = "inline-reply";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub key: String,
    pub label: String,
}

/// A picture to show next to the text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Image {
    /// A file path, a `file://` URL or an icon name.
    Path(String),
    /// Raw pixels, already converted to RGBA with no row padding.
    Pixels {
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub id: u32,
    pub app: String,
    /// `app_icon`, or the `desktop-entry` hint when that is empty: a path or
    /// an icon name.
    pub icon: String,
    pub summary: String,
    pub body: String,
    /// In the order the app gave them, `default` included.
    pub actions: Vec<Action>,
    pub urgency: Urgency,
    /// How long the popup stays, when the app said. `None` leaves it to the
    /// settings.
    pub timeout: Option<Duration>,
    pub image: Option<Image>,
    /// Stays after one of its actions is invoked.
    pub resident: bool,
    /// Never goes to the history.
    pub transient: bool,
    /// What to play when it pops up.
    pub sound: Option<Sound>,
    pub received: SystemTime,
}

/// The arguments of `Notify`, minus `replaces_id`, which became `id`.
#[derive(Debug)]
pub struct Request {
    pub app_name: String,
    pub app_icon: String,
    pub summary: String,
    pub body: String,
    pub actions: Vec<String>,
    pub hints: HashMap<String, OwnedValue>,
    pub expire_timeout: i32,
}

impl Note {
    pub fn new(id: u32, request: Request, received: SystemTime) -> Self {
        let hint = |name: &str| request.hints.get(name).map(|value| &**value);
        let flag = |name: &str| hint(name).and_then(boolean).unwrap_or(false);

        let urgency = match hint("urgency").and_then(integer) {
            Some(0) => Urgency::Low,
            Some(2) => Urgency::Critical,
            _ => Urgency::Normal,
        };
        let icon = if request.app_icon.is_empty() {
            hint("desktop-entry").and_then(string).unwrap_or_default()
        } else {
            request.app_icon
        };
        // The spec renamed these over time; apps still send every spelling.
        let image = ["image-data", "image_data", "icon_data"]
            .into_iter()
            .find_map(|name| hint(name).and_then(pixels))
            .or_else(|| {
                ["image-path", "image_path"]
                    .into_iter()
                    .find_map(|name| hint(name).and_then(string))
                    .filter(|path| !path.is_empty())
                    .map(Image::Path)
            });
        // A file is the more exact of the two, when an app sends both.
        let sound = if flag("suppress-sound") {
            None
        } else {
            hint("sound-file")
                .and_then(string)
                .and_then(|file| Sound::file(&file))
                .or_else(|| {
                    hint("sound-name")
                        .and_then(string)
                        .and_then(|name| Sound::name(&name))
                })
        };

        Self {
            id,
            app: request.app_name,
            icon,
            summary: request.summary,
            body: request.body,
            actions: request
                .actions
                .as_chunks::<2>()
                .0
                .iter()
                .map(|[key, label]| Action {
                    key: key.clone(),
                    label: label.clone(),
                })
                .collect(),
            urgency,
            timeout: u64::try_from(request.expire_timeout)
                .ok()
                .filter(|&millis| millis > 0)
                .map(Duration::from_millis),
            image,
            resident: flag("resident"),
            transient: flag("transient"),
            sound,
            received,
        }
    }

    pub fn action(&self, key: &str) -> Option<&Action> {
        self.actions.iter().find(|action| action.key == key)
    }
}

/// Hints arrive wrapped in a variant.
fn inner<'a>(value: &'a Value<'a>) -> &'a Value<'a> {
    match value {
        Value::Value(boxed) => inner(boxed),
        other => other,
    }
}

fn string(value: &Value<'_>) -> Option<String> {
    match inner(value) {
        Value::Str(text) => Some(text.to_string()),
        _ => None,
    }
}

fn integer(value: &Value<'_>) -> Option<i64> {
    match *inner(value) {
        Value::U8(number) => Some(number.into()),
        Value::I32(number) => Some(number.into()),
        Value::U32(number) => Some(number.into()),
        Value::I64(number) => Some(number),
        _ => None,
    }
}

fn boolean(value: &Value<'_>) -> Option<bool> {
    match *inner(value) {
        Value::Bool(flag) => Some(flag),
        _ => integer(value).map(|number| number != 0),
    }
}

/// `(iiibiiay)`: width, height, rowstride, has alpha, bits per sample,
/// channels, data. Only 8-bit RGB and RGBA exist in practice.
fn pixels(value: &Value<'_>) -> Option<Image> {
    let Value::Structure(structure) = inner(value) else {
        return None;
    };
    let [width, height, rowstride, _alpha, bits, channels, data] = structure.fields() else {
        return None;
    };
    let number = |value: &Value<'_>| integer(value).and_then(|number| usize::try_from(number).ok());
    let (width, height, rowstride) = (number(width)?, number(height)?, number(rowstride)?);
    let (bits, channels) = (number(bits)?, number(channels)?);
    let Value::Array(data) = inner(data) else {
        return None;
    };
    let bytes: Vec<u8> = data
        .iter()
        .filter_map(|byte| match byte {
            Value::U8(byte) => Some(*byte),
            _ => None,
        })
        .collect();
    if bits != 8 || !(3..=4).contains(&channels) || width == 0 || height == 0 {
        return None;
    }
    // The last row may skip its padding.
    let needed = rowstride * (height - 1) + width * channels;
    if rowstride < width * channels || bytes.len() < needed {
        return None;
    }

    let mut rgba = Vec::with_capacity(width * height * 4);
    for row in 0..height {
        let start = row * rowstride;
        for pixel in bytes[start..start + width * channels].chunks_exact(channels) {
            rgba.extend_from_slice(&pixel[..3]);
            rgba.push(if channels == 4 { pixel[3] } else { u8::MAX });
        }
    }
    Some(Image::Pixels {
        width: u32::try_from(width).ok()?,
        height: u32::try_from(height).ok()?,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use zbus::zvariant::StructureBuilder;

    use super::*;

    fn owned(value: Value<'_>) -> OwnedValue {
        value.try_to_owned().unwrap()
    }

    fn request(hints: Vec<(&str, Value<'_>)>) -> Request {
        Request {
            app_name: "Discord".into(),
            app_icon: String::new(),
            summary: "Ada".into(),
            body: "hello".into(),
            actions: vec![
                "default".into(),
                "Open".into(),
                "reply".into(),
                "Reply".into(),
            ],
            hints: hints
                .into_iter()
                .map(|(name, value)| (name.to_owned(), owned(value)))
                .collect(),
            expire_timeout: -1,
        }
    }

    #[test]
    fn reads_the_arguments_and_hints() {
        let note = Note::new(
            7,
            request(vec![
                ("urgency", Value::U8(2)),
                ("desktop-entry", Value::from("discord")),
                ("resident", Value::from(true)),
                ("image-path", Value::from("/tmp/avatar.png")),
            ]),
            SystemTime::UNIX_EPOCH,
        );
        assert_eq!(note.urgency, Urgency::Critical);
        assert_eq!(note.icon, "discord");
        assert!(note.resident && !note.transient);
        assert_eq!(note.image, Some(Image::Path("/tmp/avatar.png".into())));
        assert_eq!(note.timeout, None);
        assert_eq!(note.action("reply").unwrap().label, "Reply");
        assert_eq!(note.actions.len(), 2);
    }

    #[test]
    fn odd_values_fall_back() {
        let mut odd = request(vec![("urgency", Value::from("high"))]);
        odd.actions.push("dangling".into());
        odd.expire_timeout = 0;
        let note = Note::new(1, odd, SystemTime::UNIX_EPOCH);
        assert_eq!(note.urgency, Urgency::Normal);
        assert_eq!(note.actions.len(), 2);
        assert_eq!(note.timeout, None);

        let mut timed = request(vec![]);
        timed.expire_timeout = 2500;
        let note = Note::new(1, timed, SystemTime::UNIX_EPOCH);
        assert_eq!(note.timeout, Some(Duration::from_millis(2500)));
    }

    #[test]
    fn reads_the_sound_hints() {
        let sound = |hints| Note::new(1, request(hints), SystemTime::UNIX_EPOCH).sound;
        assert_eq!(
            sound(vec![("sound-name", Value::from("message-new-instant"))]),
            Some(Sound::Name("message-new-instant".into()))
        );
        assert_eq!(
            sound(vec![
                ("sound-name", Value::from("message-new-instant")),
                ("sound-file", Value::from("/tmp/ding.oga")),
            ]),
            Some(Sound::File("/tmp/ding.oga".into()))
        );
        // A file that isn't one falls back to the name.
        assert_eq!(
            sound(vec![
                ("sound-name", Value::from("bell")),
                ("sound-file", Value::from("ding.oga")),
            ]),
            Some(Sound::Name("bell".into()))
        );
        assert_eq!(
            sound(vec![
                ("sound-name", Value::from("bell")),
                ("suppress-sound", Value::from(true)),
            ]),
            None
        );
        assert_eq!(sound(vec![]), None);
    }

    #[test]
    fn converts_padded_rgb_pixels_to_rgba() {
        // 2x2 RGB with a rowstride of 8: two bytes of padding per row.
        let data: Vec<u8> = vec![1, 2, 3, 4, 5, 6, 0, 0, 7, 8, 9, 10, 11, 12];
        let image = StructureBuilder::new()
            .add_field(2_i32)
            .add_field(2_i32)
            .add_field(8_i32)
            .add_field(false)
            .add_field(8_i32)
            .add_field(3_i32)
            .add_field(data)
            .build()
            .unwrap();
        let note = Note::new(
            1,
            request(vec![("image-data", Value::Structure(image))]),
            SystemTime::UNIX_EPOCH,
        );
        assert_eq!(
            note.image,
            Some(Image::Pixels {
                width: 2,
                height: 2,
                rgba: vec![1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255],
            })
        );
    }

    #[test]
    fn rejects_truncated_pixels() {
        let image = StructureBuilder::new()
            .add_field(4_i32)
            .add_field(4_i32)
            .add_field(16_i32)
            .add_field(true)
            .add_field(8_i32)
            .add_field(4_i32)
            .add_field(vec![0_u8; 10])
            .build()
            .unwrap();
        let note = Note::new(
            1,
            request(vec![("image-data", Value::Structure(image))]),
            SystemTime::UNIX_EPOCH,
        );
        assert_eq!(note.image, None);
    }
}
