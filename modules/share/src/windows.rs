//! The windows xdg-desktop-portal-hyprland offers, from its
//! `XDPH_WINDOW_SHARING_LIST`: each window as
//! `handle[HC>]class[HT>]title[HE>]address[HA>]`, the address being
//! Hyprland's window address in decimal.

use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    /// What the portal wants back: `window:<handle>`.
    pub handle: String,
    pub class: String,
    pub title: String,
    /// Hyprland's address, as `hyprctl clients` writes it: `0x…`. Empty when
    /// the portal didn't know it.
    pub address: String,
}

impl Window {
    pub fn to_json(&self) -> Value {
        json!({
            "handle": self.handle,
            "class": self.class,
            "title": self.title,
            "address": self.address,
        })
    }
}

pub fn parse(list: &str) -> Vec<Window> {
    list.split("[HA>]")
        .filter_map(|entry| {
            let (handle, rest) = entry.split_once("[HC>]")?;
            let (class, rest) = rest.split_once("[HT>]")?;
            let (title, address) = rest.split_once("[HE>]")?;
            let handle = handle.trim();
            handle.parse::<u32>().ok()?;
            let address = match address.trim().parse::<u64>() {
                Ok(0) | Err(_) => String::new(),
                Ok(address) => format!("0x{address:x}"),
            };
            Some(Window {
                handle: handle.to_owned(),
                class: class.trim().to_owned(),
                title: title.trim().to_owned(),
                address,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_portals_list() {
        let list = "1234[HC>]codium[HT>]main.rs - VSCodium[HE>]110459225861744[HA>]\
                    99[HC>]firefox[HT>]A  page [HE>]0[HA>]";
        let windows = parse(list);
        assert_eq!(windows.len(), 2);
        assert_eq!(
            windows[0],
            Window {
                handle: "1234".into(),
                class: "codium".into(),
                title: "main.rs - VSCodium".into(),
                address: "0x64764aeb6e70".into(),
            }
        );
        // No Hyprland address: no live thumbnail, but still pickable.
        assert_eq!(windows[1].address, "");
        assert_eq!(windows[1].title, "A  page");
    }

    #[test]
    fn skips_what_it_cant_read() {
        assert!(parse("").is_empty());
        assert!(parse("garbage[HA>]x[HC>]y[HT>]z[HE>]1[HA>]").is_empty());
    }
}
