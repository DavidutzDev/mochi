//! Who has a camera open: the processes with a `/dev/video*` device among
//! their open files. The kernel says nothing when one opens, so the module
//! looks every few seconds; only the user's own processes are readable,
//! which are the ones that matter.

use std::path::Path;

/// The names of the processes under `proc` with a camera open, each once
/// and sorted, leaving out `own`, Mochi itself.
pub fn users(proc: &Path, own: u32) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(proc) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|entry| {
            let pid: u32 = entry.file_name().to_str()?.parse().ok()?;
            if pid == own || !has_camera(&entry.path()) {
                return None;
            }
            let name = std::fs::read_to_string(entry.path().join("comm")).ok()?;
            Some(name.trim().to_owned())
        })
        .collect();
    names.sort();
    names.dedup();
    names
}

fn has_camera(process: &Path) -> bool {
    let Ok(files) = std::fs::read_dir(process.join("fd")) else {
        return false;
    };
    files.flatten().any(|file| {
        std::fs::read_link(file.path()).is_ok_and(|target| {
            target
                .to_str()
                .is_some_and(|target| target.starts_with("/dev/video"))
        })
    })
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::symlink;

    use super::*;

    #[test]
    fn finds_who_has_a_camera_open() {
        let root = std::env::temp_dir().join(format!("mochi-proc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let process = |pid: u32, name: &str, files: &[&str]| {
            let fd = root.join(pid.to_string()).join("fd");
            std::fs::create_dir_all(&fd).unwrap();
            std::fs::write(root.join(pid.to_string()).join("comm"), format!("{name}\n")).unwrap();
            for (number, target) in files.iter().enumerate() {
                symlink(target, fd.join(number.to_string())).unwrap();
            }
        };
        process(10, "firefox", &["/dev/null", "/dev/video0"]);
        process(11, "firefox", &["/dev/video2"]);
        process(12, "kitty", &["/dev/pts/1"]);
        process(13, "mochid", &["/dev/video0"]);
        std::fs::create_dir_all(root.join("self")).unwrap();
        assert_eq!(users(&root, 13), ["firefox"]);
        assert!(users(&root.join("missing"), 0).is_empty());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
