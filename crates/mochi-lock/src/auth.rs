//! Checking the password with PAM, through the few libpam calls it needs.
//!
//! [`Granted`] is the only way to unlock: [`check`] makes one when PAM says
//! the password is right, and nothing else can, since its field is private
//! to this module. The lock asks for one to unlock, so no other path to
//! unlocking compiles.
//!
//! The password lives in a [`Password`], wiped from memory when it goes,
//! and the copy PAM asks for is wiped by PAM's modules after use.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::{Path, PathBuf};

use zeroize::Zeroizing;

/// The proof PAM accepted the password. Only [`check`] makes one.
#[derive(Debug)]
pub struct Granted(());

/// The most bytes a password holds. Room for all of them is taken at once,
/// so typing never moves the password and leaves a copy behind.
pub const CAPACITY: usize = 1024;

/// The password being typed: wiped from memory when it's dropped.
pub struct Password(Zeroizing<Vec<u8>>);

impl std::fmt::Debug for Password {
    // Never the password, even in a debug print.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Password({} characters)", self.chars())
    }
}

impl Default for Password {
    fn default() -> Self {
        Self(Zeroizing::new(Vec::with_capacity(CAPACITY)))
    }
}

impl Password {
    /// Adds typed text. `false` when it doesn't fit, which leaves it as it
    /// was.
    pub fn push(&mut self, text: &str) -> bool {
        if self.0.len() + text.len() > CAPACITY {
            return false;
        }
        self.0.extend_from_slice(text.as_bytes());
        true
    }

    /// Takes the last character away.
    pub fn pop(&mut self) {
        // The bytes are always whole UTF-8, so the last character starts at
        // the last byte that isn't a continuation byte.
        while let Some(byte) = self.0.pop() {
            if byte & 0b1100_0000 != 0b1000_0000 {
                break;
            }
        }
    }

    pub fn clear(&mut self) {
        // Zeroizing wipes what `clear` would only forget.
        zeroize::Zeroize::zeroize(&mut *self.0);
        self.0.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// How many characters, for the dots.
    pub fn chars(&self) -> usize {
        self.0
            .iter()
            .filter(|byte| *byte & 0b1100_0000 != 0b1000_0000)
            .count()
    }

    /// Hands it over for a check and leaves this one empty.
    pub fn take(&mut self) -> Password {
        std::mem::take(self)
    }
}

/// Why the password wasn't accepted, with what PAM said, like "Account
/// locked due to 3 failed logins".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    pub messages: Vec<String>,
}

/// Where PAM's service files are, `/etc/pam.d`, or with the `test-pam`
/// feature, `MOCHI_LOCK_PAM_DIR` when it's set.
pub fn config_dir() -> PathBuf {
    #[cfg(feature = "test-pam")]
    if let Some(dir) = std::env::var_os("MOCHI_LOCK_PAM_DIR") {
        return PathBuf::from(dir);
    }
    PathBuf::from("/etc/pam.d")
}

/// Whether PAM has the service: without it, PAM falls back to its `other`
/// service, which refuses everyone, and the lock could never be undone.
pub fn has_service(service: &str) -> bool {
    !service.is_empty() && !service.contains('/') && config_dir().join(service).is_file()
}

/// The user running this, by name.
pub fn user() -> Option<String> {
    // SAFETY: getuid can't fail. getpwuid_r writes into the buffers we pass
    // and points `found` into them or leaves it null.
    unsafe {
        let mut passwd: libc::passwd = std::mem::zeroed();
        let mut buffer = vec![0 as c_char; 4096];
        let mut found: *mut libc::passwd = std::ptr::null_mut();
        let status = libc::getpwuid_r(
            libc::getuid(),
            &raw mut passwd,
            buffer.as_mut_ptr(),
            buffer.len(),
            &raw mut found,
        );
        if status != 0 || found.is_null() || passwd.pw_name.is_null() {
            return None;
        }
        CStr::from_ptr(passwd.pw_name)
            .to_str()
            .ok()
            .map(str::to_owned)
    }
}

/// Checks `password` for `user` with the PAM `service`. Blocks while PAM
/// works, which can take seconds after a wrong one: run it off the thread
/// that draws.
pub fn check(service: &str, user: &str, password: Password) -> Result<Granted, Refused> {
    let refused = |message: &str| Refused {
        messages: vec![message.to_owned()],
    };
    // Never an empty password, whatever the service would say: some
    // modules let anything through, and PAM_DISALLOW_NULL_AUTHTOK only
    // stops the ones that look at it.
    if password.is_empty() {
        return Err(refused("Type your password"));
    }
    let (Ok(service_c), Ok(user_c)) = (CString::new(service), CString::new(user)) else {
        return Err(refused("the PAM service or user name isn't valid"));
    };
    let mut talk = Talk {
        password: &password,
        answered: false,
        messages: Vec::new(),
    };
    let conversation = ffi::PamConv {
        conv: Some(converse),
        appdata_ptr: (&raw mut talk).cast(),
    };
    let mut handle: *mut c_void = std::ptr::null_mut();
    let dir = config_dir();
    // SAFETY: the strings and the conversation outlive the handle, which is
    // ended below before they go; `talk` is only touched through it by
    // `converse`, on this thread, while pam_authenticate runs.
    let status = unsafe { start(&service_c, &user_c, &conversation, &dir, &mut handle) };
    if status != ffi::PAM_SUCCESS || handle.is_null() {
        return Err(refused("PAM couldn't start"));
    }
    // SAFETY: the handle came from pam_start and is ended once, after.
    let status = unsafe { ffi::pam_authenticate(handle, ffi::PAM_DISALLOW_NULL_AUTHTOK) };
    // SAFETY: as above.
    unsafe { ffi::pam_end(handle, status) };
    let messages = talk.messages;
    // Wiped now, not whenever the caller lets go.
    drop(password);
    if status == ffi::PAM_SUCCESS {
        Ok(Granted(()))
    } else {
        Err(Refused { messages })
    }
}

/// pam_start, or with the `test-pam` feature, pam_start_confdir for a
/// directory that isn't `/etc/pam.d`.
///
/// # Safety
///
/// As for pam_start: the arguments must outlive the handle.
unsafe fn start(
    service: &CStr,
    user: &CStr,
    conversation: &ffi::PamConv,
    dir: &Path,
    handle: &mut *mut c_void,
) -> c_int {
    #[cfg(feature = "test-pam")]
    if dir != Path::new("/etc/pam.d") {
        use std::os::unix::ffi::OsStrExt;
        let Ok(dir) = CString::new(dir.as_os_str().as_bytes()) else {
            return -1;
        };
        // SAFETY: as for pam_start, with the directory too.
        return unsafe {
            ffi::pam_start_confdir(
                service.as_ptr(),
                user.as_ptr(),
                conversation,
                dir.as_ptr(),
                handle,
            )
        };
    }
    let _ = dir;
    // SAFETY: the caller keeps the arguments alive for the handle.
    unsafe { ffi::pam_start(service.as_ptr(), user.as_ptr(), conversation, handle) }
}

/// What the conversation needs: the password, given once, and what PAM
/// says along the way.
struct Talk<'a> {
    password: &'a Password,
    answered: bool,
    messages: Vec<String>,
}

/// PAM's questions: the password for the first hidden one, nothing for a
/// second (one password answers one question), and what it says kept for
/// the screen. A question that shows what's typed, like a user name, gets
/// no answer either: the lock knows only the password.
extern "C" fn converse(
    count: c_int,
    messages: *mut *const ffi::PamMessage,
    responses: *mut *mut ffi::PamResponse,
    data: *mut c_void,
) -> c_int {
    let Ok(count) = usize::try_from(count) else {
        return ffi::PAM_CONV_ERR;
    };
    if count == 0 || messages.is_null() || responses.is_null() || data.is_null() {
        return ffi::PAM_CONV_ERR;
    }
    // SAFETY: `data` is the `Talk` `check` passed, alive and only used here
    // while pam_authenticate runs on that thread.
    let talk = unsafe { &mut *data.cast::<Talk<'_>>() };
    // SAFETY: calloc gives zeroed memory, which PAM frees with free(): every
    // `resp` starts null.
    let replies =
        unsafe { libc::calloc(count, size_of::<ffi::PamResponse>()) }.cast::<ffi::PamResponse>();
    if replies.is_null() {
        return ffi::PAM_BUF_ERR;
    }
    let fail = |replies: *mut ffi::PamResponse| {
        for index in 0..count {
            // SAFETY: `replies` holds `count` entries; a filled `resp` is a
            // malloc'd copy of the password, wiped before it's freed.
            unsafe {
                let reply = &mut *replies.add(index);
                if !reply.resp.is_null() {
                    let length = libc::strlen(reply.resp);
                    std::ptr::write_bytes(reply.resp, 0, length);
                    libc::free(reply.resp.cast());
                }
            }
        }
        // SAFETY: allocated by calloc above.
        unsafe { libc::free(replies.cast()) };
        ffi::PAM_CONV_ERR
    };
    for index in 0..count {
        // Linux-PAM passes an array of pointers to messages.
        // SAFETY: PAM gives `count` valid message pointers.
        let message = unsafe { &**messages.add(index) };
        let text = if message.msg.is_null() {
            String::new()
        } else {
            // SAFETY: PAM's messages are NUL-terminated strings.
            unsafe { CStr::from_ptr(message.msg) }
                .to_string_lossy()
                .trim()
                .to_owned()
        };
        match message.msg_style {
            ffi::PAM_PROMPT_ECHO_OFF if !talk.answered => {
                talk.answered = true;
                let bytes = &talk.password.0;
                // SAFETY: room for the password and its NUL; PAM frees it,
                // and its modules wipe it first.
                let copy = unsafe { libc::malloc(bytes.len() + 1) }.cast::<u8>();
                if copy.is_null() {
                    return fail(replies);
                }
                // SAFETY: `copy` has room for the bytes and the NUL.
                unsafe {
                    std::ptr::copy_nonoverlapping(bytes.as_ptr(), copy, bytes.len());
                    *copy.add(bytes.len()) = 0;
                    (*replies.add(index)).resp = copy.cast();
                }
            }
            ffi::PAM_PROMPT_ECHO_OFF | ffi::PAM_PROMPT_ECHO_ON => return fail(replies),
            ffi::PAM_ERROR_MSG | ffi::PAM_TEXT_INFO => {
                if !text.is_empty() {
                    talk.messages.push(text);
                }
            }
            _ => return fail(replies),
        }
    }
    // SAFETY: PAM takes the replies and frees them.
    unsafe { *responses = replies };
    ffi::PAM_SUCCESS
}

/// The libpam calls and constants used, from `security/_pam_types.h`.
mod ffi {
    use std::ffi::{c_char, c_int, c_void};

    pub const PAM_SUCCESS: c_int = 0;
    pub const PAM_BUF_ERR: c_int = 5;
    pub const PAM_CONV_ERR: c_int = 19;
    pub const PAM_DISALLOW_NULL_AUTHTOK: c_int = 0x0001;
    pub const PAM_PROMPT_ECHO_OFF: c_int = 1;
    pub const PAM_PROMPT_ECHO_ON: c_int = 2;
    pub const PAM_ERROR_MSG: c_int = 3;
    pub const PAM_TEXT_INFO: c_int = 4;

    #[repr(C)]
    pub struct PamMessage {
        pub msg_style: c_int,
        pub msg: *const c_char,
    }

    #[repr(C)]
    pub struct PamResponse {
        pub resp: *mut c_char,
        pub resp_retcode: c_int,
    }

    pub type Converse =
        extern "C" fn(c_int, *mut *const PamMessage, *mut *mut PamResponse, *mut c_void) -> c_int;

    #[repr(C)]
    pub struct PamConv {
        pub conv: Option<Converse>,
        pub appdata_ptr: *mut c_void,
    }

    #[link(name = "pam")]
    unsafe extern "C" {
        pub fn pam_start(
            service: *const c_char,
            user: *const c_char,
            conversation: *const PamConv,
            handle: *mut *mut c_void,
        ) -> c_int;
        #[cfg(feature = "test-pam")]
        pub fn pam_start_confdir(
            service: *const c_char,
            user: *const c_char,
            conversation: *const PamConv,
            dir: *const c_char,
            handle: *mut *mut c_void,
        ) -> c_int;
        pub fn pam_authenticate(handle: *mut c_void, flags: c_int) -> c_int;
        pub fn pam_end(handle: *mut c_void, status: c_int) -> c_int;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_password_takes_characters_away_whole_and_never_moves() {
        let mut password = Password::default();
        let start = password.0.as_ptr();
        assert!(password.push("pa"));
        assert!(password.push("ß"));
        assert!(password.push("🔑"));
        assert_eq!(password.chars(), 4);
        password.pop();
        assert_eq!(password.chars(), 3);
        password.pop();
        assert_eq!(&password.0[..], b"pa");
        assert!(!password.push(&"x".repeat(CAPACITY)));
        assert_eq!(
            &password.0[..],
            b"pa",
            "a push that doesn't fit changes nothing"
        );
        assert_eq!(password.0.as_ptr(), start, "it never moved");
        assert!(format!("{password:?}").contains("2 characters"));
        assert!(!format!("{password:?}").contains("pa"));
        let taken = password.take();
        assert!(password.is_empty());
        assert_eq!(taken.chars(), 2);
    }

    #[test]
    fn a_missing_or_odd_service_is_no_service() {
        assert!(!has_service(""));
        assert!(!has_service("../passwd"));
        assert!(!has_service("mochi-lock-surely-not-installed"));
    }

    #[test]
    fn knows_who_runs_it() {
        assert!(user().is_some_and(|name| !name.is_empty()));
    }

    /// Real PAM, from a directory of test services: one that refuses
    /// everyone, one that lets everyone in, and one that takes "sesame"
    /// only, through pam_exec. One test, since the directory is the
    /// process's environment.
    #[cfg(feature = "test-pam")]
    #[test]
    fn pam_says_yes_only_when_its_service_does() {
        let output = std::process::Command::new("pkg-config")
            .args(["--variable=libdir", "pam"])
            .output()
            .expect("pkg-config finds PAM for these tests");
        let modules =
            PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()).join("security");
        assert!(
            modules.join("pam_deny.so").is_file(),
            "{}",
            modules.display()
        );
        let dir = std::env::temp_dir().join(format!("mochi-lock-pam-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let module = |name: &str| modules.join(name).display().to_string();
        let script = dir.join("sesame.sh");
        // pam_exec gives no PATH: shell builtins only. The password comes
        // with a NUL after it, which `read` drops.
        std::fs::write(
            &script,
            "#!/bin/sh\nIFS= read -r typed 2>/dev/null\n[ \"$typed\" = sesame ]\n",
        )
        .unwrap();
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let service = |name: &str, line: String| std::fs::write(dir.join(name), line).unwrap();
        service("deny", format!("auth required {}\n", module("pam_deny.so")));
        service(
            "permit",
            format!("auth required {}\n", module("pam_permit.so")),
        );
        service(
            "sesame",
            format!(
                "auth required {} expose_authtok quiet {}\n",
                module("pam_exec.so"),
                script.display()
            ),
        );
        // SAFETY: the only test that touches the environment.
        unsafe { std::env::set_var("MOCHI_LOCK_PAM_DIR", &dir) };

        let typed = |text: &str| {
            let mut password = Password::default();
            password.push(text);
            password
        };
        let me = user().unwrap();
        assert!(has_service("deny"));
        assert!(!has_service("missing"));
        assert!(check("deny", &me, typed("anything")).is_err());
        assert!(check("permit", &me, typed("anything")).is_ok());
        assert!(check("sesame", &me, typed("open")).is_err());
        assert!(check("sesame", &me, typed("sesame ")).is_err());
        assert!(check("sesame", &me, typed("sesame")).is_ok());
        // An empty password is refused, even by a service that lets
        // everyone in.
        assert!(check("permit", &me, Password::default()).is_err());

        // SAFETY: as above.
        unsafe { std::env::remove_var("MOCHI_LOCK_PAM_DIR") };
        std::fs::remove_dir_all(dir).unwrap();
    }
}
