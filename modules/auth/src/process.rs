//! Who asks, as the kernel says rather than as the asker does. A process
//! can name itself anything (`comm`, `argv[0]`), but not change the file it
//! runs (`/proc/<pid>/exe`), its parent, or the user it runs as.
//!
//! For `mochi askpass`, mochid knows the process that wrote the question
//! from the kernel (`SCM_CREDENTIALS`); the one asking is its parent, sudo or
//! ssh, whose command line the prompt shows: `sudo -A rm -rf /` can't hide
//! behind "sudo needs a password".

use std::path::{Path, PathBuf};

/// A process, as `/proc` has it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Process {
    pub pid: u32,
    /// The file it runs. Unreadable for a process running as another user,
    /// like setuid sudo.
    pub exe: Option<PathBuf>,
    /// The name of the file it ran, as `exec` set it, for when `exe` can't be
    /// read. A process can rename itself, but setuid programs don't.
    pub comm: String,
    /// When it started, in clock ticks since boot: with its id, which process
    /// it is, as ids are reused.
    pub start: Option<u64>,
    /// Its arguments, as it started.
    pub command: Vec<String>,
    /// It runs as root (its effective user): sudo, pkexec, or another
    /// setuid program. A program the user starts can't make itself that.
    pub privileged: bool,
    /// The file it runs is in a place the user can write to, like the home
    /// folder or /tmp: not a program the system installed.
    pub user_writable: bool,
}

impl Process {
    /// Reads a process from `/proc`, `None` once it's gone.
    pub fn read(pid: u32) -> Option<Self> {
        Self::read_in(Path::new("/proc"), pid)
    }

    fn read_in(proc: &Path, pid: u32) -> Option<Self> {
        let dir = proc.join(pid.to_string());
        let status = std::fs::read_to_string(dir.join("status")).ok()?;
        let exe = std::fs::read_link(dir.join("exe")).ok();
        let command = std::fs::read(dir.join("cmdline"))
            .map(|bytes| {
                bytes
                    .split(|byte| *byte == 0)
                    .filter(|part| !part.is_empty())
                    .map(|part| String::from_utf8_lossy(part).into_owned())
                    .collect()
            })
            .unwrap_or_default();
        let user_writable = exe.as_deref().is_some_and(writable);
        let comm = std::fs::read_to_string(dir.join("comm"))
            .map(|comm| comm.trim_end().to_owned())
            .unwrap_or_default();
        Some(Self {
            pid,
            exe,
            comm,
            start: start_of(proc, pid),
            command,
            privileged: effective_uid(&status) == Some(0),
            user_writable,
        })
    }

    /// Its parent, by the id the kernel keeps.
    pub fn parent(&self) -> Option<Self> {
        parent_of(Path::new("/proc"), self.pid).and_then(Self::read)
    }

    /// Its name: the file it runs, or, when that can't be read, the name of
    /// the file it ran. Never the name in its command line, which it picks.
    pub fn name(&self) -> String {
        self.exe
            .as_deref()
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().into_owned())
            .or_else(|| Some(self.comm.clone()).filter(|comm| !comm.is_empty()))
            .map(|name| unwrapped(&name).to_owned())
            .unwrap_or_else(|| format!("process {}", self.pid))
    }

    /// Its command line to show: its name, as [`Process::name`] says it,
    /// then its arguments. The first argument is the name it gave itself,
    /// like `exec -a sudo bash`, so it's left out.
    pub fn command_line(&self) -> String {
        std::iter::once(self.name())
            .chain(self.command.iter().skip(1).cloned())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Whether it adopts the processes whose parent exits: init, or a
    /// user's systemd. A `mochi askpass` with it as its parent outlived
    /// the program that ran it, which could be anything.
    pub fn is_reaper(&self) -> bool {
        self.pid == 1 || self.name() == "systemd"
    }

    /// Whether it's Mochi's own CLI: `mochi`, or Nix's wrapped copy of it,
    /// installed beside the running mochid. A program of the user's named
    /// `mochi` elsewhere isn't.
    pub fn is_mochi(&self) -> bool {
        let beside = std::env::current_exe()
            .ok()
            .and_then(|daemon| daemon.parent().map(Path::to_path_buf));
        self.exe
            .as_deref()
            .is_some_and(|exe| is_mochi_in(exe, beside.as_deref()))
    }

    /// Why to look twice before answering it, if any. That its program
    /// can't be seen only counts for a `secret`, like a password: ssh-agent
    /// hides itself, and a yes or no tells whoever asks nothing more.
    pub fn doubt(&self, secret: bool) -> Option<&'static str> {
        if self.user_writable {
            return Some(
                "It runs from a folder you can write to, not one the system installs programs in.",
            );
        }
        if secret && self.exe.is_none() && !self.privileged {
            return Some("Its program can't be seen.");
        }
        None
    }
}

/// Whether `exe` is `mochi` in the folder `daemon`, where mochid runs from.
fn is_mochi_in(exe: &Path, daemon: Option<&Path>) -> bool {
    let named = exe
        .file_name()
        .is_some_and(|name| unwrapped(&name.to_string_lossy()) == "mochi");
    named && daemon.is_some() && exe.parent() == daemon
}

/// `.mochi-wrapped`, as Nix's wrappers name the program they start, is
/// `mochi`.
fn unwrapped(name: &str) -> &str {
    name.strip_prefix('.')
        .and_then(|name| name.strip_suffix("-wrapped"))
        .unwrap_or(name)
}

/// The effective user id, the second number of `Uid:` in `status`.
fn effective_uid(status: &str) -> Option<u32> {
    status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

/// The parent's id, the field after the state in `stat`, whose command
/// name may hold spaces and parentheses.
fn parent_of(proc: &Path, pid: u32) -> Option<u32> {
    let stat = std::fs::read_to_string(proc.join(pid.to_string()).join("stat")).ok()?;
    let rest = &stat[stat.rfind(')')? + 2..];
    rest.split(' ').nth(1)?.parse().ok()
}

/// Whether the user can change the file at `path`: write to it or to its
/// folder.
fn writable(path: &Path) -> bool {
    let can_write = |path: &Path| {
        let Ok(path) = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()) else {
            return false;
        };
        // SAFETY: access only reads the path, a valid C string.
        unsafe { libc::access(path.as_ptr(), libc::W_OK) == 0 }
    };
    can_write(path) || path.parent().is_some_and(can_write)
}

/// `mochi askpass` and the program that ran it, as it was when it asked,
/// to check that the answer goes to that program and nowhere else.
///
/// A process keeps its id when it runs another program, so a program could
/// start `mochi askpass` and then become sudo, with the answer going to a
/// file of its own. sudo and ssh read the answer from a pipe they made, so
/// it has to be a pipe that only the program asking reads, and a root one
/// has to be sudo asking through mochi-askpass, with nothing else started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asking {
    cli: u32,
    parent: u32,
    parent_start: Option<u64>,
    /// The pipe `mochi askpass` answers on, its standard output.
    pipe: Option<u64>,
}

impl Asking {
    pub fn new(cli: &Process, parent: &Process) -> Self {
        Self {
            cli: cli.pid,
            parent: parent.pid,
            parent_start: parent.start,
            pipe: pipe_of(Path::new("/proc"), cli.pid, 1),
        }
    }

    /// Whether the answer would go only to the program asking, or why not.
    pub fn check(&self) -> Result<(), String> {
        let proc = Path::new("/proc");
        let parent = Process::read(self.parent)
            .filter(|parent| parent.start == self.parent_start)
            .ok_or("The program that asked is gone.")?;
        let pipe = self
            .pipe
            .ok_or("Its answer goes to a file or a terminal, not to the program asking.")?;
        if pipe_of(proc, self.cli, 1) != Some(pipe) {
            return Err("Its answer would go somewhere else now.".into());
        }
        let readers = readers(proc, pipe);
        if let Some(other) = readers.iter().find(|reader| **reader != parent.pid) {
            // By the name it runs as: `cat`, not the `coreutils` it is.
            let name = Process::read(*other)
                .map(|other| other.comm)
                .filter(|comm| !comm.is_empty())
                .unwrap_or_else(|| format!("process {other}"));
            return Err(format!("{name} could read the answer too."));
        }
        if fds_readable(proc, parent.pid) && !readers.contains(&parent.pid) {
            return Err("The program named doesn't read the answer.".into());
        }
        if parent.privileged {
            if parent.name() != "sudo" || !sudo_asks_through_askpass(&parent.command) {
                return Err("It runs as root, but isn't sudo asking through mochi-askpass.".into());
            }
            if children(proc, parent.pid) != [self.cli] {
                return Err("sudo started another program besides this one.".into());
            }
        }
        Ok(())
    }
}

/// Whether sudo's command line asks for the password through askpass: `-A`
/// or `--askpass` among its options, read as sudo reads them, so that the
/// value of another option, like `-pA`, doesn't count.
fn sudo_asks_through_askpass(command: &[String]) -> bool {
    /// Short options that take a value, after `getopt`'s string in sudo.
    const VALUED: &str = "aCcDgpRrTtUu";
    /// Long options that take a value.
    const LONG_VALUED: [&str; 11] = [
        "auth-type",
        "close-from",
        "chdir",
        "group",
        "prompt",
        "chroot",
        "role",
        "type",
        "command-timeout",
        "other-user",
        "user",
    ];
    let mut args = command.iter().skip(1);
    while let Some(arg) = args.next() {
        // Options end at the command, or at `--`.
        if arg == "--" || arg == "-" || !arg.starts_with('-') {
            return false;
        }
        if let Some(long) = arg.strip_prefix("--") {
            let (name, value) = long
                .split_once('=')
                .map_or((long, None), |(name, value)| (name, Some(value)));
            // getopt takes any prefix that names one option.
            let valued = LONG_VALUED.iter().any(|option| option.starts_with(name));
            if "askpass".starts_with(name) && !valued {
                return true;
            }
            if valued && value.is_none() {
                args.next();
            }
            continue;
        }
        let flags = &arg[1..];
        for (at, flag) in flags.char_indices() {
            if flag == 'A' {
                return true;
            }
            // -h takes a value only when it's attached.
            if flag == 'h' {
                break;
            }
            if VALUED.contains(flag) {
                if at + flag.len_utf8() == flags.len() {
                    args.next();
                }
                break;
            }
        }
    }
    false
}

/// When `pid` started, the 22nd field of `stat`.
fn start_of(proc: &Path, pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(proc.join(pid.to_string()).join("stat")).ok()?;
    let rest = &stat[stat.rfind(')')? + 2..];
    rest.split(' ').nth(19)?.parse().ok()
}

/// The pipe a process's file `fd` is, by its inode.
fn pipe_of(proc: &Path, pid: u32, fd: u32) -> Option<u64> {
    let target =
        std::fs::read_link(proc.join(pid.to_string()).join("fd").join(fd.to_string())).ok()?;
    target
        .to_str()?
        .strip_prefix("pipe:[")?
        .strip_suffix(']')?
        .parse()
        .ok()
}

/// Whether Mochi can see a process's open files: its user's, and not one
/// that hid them.
fn fds_readable(proc: &Path, pid: u32) -> bool {
    std::fs::read_dir(proc.join(pid.to_string()).join("fd")).is_ok()
}

/// The processes Mochi can see that have the pipe `inode` open for reading.
fn readers(proc: &Path, inode: u64) -> Vec<u32> {
    let wanted = format!("pipe:[{inode}]");
    let mut found = Vec::new();
    for pid in pids(proc) {
        let dir = proc.join(pid.to_string());
        let Ok(fds) = std::fs::read_dir(dir.join("fd")) else {
            continue;
        };
        let reads = fds.flatten().any(|fd| {
            let is_pipe = std::fs::read_link(fd.path())
                .is_ok_and(|target| target.as_os_str() == wanted.as_str());
            is_pipe && opened_for_reading(&dir.join("fdinfo").join(fd.file_name()))
        });
        if reads {
            found.push(pid);
        }
    }
    found
}

/// Whether the file in `fdinfo` is open for reading: its `flags`' access
/// mode isn't write-only.
fn opened_for_reading(fdinfo: &Path) -> bool {
    std::fs::read_to_string(fdinfo).is_ok_and(|info| {
        info.lines()
            .find_map(|line| line.strip_prefix("flags:"))
            .and_then(|flags| u32::from_str_radix(flags.trim(), 8).ok())
            .is_some_and(|flags| flags & 0o3 != libc::O_WRONLY as u32)
    })
}

/// The processes `pid` started that are still running.
fn children(proc: &Path, pid: u32) -> Vec<u32> {
    pids(proc)
        .filter(|child| parent_of(proc, *child) == Some(pid))
        .collect()
}

fn pids(proc: &Path) -> impl Iterator<Item = u32> {
    std::fs::read_dir(proc)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().to_str()?.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_this_process() {
        let me = Process::read(std::process::id()).unwrap();
        assert!(me.exe.is_some());
        assert!(!me.command.is_empty());
        // SAFETY: geteuid has no preconditions.
        let root = unsafe { libc::geteuid() } == 0;
        assert_eq!(me.privileged, root);
        let parent = me.parent().unwrap();
        assert_eq!(Some(parent.pid), parent_of(Path::new("/proc"), me.pid));
    }

    #[test]
    fn a_name_set_by_the_process_is_not_its_name() {
        // A process named "sudo" in its comm and argv[0], running a copy in
        // a folder of its own: its name is the file's, and it's in doubt.
        let dir = std::env::temp_dir().join(format!("mochi-auth-process-{}", std::process::id()));
        let fake = dir.join("1");
        std::fs::create_dir_all(&fake).unwrap();
        std::fs::write(
            fake.join("status"),
            "Name:\tsudo\nUid:\t1000\t1000\t1000\t1000\n",
        )
        .unwrap();
        std::fs::write(fake.join("cmdline"), b"sudo\0-A\0true\0").unwrap();
        let copy = dir.join("evil");
        std::fs::write(&copy, "").unwrap();
        std::os::unix::fs::symlink(&copy, fake.join("exe")).unwrap();
        std::fs::write(fake.join("stat"), "1 (sudo) S 7 1 1").unwrap();
        let process = Process::read_in(&dir, 1).unwrap();
        assert_eq!(process.name(), "evil");
        assert_eq!(process.command, ["sudo", "-A", "true"]);
        assert_eq!(process.command_line(), "evil -A true");
        assert!(process.is_reaper(), "pid 1");
        assert!(!process.privileged);
        assert!(process.doubt(true).is_some());
        assert!(process.doubt(false).is_some(), "a copy of its own, still");
        assert_eq!(parent_of(&dir, 1), Some(7));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn reads_the_effective_user_and_nix_wrappers() {
        assert_eq!(effective_uid("Uid:\t1000\t0\t0\t0\n"), Some(0));
        assert_eq!(effective_uid("Name:\tx\n"), None);
        assert_eq!(unwrapped(".mochi-wrapped"), "mochi");
        assert_eq!(unwrapped("sudo"), "sudo");
    }

    #[test]
    fn mochi_is_the_one_beside_mochid() {
        let bin = Some(Path::new("/nix/store/x-mochi/bin"));
        assert!(is_mochi_in(Path::new("/nix/store/x-mochi/bin/mochi"), bin));
        assert!(is_mochi_in(
            Path::new("/nix/store/x-mochi/bin/.mochi-wrapped"),
            bin
        ));
        // A program the user named mochi, for sudo to run as SUDO_ASKPASS.
        assert!(!is_mochi_in(Path::new("/home/me/bin/mochi"), bin));
        assert!(!is_mochi_in(
            Path::new("/nix/store/x-mochi/bin/mochid"),
            bin
        ));
        assert!(!is_mochi_in(
            Path::new("/nix/store/x-mochi/bin/mochi"),
            None
        ));
    }

    fn args(line: &str) -> Vec<String> {
        line.split(' ').map(str::to_owned).collect()
    }

    #[test]
    fn reads_sudos_options_as_sudo_does() {
        assert!(sudo_asks_through_askpass(&args("sudo -A apt upgrade")));
        assert!(sudo_asks_through_askpass(&args("sudo -kA true")));
        assert!(sudo_asks_through_askpass(&args(
            "sudo -u root --askpass true"
        )));
        assert!(sudo_asks_through_askpass(&args("sudo --ask true")));
        assert!(!sudo_asks_through_askpass(&args("sudo -S apt upgrade")));
        // The value of another option, not -A.
        assert!(!sudo_asks_through_askpass(&args("sudo -pA true")));
        assert!(!sudo_asks_through_askpass(&args("sudo -p -A true")));
        assert!(!sudo_asks_through_askpass(&args("sudo --prompt -A true")));
        assert!(!sudo_asks_through_askpass(&args("sudo --pro -A true")));
        // After the command, it's the command's.
        assert!(!sudo_asks_through_askpass(&args("sudo ls -A")));
        assert!(!sudo_asks_through_askpass(&args("sudo -- -A")));
        // -h's value is only ever attached.
        assert!(sudo_asks_through_askpass(&args("sudo -h -A true")));
    }

    /// A pipe this process reads, its write end given to a child as its
    /// standard output, as sudo and ssh run askpass.
    fn askpass_pipe() -> (std::fs::File, std::process::Child) {
        use std::os::fd::FromRawFd;
        let mut ends = [0; 2];
        // SAFETY: pipe2 fills `ends` with two new descriptors.
        let made = unsafe { libc::pipe2(ends.as_mut_ptr(), libc::O_CLOEXEC) };
        assert_eq!(made, 0);
        // SAFETY: both are open and owned here alone.
        let (read, write) = unsafe {
            (
                std::fs::File::from_raw_fd(ends[0]),
                std::fs::File::from_raw_fd(ends[1]),
            )
        };
        let child = std::process::Command::new("sleep")
            .arg("30")
            .stdout(write)
            .spawn()
            .unwrap();
        (read, child)
    }

    #[test]
    fn the_answer_goes_only_to_the_program_asking() {
        let (read, mut child) = askpass_pipe();
        let me = Process::read(std::process::id()).unwrap();
        let cli = Process::read(child.id()).unwrap();
        let asking = Asking::new(&cli, &me);
        assert!(asking.pipe.is_some());
        assert_eq!(readers(Path::new("/proc"), asking.pipe.unwrap()), [me.pid]);
        assert_eq!(asking.check(), Ok(()));

        // Another program holding the pipe's read end could read it too.
        let other = std::process::Command::new("sleep")
            .arg("30")
            .stdin(read.try_clone().unwrap())
            .spawn();
        let mut other = other.unwrap();
        let said = asking.check().unwrap_err();
        assert!(said.contains("could read the answer too"), "{said}");
        other.kill().unwrap();
        other.wait().unwrap();

        // The program named, not reading it: the answer goes elsewhere.
        drop(read);
        assert!(asking.check().is_err());
        child.kill().unwrap();
        child.wait().unwrap();
        assert_eq!(
            asking.check(),
            Err("Its answer would go somewhere else now.".into())
        );
    }

    #[test]
    fn finds_the_children_of_a_process() {
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        assert!(children(Path::new("/proc"), std::process::id()).contains(&child.id()));
        child.kill().unwrap();
        child.wait().unwrap();
    }
}
