//! The history and the pins together. Both are [`Store`]s: the history
//! where the `storage` setting says, the pins always on disk, so they last
//! across logouts. Pinning moves an entry from one to the other under the
//! same id, and the two never give out the same id, so the views and the
//! actions only ever deal with ids.
//!
//! Clearing the history and its limits leave the pins alone. Copying
//! something that is pinned keeps it out of the history.

use std::io;

use zeroize::Zeroizing;

use crate::store::{Clip, Entry, Store};

#[derive(Debug)]
pub struct History {
    recent: Store,
    /// None until something is pinned, or a pins file exists.
    pins: Option<Store>,
}

impl History {
    pub fn new(recent: Store) -> Self {
        Self { recent, pins: None }
    }

    /// Takes the pins, opened elsewhere since that may need the keyring.
    pub fn set_pins(&mut self, mut pins: Store) {
        self.recent.reserve(pins.next_id());
        pins.reserve(self.recent.next_id());
        self.pins = Some(pins);
    }

    pub fn has_pins(&self) -> bool {
        self.pins.is_some()
    }

    /// The history, newest first, without the pins.
    pub fn recent(&self) -> &[Entry] {
        self.recent.entries()
    }

    /// The pins, the latest pinned first.
    pub fn pinned(&self) -> &[Entry] {
        self.pins.as_ref().map_or(&[], Store::entries)
    }

    pub fn is_pinned(&self, id: u64) -> bool {
        self.pins
            .as_ref()
            .is_some_and(|pins| pins.get(id).is_some())
    }

    pub fn get(&self, id: u64) -> Option<&Entry> {
        self.recent.get(id).or_else(|| self.pins.as_ref()?.get(id))
    }

    /// Every format of an entry, with its bytes.
    pub fn content(&mut self, id: u64) -> io::Result<Vec<(String, Zeroizing<Vec<u8>>)>> {
        match &mut self.pins {
            Some(pins) if pins.get(id).is_some() => pins.content(id),
            _ => self.recent.content(id),
        }
    }

    /// Keeps a copy and returns its id. A copy of something pinned isn't
    /// kept twice: it's the pin's id.
    pub fn add(&mut self, clip: &Clip, time: u64) -> io::Result<u64> {
        if let Some(id) = self.pins.as_ref().and_then(|pins| pins.find(clip)) {
            return Ok(id);
        }
        let id = self.recent.add(clip, time)?;
        if let Some(pins) = &mut self.pins {
            pins.reserve(self.recent.next_id());
        }
        Ok(id)
    }

    /// Moves a history entry to the top. Pins keep their order.
    pub fn touch(&mut self, id: u64, time: u64) -> io::Result<()> {
        if self.recent.get(id).is_some() {
            self.recent.touch(id, time)?;
        }
        Ok(())
    }

    /// Moves an entry into the pins, at their top. Pinning a pin does
    /// nothing.
    pub fn pin(&mut self, id: u64, time: u64) -> io::Result<()> {
        let Some(pins) = &mut self.pins else {
            return Err(io::Error::other("the pins aren't open"));
        };
        if pins.get(id).is_some() {
            return Ok(());
        }
        let kind = self
            .recent
            .get(id)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no such entry"))?
            .kind;
        let formats = self.recent.content(id)?;
        pins.keep(&Clip { kind, formats }, time, id)?;
        self.recent.remove(id)
    }

    /// Moves a pin back into the history, at its top.
    pub fn unpin(&mut self, id: u64, time: u64) -> io::Result<()> {
        let Some(pins) = &mut self.pins else {
            return Ok(());
        };
        let Some(kind) = pins.get(id).map(|entry| entry.kind) else {
            return Ok(());
        };
        let formats = pins.content(id)?;
        self.recent.keep(&Clip { kind, formats }, time, id)?;
        pins.remove(id)
    }

    /// Removes an entry, pinned or not.
    pub fn remove(&mut self, id: u64) -> io::Result<()> {
        match &mut self.pins {
            Some(pins) if pins.get(id).is_some() => pins.remove(id),
            _ => self.recent.remove(id),
        }
    }

    /// Removes everything but the pins.
    pub fn clear(&mut self) -> io::Result<()> {
        self.recent.clear()
    }

    /// Applies the history's limits; see [`Store::expire`]. Pins don't
    /// count toward them.
    pub fn expire(&mut self, max_entries: usize, max_age: u64, now: u64) -> io::Result<Vec<u64>> {
        self.recent.expire(max_entries, max_age, now)
    }

    pub fn tidy(&mut self) -> io::Result<()> {
        self.recent.tidy()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;
    use crate::store::{Key, Kind};

    struct Dir(PathBuf);

    impl Dir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "mochi-clipboard-history-{name}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&dir);
            Self(dir)
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn text(text: &str) -> Clip {
        Clip {
            kind: Kind::Text,
            formats: vec![(
                "text/plain;charset=utf-8".into(),
                Zeroizing::new(text.as_bytes().to_vec()),
            )],
        }
    }

    fn key() -> Option<Key> {
        Some(Zeroizing::new([9; 32]))
    }

    /// The history in memory, the pins encrypted, as in memory mode.
    fn open(dir: &Dir) -> History {
        let mut history = History::new(Store::open(&dir.0.join("history"), None).unwrap());
        history.set_pins(Store::open(&dir.0.join("pins"), key()).unwrap());
        history
    }

    fn previews(entries: &[Entry]) -> Vec<&str> {
        entries.iter().map(|entry| entry.preview.as_str()).collect()
    }

    #[test]
    fn pins_survive_clearing_the_limit_and_a_restart() {
        let dir = Dir::new("pins");
        let mut history = open(&dir);
        let token = history.add(&text("api token"), 10).unwrap();
        history.add(&text("one"), 20).unwrap();
        history.pin(token, 30).unwrap();
        assert_eq!(previews(history.pinned()), ["api token"]);
        assert_eq!(previews(history.recent()), ["one"]);

        // The limit counts only the history.
        history.add(&text("two"), 40).unwrap();
        history.add(&text("three"), 50).unwrap();
        assert_eq!(history.expire(1, 0, 50).unwrap().len(), 2);
        assert_eq!(previews(history.recent()), ["three"]);
        assert_eq!(previews(history.pinned()), ["api token"]);

        history.clear().unwrap();
        assert!(history.recent().is_empty());
        assert_eq!(previews(history.pinned()), ["api token"]);
        drop(history);

        // The memory history goes at logout; the pins stay, and their ids
        // aren't given out again.
        fs::remove_file(dir.0.join("history")).unwrap();
        let mut history = open(&dir);
        assert!(history.recent().is_empty());
        assert_eq!(previews(history.pinned()), ["api token"]);
        assert_eq!(
            history.content(token).unwrap()[0].1.as_slice(),
            b"api token"
        );
        let new = history.add(&text("after"), 60).unwrap();
        assert_ne!(new, token);

        // Encrypted like the disk history: the text isn't in the file.
        let raw = fs::read(dir.0.join("pins")).unwrap();
        assert!(!raw.windows(5).any(|window| window == b"token"));
    }

    #[test]
    fn unpinning_puts_it_back_on_top_and_copies_of_pins_stay_out() {
        let dir = Dir::new("unpin");
        let mut history = open(&dir);
        let pinned = history.add(&text("pinned"), 10).unwrap();
        history.add(&text("other"), 20).unwrap();
        history.pin(pinned, 30).unwrap();

        // Copying it again doesn't put it in the history twice.
        assert_eq!(history.add(&text("pinned"), 40).unwrap(), pinned);
        assert_eq!(previews(history.recent()), ["other"]);

        history.unpin(pinned, 50).unwrap();
        assert!(history.pinned().is_empty());
        assert_eq!(previews(history.recent()), ["pinned", "other"]);
        assert!(!history.is_pinned(pinned));
        assert_eq!(history.content(pinned).unwrap()[0].1.as_slice(), b"pinned");

        history.pin(pinned, 60).unwrap();
        history.remove(pinned).unwrap();
        assert!(history.pinned().is_empty());
        assert!(history.get(pinned).is_none());
    }
}
