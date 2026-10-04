//! The history file: an append-only log, read once at startup into an index
//! in memory.
//!
//! Each record has two sealed parts: the entry's info (time, kind, hash,
//! formats and a preview), which the picker needs, and its content. Loading
//! reads the info and skips the content, so startup doesn't slow down with
//! images. Removing an entry appends a record saying so; rewriting the file
//! drops what's dead, by copying the live records as they are, still sealed.
//!
//! The file starts with a 16-byte header: `MOCHICLP`, the format version,
//! and a flags byte, 1 when the parts are encrypted. Then each record:
//!
//! ```text
//! u32 length | sealed info | u32 length | sealed content
//! ```
//!
//! With a key, a sealed part is a random 24-byte nonce and the part
//! encrypted with XChaCha20-Poly1305. Without one, it's the part followed
//! by the first 16 bytes of its BLAKE3 hash. Either way a torn or altered
//! record is caught. Integers are little-endian.
//!
//! Info, unsealed: `op` (1 add, 2 remove, 3 touch), `id: u64`, `time: u64`,
//! and for an add: `kind`, `compressed`, `hash: [u8; 32]`, `width: u32`,
//! `height: u32`, the format count and each format as `u16` length, MIME
//! type and `u32` size, then the preview as `u32` length and UTF-8. Content,
//! unsealed: the formats' bytes one after another, compressed with zstd
//! when `compressed` is 1.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use zeroize::{Zeroize, Zeroizing};

const MAGIC: &[u8; 8] = b"MOCHICLP";
const VERSION: u8 = 1;
const ENCRYPTED: u8 = 1;
const HEADER: u64 = 16;

const ADD: u8 = 1;
const REMOVE: u8 = 2;
const TOUCH: u8 = 3;

const NONCE: usize = 24;
const CHECKSUM: usize = 16;
/// How much of a text the index keeps, to show and search.
const PREVIEW: usize = 4096;
/// More than any real record's info; anything larger is damage.
const MAX_INFO: u32 = 1 << 20;

/// The 32-byte key for an encrypted history.
pub type Key = Zeroizing<[u8; 32]>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Text,
    Image,
}

/// Something copied, with every format kept, the main one first.
#[derive(Debug)]
pub struct Clip {
    pub kind: Kind,
    pub formats: Vec<(String, Zeroizing<Vec<u8>>)>,
}

/// One entry, as the index keeps it.
#[derive(Debug)]
pub struct Entry {
    pub id: u64,
    /// Seconds since the epoch, of the last copy.
    pub time: u64,
    pub kind: Kind,
    pub hash: [u8; 32],
    /// Image size in pixels, 0 when unknown or for text.
    pub width: u32,
    pub height: u32,
    /// MIME types and sizes, the main one first.
    pub formats: Vec<(String, u32)>,
    /// The start of the text, for an image nothing.
    pub preview: Zeroizing<String>,
    compressed: bool,
    /// Where the add record is, all of it, and its time when written.
    record: (u64, u64),
    written: u64,
    /// Where the sealed content is.
    content: (u64, u64),
}

impl Entry {
    pub fn size(&self) -> u64 {
        self.formats.iter().map(|(_, size)| u64::from(*size)).sum()
    }
}

/// Encrypts with the key, or checksums without one.
enum Seal {
    Plain,
    Key(Box<XChaCha20Poly1305>),
}

impl std::fmt::Debug for Seal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Plain => "Plain",
            Self::Key(_) => "Key",
        })
    }
}

impl Seal {
    fn seal(&self, part: &[u8], aad: &[u8]) -> io::Result<Vec<u8>> {
        match self {
            Self::Plain => {
                let mut sealed = Vec::with_capacity(part.len() + CHECKSUM);
                sealed.extend_from_slice(part);
                sealed.extend_from_slice(&checksum(part, aad));
                Ok(sealed)
            }
            Self::Key(cipher) => {
                let mut nonce = [0; NONCE];
                getrandom::fill(&mut nonce).map_err(|error| io::Error::other(error.to_string()))?;
                let encrypted = cipher
                    .encrypt(XNonce::from_slice(&nonce), Payload { msg: part, aad })
                    .map_err(|_| io::Error::other("cannot encrypt"))?;
                let mut sealed = Vec::with_capacity(NONCE + encrypted.len());
                sealed.extend_from_slice(&nonce);
                sealed.extend_from_slice(&encrypted);
                Ok(sealed)
            }
        }
    }

    fn open(&self, sealed: &[u8], aad: &[u8]) -> io::Result<Zeroizing<Vec<u8>>> {
        let broken = || io::Error::new(io::ErrorKind::InvalidData, "a damaged record");
        match self {
            Self::Plain => {
                let split = sealed.len().checked_sub(CHECKSUM).ok_or_else(broken)?;
                let (part, sum) = sealed.split_at(split);
                if checksum(part, aad) != sum {
                    return Err(broken());
                }
                Ok(Zeroizing::new(part.to_vec()))
            }
            Self::Key(cipher) => {
                if sealed.len() < NONCE {
                    return Err(broken());
                }
                let (nonce, encrypted) = sealed.split_at(NONCE);
                cipher
                    .decrypt(
                        XNonce::from_slice(nonce),
                        Payload {
                            msg: encrypted,
                            aad,
                        },
                    )
                    .map(Zeroizing::new)
                    .map_err(|_| broken())
            }
        }
    }

    fn flags(&self) -> u8 {
        match self {
            Self::Plain => 0,
            Self::Key(_) => ENCRYPTED,
        }
    }
}

fn checksum(part: &[u8], aad: &[u8]) -> [u8; CHECKSUM] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(aad);
    hasher.update(part);
    let mut sum = [0; CHECKSUM];
    sum.copy_from_slice(&hasher.finalize().as_bytes()[..CHECKSUM]);
    sum
}

/// What a part is sealed with, besides the key: an info part can't pass for
/// another kind of part, nor content for another entry's.
const INFO_AAD: &[u8] = b"mochi clipboard info";
fn content_aad(id: u64) -> Vec<u8> {
    let mut aad = b"mochi clipboard content ".to_vec();
    aad.extend_from_slice(&id.to_le_bytes());
    aad
}

#[derive(Debug)]
pub struct Store {
    path: PathBuf,
    file: File,
    seal: Seal,
    /// Newest first.
    entries: Vec<Entry>,
    next_id: u64,
    /// Bytes of records that no longer count.
    dead: u64,
}

impl Store {
    /// Opens the history at `path`, creating it and its directory, readable
    /// by the user only. A file that isn't a history, or was written with
    /// another key or none, is moved aside as `<name>.unreadable`, and the
    /// history starts empty. A damaged end, from a crash during a write, is
    /// cut off.
    pub fn open(path: &Path, key: Option<Key>) -> io::Result<Self> {
        if let Some(dir) = path.parent() {
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(dir)?;
        }
        let seal = match key {
            Some(key) => Seal::Key(Box::new(XChaCha20Poly1305::new((&*key).into()))),
            None => Seal::Plain,
        };
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(path)?;

        let mut store = Self {
            path: path.to_owned(),
            file,
            seal,
            entries: Vec::new(),
            next_id: 1,
            dead: 0,
        };
        let length = store.file.metadata()?.len();
        if length == 0 {
            store.write_header()?;
            return Ok(store);
        }

        let mut header = [0; HEADER as usize];
        let readable = store.file.read_exact(&mut header).is_ok()
            && &header[..8] == MAGIC
            && header[8] == VERSION
            && header[9] == store.seal.flags();
        if readable && store.replay(length).is_ok() {
            return Ok(store);
        }

        // Not ours to read: keep it, but out of the way.
        let mut aside = path.as_os_str().to_owned();
        aside.push(".unreadable");
        tracing::warn!(
            path = %path.display(),
            aside = ?aside,
            "the clipboard history can't be read with this key; starting a new one"
        );
        fs::rename(path, &aside)?;
        file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        store.file = file;
        store.entries.clear();
        store.next_id = 1;
        store.dead = 0;
        store.write_header()?;
        Ok(store)
    }

    fn write_header(&mut self) -> io::Result<()> {
        let mut header = [0; HEADER as usize];
        header[..8].copy_from_slice(MAGIC);
        header[8] = VERSION;
        header[9] = self.seal.flags();
        self.file.set_len(0)?;
        self.file.seek(SeekFrom::Start(0))?;
        self.file.write_all(&header)
    }

    /// Reads every record's info. Fails when the first record can't be
    /// opened, which means the wrong key; a bad record later on is the end
    /// of a torn write, and is cut off with everything after it.
    fn replay(&mut self, length: u64) -> io::Result<()> {
        let mut offset = HEADER;
        let mut first = true;
        while offset < length {
            match self.read_record(offset, length) {
                Ok(next) => offset = next,
                Err(error) if first => return Err(error),
                Err(error) => {
                    tracing::warn!(%error, offset, "cutting off a damaged clipboard record");
                    self.file.set_len(offset)?;
                    break;
                }
            }
            first = false;
        }
        self.sort();
        Ok(())
    }

    /// Reads the record at `offset` into the index and returns where the
    /// next one starts.
    fn read_record(&mut self, offset: u64, length: u64) -> io::Result<u64> {
        let broken = || io::Error::new(io::ErrorKind::InvalidData, "a damaged record");
        self.file.seek(SeekFrom::Start(offset))?;
        let info_length = read_u32(&mut self.file)?;
        if info_length > MAX_INFO || offset + 8 + u64::from(info_length) > length {
            return Err(broken());
        }
        let mut sealed = vec![0; info_length as usize];
        self.file.read_exact(&mut sealed)?;
        let info = self.seal.open(&sealed, INFO_AAD)?;
        let content_length = u64::from(read_u32(&mut self.file)?);
        let content_offset = offset + 8 + u64::from(info_length);
        let end = content_offset + content_length;
        if end > length {
            return Err(broken());
        }

        let mut reader = Reader(&info);
        let op = reader.u8()?;
        let id = reader.u64()?;
        let time = reader.u64()?;
        self.next_id = self.next_id.max(id + 1);
        let record_length = end - offset;
        match op {
            ADD => {
                let kind = match reader.u8()? {
                    0 => Kind::Text,
                    1 => Kind::Image,
                    _ => return Err(broken()),
                };
                let compressed = reader.u8()? == 1;
                let hash = reader.array::<32>()?;
                let width = reader.u32()?;
                let height = reader.u32()?;
                let count = reader.u8()?;
                let mut formats = Vec::with_capacity(usize::from(count));
                for _ in 0..count {
                    let length = usize::from(reader.u16()?);
                    let mime =
                        String::from_utf8(reader.bytes(length)?.to_vec()).map_err(|_| broken())?;
                    formats.push((mime, reader.u32()?));
                }
                let length = reader.u32()? as usize;
                let preview = Zeroizing::new(
                    String::from_utf8(reader.bytes(length)?.to_vec()).map_err(|_| broken())?,
                );
                self.entries.push(Entry {
                    id,
                    time,
                    kind,
                    hash,
                    width,
                    height,
                    formats,
                    preview,
                    compressed,
                    record: (offset, record_length),
                    written: time,
                    content: (content_offset, content_length),
                });
            }
            REMOVE => {
                self.dead += record_length;
                if let Some(index) = self.index(id) {
                    let entry = self.entries.remove(index);
                    self.dead += entry.record.1;
                }
            }
            TOUCH => {
                self.dead += record_length;
                if let Some(index) = self.index(id) {
                    self.entries[index].time = time;
                }
            }
            _ => return Err(broken()),
        }
        Ok(end)
    }

    /// Newest first.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn get(&self, id: u64) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    fn index(&self, id: u64) -> Option<usize> {
        self.entries.iter().position(|entry| entry.id == id)
    }

    fn sort(&mut self) {
        self.entries
            .sort_by(|a, b| b.time.cmp(&a.time).then(b.id.cmp(&a.id)));
    }

    /// Adds a copy at `time` and returns its id. Copying the same thing
    /// again moves the entry to the top instead of storing it twice.
    pub fn add(&mut self, clip: &Clip, time: u64) -> io::Result<u64> {
        let hash = hash(clip);
        if let Some(index) = self.entries.iter().position(|entry| entry.hash == hash) {
            let id = self.entries[index].id;
            self.touch(id, time)?;
            return Ok(id);
        }

        let id = self.next_id;
        self.next_id += 1;
        let (width, height) = match clip.kind {
            Kind::Image => clip
                .formats
                .first()
                .and_then(|(_, data)| png_size(data))
                .unwrap_or((0, 0)),
            Kind::Text => (0, 0),
        };
        let preview = match clip.kind {
            Kind::Text => clip
                .formats
                .first()
                .map(|(_, data)| preview(data))
                .unwrap_or_default(),
            Kind::Image => Zeroizing::new(String::new()),
        };
        // Text shrinks a lot; images are already compressed.
        let compressed = clip.kind == Kind::Text;

        let mut info = Zeroizing::new(Vec::new());
        info.push(ADD);
        info.extend_from_slice(&id.to_le_bytes());
        info.extend_from_slice(&time.to_le_bytes());
        info.push(match clip.kind {
            Kind::Text => 0,
            Kind::Image => 1,
        });
        info.push(u8::from(compressed));
        info.extend_from_slice(&hash);
        info.extend_from_slice(&width.to_le_bytes());
        info.extend_from_slice(&height.to_le_bytes());
        let formats: Vec<_> = clip.formats.iter().take(usize::from(u8::MAX)).collect();
        info.push(formats.len() as u8);
        for (mime, data) in &formats {
            let mime = &mime.as_bytes()[..mime.len().min(usize::from(u16::MAX))];
            info.extend_from_slice(&(mime.len() as u16).to_le_bytes());
            info.extend_from_slice(mime);
            info.extend_from_slice(&size(data)?.to_le_bytes());
        }
        info.extend_from_slice(&(preview.len() as u32).to_le_bytes());
        info.extend_from_slice(preview.as_bytes());

        let mut content = Zeroizing::new(Vec::new());
        for (_, data) in &formats {
            content.extend_from_slice(data);
        }
        if compressed {
            content = Zeroizing::new(zstd::bulk::compress(&content, 3)?);
        }

        let sealed_info = self.seal.seal(&info, INFO_AAD)?;
        let sealed_content = self.seal.seal(&content, &content_aad(id))?;
        let offset = self.file.seek(SeekFrom::End(0))?;
        let mut record = Vec::with_capacity(8 + sealed_info.len() + sealed_content.len());
        record.extend_from_slice(&size(&sealed_info)?.to_le_bytes());
        record.extend_from_slice(&sealed_info);
        record.extend_from_slice(&size(&sealed_content)?.to_le_bytes());
        record.extend_from_slice(&sealed_content);
        self.file.write_all(&record)?;

        let content_offset = offset + 8 + sealed_info.len() as u64;
        self.entries.push(Entry {
            id,
            time,
            kind: clip.kind,
            hash,
            width,
            height,
            formats: formats
                .iter()
                .map(|(mime, data)| (mime.clone(), data.len() as u32))
                .collect(),
            preview,
            compressed,
            record: (offset, record.len() as u64),
            written: time,
            content: (content_offset, sealed_content.len() as u64),
        });
        self.sort();
        Ok(id)
    }

    /// Moves an entry to the top, as copied at `time`.
    pub fn touch(&mut self, id: u64, time: u64) -> io::Result<()> {
        let length = self.append_op(TOUCH, id, time)?;
        self.dead += length;
        if let Some(index) = self.index(id) {
            self.entries[index].time = time;
        }
        self.sort();
        Ok(())
    }

    fn append_op(&mut self, op: u8, id: u64, time: u64) -> io::Result<u64> {
        let mut info = Vec::with_capacity(17);
        info.push(op);
        info.extend_from_slice(&id.to_le_bytes());
        info.extend_from_slice(&time.to_le_bytes());
        let sealed = self.seal.seal(&info, INFO_AAD)?;
        let mut record = Vec::with_capacity(8 + sealed.len());
        record.extend_from_slice(&size(&sealed)?.to_le_bytes());
        record.extend_from_slice(&sealed);
        record.extend_from_slice(&0u32.to_le_bytes());
        self.file.seek(SeekFrom::End(0))?;
        self.file.write_all(&record)?;
        Ok(record.len() as u64)
    }

    /// Every format of an entry, with its bytes.
    pub fn content(&mut self, id: u64) -> io::Result<Vec<(String, Zeroizing<Vec<u8>>)>> {
        let entry = self
            .get(id)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no such entry"))?;
        let (offset, length) = entry.content;
        let compressed = entry.compressed;
        let formats = entry.formats.clone();
        let total: u64 = entry.size();

        let mut sealed = Zeroizing::new(vec![0; length as usize]);
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.read_exact(&mut sealed)?;
        let mut content = self.seal.open(&sealed, &content_aad(id))?;
        if compressed {
            content = Zeroizing::new(zstd::bulk::decompress(&content, total as usize)?);
        }

        let mut rest = content.as_slice();
        let mut out = Vec::with_capacity(formats.len());
        for (mime, size) in formats {
            let size = (size as usize).min(rest.len());
            let (data, tail) = rest.split_at(size);
            out.push((mime, Zeroizing::new(data.to_vec())));
            rest = tail;
        }
        Ok(out)
    }

    /// Removes an entry and rewrites the file at once, so nothing of it is
    /// left behind.
    pub fn remove(&mut self, id: u64) -> io::Result<()> {
        let Some(index) = self.index(id) else {
            return Ok(());
        };
        let entry = self.entries.remove(index);
        self.dead += entry.record.1;
        self.compact()
    }

    /// Removes everything, the file emptied to its header.
    pub fn clear(&mut self) -> io::Result<()> {
        self.entries.clear();
        self.dead = 0;
        self.write_header()?;
        self.file.sync_data()
    }

    /// Drops entries beyond the `max_entries` newest, and, when `max_age` is
    /// set, those last copied more than `max_age` seconds before `now`.
    /// Returns the ids removed.
    pub fn expire(&mut self, max_entries: usize, max_age: u64, now: u64) -> io::Result<Vec<u64>> {
        let mut removed = Vec::new();
        let mut kept = Vec::with_capacity(self.entries.len());
        for (index, entry) in std::mem::take(&mut self.entries).into_iter().enumerate() {
            let old = max_age > 0 && now.saturating_sub(entry.time) > max_age;
            if index >= max_entries || old {
                self.dead += entry.record.1;
                removed.push(entry.id);
            } else {
                kept.push(entry);
            }
        }
        self.entries = kept;
        if !removed.is_empty() {
            self.compact()?;
        }
        Ok(removed)
    }

    /// Rewrites the file with only the live records, copied as they are,
    /// and a touch record for each entry copied again since. Runs when
    /// something was removed, and when more than half the file is dead.
    fn compact(&mut self) -> io::Result<()> {
        let mut temporary = self.path.as_os_str().to_owned();
        temporary.push(".new");
        let temporary = PathBuf::from(temporary);
        let mut new = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&temporary)?;
        let mut header = [0; HEADER as usize];
        header[..8].copy_from_slice(MAGIC);
        header[8] = VERSION;
        header[9] = self.seal.flags();
        new.write_all(&header)?;

        // Oldest first, so replaying gives the same order.
        let mut offset = HEADER;
        let mut touches = Vec::new();
        for entry in self.entries.iter_mut().rev() {
            let (old, length) = entry.record;
            let mut record = Zeroizing::new(vec![0; length as usize]);
            self.file.seek(SeekFrom::Start(old))?;
            self.file.read_exact(&mut record)?;
            new.write_all(&record)?;
            entry.content.0 = entry.content.0 - old + offset;
            entry.record.0 = offset;
            offset += length;
            if entry.time != entry.written {
                touches.push((entry.id, entry.time));
            }
        }
        self.file = new;
        self.dead = 0;
        for (id, time) in touches {
            self.append_op(TOUCH, id, time)?;
            self.dead = 0;
        }
        self.file.sync_data()?;
        fs::rename(&temporary, &self.path)?;
        // The touch records are live now, but compacting again rewrites
        // them; counting them as dead would only compact sooner.
        Ok(())
    }

    /// Compacts when more than half the file no longer counts.
    pub fn tidy(&mut self) -> io::Result<()> {
        let live: u64 = self.entries.iter().map(|entry| entry.record.1).sum();
        if self.dead > live.max(64 * 1024) {
            self.compact()?;
        }
        Ok(())
    }
}

fn hash(clip: &Clip) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&[match clip.kind {
        Kind::Text => 0,
        Kind::Image => 1,
    }]);
    if let Some((_, data)) = clip.formats.first() {
        hasher.update(data);
    }
    *hasher.finalize().as_bytes()
}

fn size(data: &[u8]) -> io::Result<u32> {
    u32::try_from(data.len()).map_err(|_| io::Error::other("too large to keep"))
}

/// The start of a text, cut at a character.
fn preview(data: &[u8]) -> Zeroizing<String> {
    let mut end = data.len().min(PREVIEW);
    let mut text = loop {
        match std::str::from_utf8(&data[..end]) {
            Ok(text) => break text.to_owned(),
            // Cut inside a character: step back to its start.
            Err(error) if error.error_len().is_none() => end = error.valid_up_to(),
            Err(_) => break String::from_utf8_lossy(&data[..end]).into_owned(),
        }
    };
    let preview = Zeroizing::new(text.clone());
    text.zeroize();
    preview
}

/// A PNG's size, from its header.
fn png_size(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() < 24 || &data[..8] != b"\x89PNG\r\n\x1a\n" || &data[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(data[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(data[20..24].try_into().ok()?);
    Some((width, height))
}

fn read_u32(file: &mut File) -> io::Result<u32> {
    let mut bytes = [0; 4];
    file.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

/// Reads fields from an unsealed info part.
struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn bytes(&mut self, length: usize) -> io::Result<&[u8]> {
        if self.0.len() < length {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "a short record"));
        }
        let (bytes, rest) = self.0.split_at(length);
        self.0 = rest;
        Ok(bytes)
    }

    fn array<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        let mut array = [0; N];
        array.copy_from_slice(self.bytes(N)?);
        Ok(array)
    }

    fn u8(&mut self) -> io::Result<u8> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> io::Result<u16> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> io::Result<u64> {
        Ok(u64::from_le_bytes(self.array()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dir(PathBuf);

    impl Dir {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("mochi-clipboard-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            Self(dir)
        }

        fn file(&self) -> PathBuf {
            self.0.join("history")
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

    fn key(byte: u8) -> Option<Key> {
        Some(Zeroizing::new([byte; 32]))
    }

    fn previews(store: &Store) -> Vec<&str> {
        store
            .entries()
            .iter()
            .map(|entry| entry.preview.as_str())
            .collect()
    }

    #[test]
    fn keeps_copies_newest_first_across_a_restart() {
        let dir = Dir::new("restart");
        let mut store = Store::open(&dir.file(), key(7)).unwrap();
        store.add(&text("first"), 10).unwrap();
        let mut html = text("second");
        html.formats.push((
            "text/html".into(),
            Zeroizing::new(b"<b>second</b>".to_vec()),
        ));
        let second = store.add(&html, 20).unwrap();
        assert_eq!(previews(&store), ["second", "first"]);
        drop(store);

        let mut store = Store::open(&dir.file(), key(7)).unwrap();
        assert_eq!(previews(&store), ["second", "first"]);
        let content = store.content(second).unwrap();
        assert_eq!(content[0].0, "text/plain;charset=utf-8");
        assert_eq!(content[0].1.as_slice(), b"second");
        assert_eq!(content[1].1.as_slice(), b"<b>second</b>");
    }

    #[test]
    fn copying_again_moves_the_entry_up() {
        let dir = Dir::new("touch");
        let mut store = Store::open(&dir.file(), None).unwrap();
        let first = store.add(&text("first"), 10).unwrap();
        store.add(&text("second"), 20).unwrap();
        assert_eq!(store.add(&text("first"), 30).unwrap(), first);
        assert_eq!(previews(&store), ["first", "second"]);
        assert_eq!(store.entries().len(), 2);
        drop(store);
        let store = Store::open(&dir.file(), None).unwrap();
        assert_eq!(previews(&store), ["first", "second"]);
    }

    #[test]
    fn nothing_is_readable_without_the_key() {
        let dir = Dir::new("encrypted");
        let mut store = Store::open(&dir.file(), key(7)).unwrap();
        store.add(&text("hunter2 is my password"), 10).unwrap();
        drop(store);

        let raw = fs::read(dir.file()).unwrap();
        assert!(!raw.windows(7).any(|window| window == b"hunter2"));

        // Another key, or none, moves the file aside and starts over.
        let store = Store::open(&dir.file(), key(8)).unwrap();
        assert!(store.entries().is_empty());
        assert!(dir.0.join("history.unreadable").exists());
        drop(store);
        let store = Store::open(&dir.file(), key(8)).unwrap();
        assert!(store.entries().is_empty());
    }

    #[test]
    fn removing_leaves_nothing_in_the_file() {
        let dir = Dir::new("remove");
        let mut store = Store::open(&dir.file(), None).unwrap();
        let secret = store.add(&text("a secret token"), 10).unwrap();
        store.add(&text("keep me"), 20).unwrap();
        store.add(&text("a secret token"), 30).unwrap();
        store.remove(secret).unwrap();
        assert_eq!(previews(&store), ["keep me"]);

        let raw = fs::read(dir.file()).unwrap();
        assert!(!raw.windows(6).any(|window| window == b"secret"));
        let keep = store.entries()[0].id;
        assert_eq!(store.content(keep).unwrap()[0].1.as_slice(), b"keep me");
        drop(store);
        let store = Store::open(&dir.file(), None).unwrap();
        assert_eq!(previews(&store), ["keep me"]);
    }

    #[test]
    fn compacting_keeps_order_and_content() {
        let dir = Dir::new("compact");
        let mut store = Store::open(&dir.file(), key(1)).unwrap();
        let a = store.add(&text("a"), 10).unwrap();
        let b = store.add(&text("b"), 20).unwrap();
        let c = store.add(&text("c"), 30).unwrap();
        store.add(&text("a"), 40).unwrap();
        store.remove(c).unwrap();
        assert_eq!(previews(&store), ["a", "b"]);
        assert_eq!(store.content(a).unwrap()[0].1.as_slice(), b"a");
        assert_eq!(store.content(b).unwrap()[0].1.as_slice(), b"b");
        drop(store);
        let mut store = Store::open(&dir.file(), key(1)).unwrap();
        assert_eq!(previews(&store), ["a", "b"]);
        assert_eq!(store.content(b).unwrap()[0].1.as_slice(), b"b");
        assert!(store.get(c).is_none());
    }

    #[test]
    fn expires_by_count_and_age() {
        let dir = Dir::new("expire");
        let mut store = Store::open(&dir.file(), None).unwrap();
        for (index, word) in ["one", "two", "three", "four"].iter().enumerate() {
            store.add(&text(word), 100 * (index as u64 + 1)).unwrap();
        }
        assert_eq!(store.expire(3, 0, 400).unwrap().len(), 1);
        assert_eq!(previews(&store), ["four", "three", "two"]);
        // Older than 150 s at 450: two, last copied at 200.
        assert_eq!(store.expire(10, 150, 450).unwrap().len(), 1);
        assert_eq!(previews(&store), ["four", "three"]);
        store.clear().unwrap();
        assert!(store.entries().is_empty());
        assert_eq!(fs::metadata(dir.file()).unwrap().len(), HEADER);
    }

    #[test]
    fn a_torn_write_loses_only_the_last_record() {
        let dir = Dir::new("torn");
        let mut store = Store::open(&dir.file(), key(3)).unwrap();
        store.add(&text("whole"), 10).unwrap();
        store.add(&text("torn"), 20).unwrap();
        drop(store);
        let length = fs::metadata(dir.file()).unwrap().len();
        OpenOptions::new()
            .write(true)
            .open(dir.file())
            .unwrap()
            .set_len(length - 5)
            .unwrap();

        let mut store = Store::open(&dir.file(), key(3)).unwrap();
        assert_eq!(previews(&store), ["whole"]);
        store.add(&text("after"), 30).unwrap();
        drop(store);
        let store = Store::open(&dir.file(), key(3)).unwrap();
        assert_eq!(previews(&store), ["after", "whole"]);
    }

    #[test]
    fn images_keep_their_size_and_bytes() {
        let dir = Dir::new("image");
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png.extend_from_slice(&640u32.to_be_bytes());
        png.extend_from_slice(&480u32.to_be_bytes());
        png.extend_from_slice(&[0; 100]);
        let clip = Clip {
            kind: Kind::Image,
            formats: vec![("image/png".into(), Zeroizing::new(png.clone()))],
        };
        let mut store = Store::open(&dir.file(), key(2)).unwrap();
        let id = store.add(&clip, 10).unwrap();
        let entry = store.get(id).unwrap();
        assert_eq!((entry.width, entry.height), (640, 480));
        assert_eq!(store.content(id).unwrap()[0].1.as_slice(), png.as_slice());
    }

    #[test]
    fn previews_stop_at_a_character() {
        let long = "é".repeat(PREVIEW);
        let preview = preview(long.as_bytes());
        assert_eq!(preview.len(), PREVIEW);
        assert!(preview.chars().all(|c| c == 'é'));
    }
}
