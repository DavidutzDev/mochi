//! Where captures go and what they're called.

use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// `setting` with a leading `~` expanded, or `fallback` under the XDG user
/// directory when the setting is empty: `$XDG_PICTURES_DIR/Screenshots`, for
/// example.
pub fn folder(setting: &str, user_dir: &str, fallback: &str, sub: &str) -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    if !setting.is_empty() {
        return expand(setting, &home);
    }
    let base = std::env::var_os(user_dir)
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .unwrap_or_else(|| home.join(fallback));
    base.join(sub)
}

fn expand(path: &str, home: &Path) -> PathBuf {
    match path.strip_prefix("~") {
        Some("") => home.to_owned(),
        Some(rest) if rest.starts_with('/') => home.join(&rest[1..]),
        _ => PathBuf::from(path),
    }
}

/// `pattern`, with `strftime` fields filled in from the local time `when`.
pub fn timestamp(pattern: &str, when: SystemTime) -> String {
    let seconds = when
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let Ok(format) = CString::new(pattern) else {
        return pattern.to_owned();
    };
    let time = libc::time_t::try_from(seconds).unwrap_or_default();
    // SAFETY: `tm` is plain data that `localtime_r` fills in completely; an
    // all-zero value is a valid starting point.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let mut buffer = [0u8; 512];
    // SAFETY: both pointers are valid for the call, `localtime_r` is the
    // thread-safe variant, and `strftime` writes at most `buffer.len()` bytes
    // and returns how many it wrote, 0 on overflow.
    let written = unsafe {
        if libc::localtime_r(&time, &mut tm).is_null() {
            return pattern.to_owned();
        }
        libc::strftime(
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            format.as_ptr(),
            &tm,
        )
    };
    String::from_utf8_lossy(&buffer[..written]).into_owned()
}

/// `<folder>/<name>.<extension>`, or with ` (2)`, ` (3)`, … added when that
/// file exists. File names can't hold `/`, so a pattern's slashes become
/// dashes.
pub fn unused(folder: &Path, name: &str, extension: &str) -> PathBuf {
    let name = name.replace('/', "-");
    let mut path = folder.join(format!("{name}.{extension}"));
    let mut number = 2;
    while path.exists() {
        path = folder.join(format!("{name} ({number}).{extension}"));
        number += 1;
    }
    path
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn expands_the_home_directory() {
        let home = Path::new("/home/me");
        assert_eq!(expand("~", home), PathBuf::from("/home/me"));
        assert_eq!(expand("~/Shots", home), PathBuf::from("/home/me/Shots"));
        assert_eq!(expand("/srv/shots", home), PathBuf::from("/srv/shots"));
        // Another user's home stays as written.
        assert_eq!(expand("~bob/x", home), PathBuf::from("~bob/x"));
    }

    #[test]
    fn fills_in_the_time() {
        let when = SystemTime::UNIX_EPOCH + Duration::from_secs(86_400 * 365);
        let name = timestamp("Screenshot %Y", when);
        // A year after the epoch is 1971 in every time zone.
        assert_eq!(name, "Screenshot 1971");
        assert_eq!(timestamp("plain", when), "plain");
    }

    #[test]
    fn never_reuses_a_name() {
        let dir = std::env::temp_dir().join(format!("mochi-files-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let first = unused(&dir, "Shot a/b", "png");
        assert_eq!(first, dir.join("Shot a-b.png"));
        std::fs::write(&first, b"").unwrap();
        assert_eq!(
            unused(&dir, "Shot a/b", "png"),
            dir.join("Shot a-b (2).png")
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
