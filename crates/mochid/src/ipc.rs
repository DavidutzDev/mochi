//! The Unix socket. Each connection gets a reader task and a writer task;
//! everything they receive goes to the daemon loop as a [`ConnectionEvent`],
//! so all state stays on the loop.

use std::io::{self, ErrorKind};
use std::os::fd::{AsRawFd, RawFd};
use std::path::Path;

use mochi_protocol::{ClientMessage, DaemonMessage, MAX_LINE};
use tokio::io::{AsyncWriteExt, Interest};
use tokio::net::unix::OwnedReadHalf;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc::{self, UnboundedSender};

pub type ConnectionId = u64;

/// A whole line and the process that wrote it.
type Line = (Vec<u8>, Option<u32>);

#[derive(Debug)]
pub enum ConnectionEvent {
    Opened {
        id: ConnectionId,
        sender: UnboundedSender<DaemonMessage>,
    },
    Message {
        id: ConnectionId,
        message: ClientMessage,
        /// The process that wrote it, as the kernel says (`SCM_CREDENTIALS`).
        /// Not the one that connected: that one may have handed the socket
        /// to another, or become another program since.
        pid: Option<u32>,
    },
    Malformed {
        id: ConnectionId,
        error: String,
    },
    Closed {
        id: ConnectionId,
    },
}

/// Binds the socket. Fails if another daemon answers on it, and replaces a
/// stale socket file left by one that crashed.
pub async fn bind(path: &Path) -> io::Result<UnixListener> {
    if UnixStream::connect(path).await.is_ok() {
        return Err(io::Error::new(
            ErrorKind::AddrInUse,
            format!("another mochid is running on {}", path.display()),
        ));
    }
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    UnixListener::bind(path)
}

pub fn accept_all(listener: UnixListener, events: UnboundedSender<ConnectionEvent>) {
    tokio::spawn(async move {
        for id in 0.. {
            match listener.accept().await {
                Ok((stream, _)) => {
                    if events.is_closed() {
                        return;
                    }
                    serve(id, stream, events.clone());
                }
                Err(error) => tracing::warn!(%error, "accept failed"),
            }
        }
    });
}

fn serve(id: ConnectionId, stream: UnixStream, events: UnboundedSender<ConnectionEvent>) {
    // The kernel says who wrote each message from now on.
    if let Err(error) = pass_credentials(stream.as_raw_fd()) {
        tracing::warn!(%error, "can't tell who writes on a connection");
    }
    let (reader, mut writer) = stream.into_split();
    let (sender, mut outgoing) = mpsc::unbounded_channel::<DaemonMessage>();

    // Sent before the reader starts, so the loop sees Opened first.
    if events.send(ConnectionEvent::Opened { id, sender }).is_err() {
        return;
    }

    // The writer ends when the loop drops this connection's sender.
    tokio::spawn(async move {
        while let Some(message) = outgoing.recv().await {
            let line = match mochi_protocol::encode(&message) {
                Ok(line) => line,
                Err(error) => {
                    tracing::error!(%error, "could not encode a message");
                    continue;
                }
            };
            if writer.write_all(&line).await.is_err() {
                break;
            }
        }
        let _ = writer.shutdown().await;
    });

    tokio::spawn(async move {
        let mut lines = Lines::default();
        let mut buffer = vec![0u8; 64 * 1024];
        loop {
            let (read, pid) = match receive(&reader, &mut buffer).await {
                Ok((0, _)) | Err(_) => break,
                Ok(received) => received,
            };
            let done = match lines.push(&buffer[..read], pid) {
                Ok(done) => done,
                Err(error) => {
                    let _ = events.send(ConnectionEvent::Malformed { id, error });
                    break;
                }
            };
            for (line, pid) in done {
                if events.send(event(id, &line, pid)).is_err() {
                    return;
                }
            }
        }
        // A last line without its newline still counts.
        if let Some((line, pid)) = lines.rest() {
            let _ = events.send(event(id, &line, pid));
        }
        let _ = events.send(ConnectionEvent::Closed { id });
    });
}

fn event(id: ConnectionId, line: &[u8], pid: Option<u32>) -> ConnectionEvent {
    match std::str::from_utf8(line)
        .map_err(|error| error.to_string())
        .and_then(|text| mochi_protocol::decode(text).map_err(|error| error.to_string()))
    {
        Ok(message) => ConnectionEvent::Message { id, message, pid },
        Err(error) => ConnectionEvent::Malformed { id, error },
    }
}

/// Splits what arrives into lines, each with the process that wrote it. A
/// line written by more than one process has none.
#[derive(Debug, Default)]
struct Lines {
    line: Vec<u8>,
    /// Who wrote the line so far: `None` before its first byte.
    writer: Option<Option<u32>>,
}

impl Lines {
    /// Takes in `bytes`, all written by `pid`; gives back the lines they
    /// end. `Err` once a line is longer than [`MAX_LINE`].
    fn push(&mut self, mut bytes: &[u8], pid: Option<u32>) -> Result<Vec<Line>, String> {
        let mut done = Vec::new();
        while !bytes.is_empty() {
            let end = bytes.iter().position(|byte| *byte == b'\n');
            let (part, rest) = bytes.split_at(end.map_or(bytes.len(), |end| end + 1));
            self.line.extend_from_slice(part);
            self.writer = Some(match self.writer {
                Some(writer) if writer != pid => None,
                _ => pid,
            });
            if self.line.len() > MAX_LINE {
                return Err(format!("line longer than {MAX_LINE} bytes"));
            }
            if end.is_some() {
                done.extend(self.rest());
            }
            bytes = rest;
        }
        Ok(done)
    }

    /// The line so far, if there's one, and who wrote it.
    fn rest(&mut self) -> Option<Line> {
        let writer = self.writer.take()?;
        Some((std::mem::take(&mut self.line), writer))
    }
}

/// Has the kernel attach the writer's credentials to what arrives on `fd`,
/// which it does for each write, so a process can't pass for another.
fn pass_credentials(fd: RawFd) -> io::Result<()> {
    let on: libc::c_int = 1;
    // SAFETY: `fd` is an open socket and `on` outlives the call.
    let result = unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_PASSCRED,
            (&raw const on).cast(),
            size_of::<libc::c_int>() as libc::socklen_t,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Reads what's there, and who wrote it. The kernel never hands over bytes
/// of two writers in one read.
async fn receive(reader: &OwnedReadHalf, buffer: &mut [u8]) -> io::Result<(usize, Option<u32>)> {
    let stream: &UnixStream = reader.as_ref();
    loop {
        stream.readable().await?;
        match stream.try_io(Interest::READABLE, || {
            receive_now(stream.as_raw_fd(), buffer)
        }) {
            Err(error) if error.kind() == ErrorKind::WouldBlock => {}
            result => return result,
        }
    }
}

fn receive_now(fd: RawFd, buffer: &mut [u8]) -> io::Result<(usize, Option<u32>)> {
    // Room for one `SCM_CREDENTIALS`, aligned as the kernel writes it.
    let mut control = [0u64; 8];
    let mut part = libc::iovec {
        iov_base: buffer.as_mut_ptr().cast(),
        iov_len: buffer.len(),
    };
    // SAFETY: an all-zero msghdr is valid; the pointers set below point at
    // `part` and `control`, which outlive the call.
    let mut header: libc::msghdr = unsafe { std::mem::zeroed() };
    header.msg_iov = &raw mut part;
    header.msg_iovlen = 1;
    header.msg_control = control.as_mut_ptr().cast();
    header.msg_controllen = size_of_val(&control) as _;
    // SAFETY: `header` describes memory this function owns.
    let read = unsafe { libc::recvmsg(fd, &raw mut header, libc::MSG_CMSG_CLOEXEC) };
    let read = usize::try_from(read).map_err(|_| io::Error::last_os_error())?;
    let mut pid = None;
    // SAFETY: the CMSG macros walk the control data recvmsg filled in,
    // within `msg_controllen`.
    unsafe {
        let mut message = libc::CMSG_FIRSTHDR(&raw const header);
        while !message.is_null() {
            if (*message).cmsg_level == libc::SOL_SOCKET
                && (*message).cmsg_type == libc::SCM_CREDENTIALS
            {
                let credentials: libc::ucred =
                    std::ptr::read_unaligned(libc::CMSG_DATA(message).cast());
                pid = u32::try_from(credentials.pid).ok().filter(|pid| *pid != 0);
            }
            message = libc::CMSG_NXTHDR(&raw const header, message);
        }
    }
    Ok((read, pid))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_lines_and_keeps_their_writer() {
        let mut lines = Lines::default();
        assert_eq!(
            lines.push(b"a\nb", Some(1)).unwrap(),
            [(b"a\n".to_vec(), Some(1))]
        );
        // The rest of "b" from another process: nobody wrote it alone.
        assert_eq!(
            lines.push(b"c\nd\n", Some(2)).unwrap(),
            [(b"bc\n".to_vec(), None), (b"d\n".to_vec(), Some(2)),]
        );
        assert_eq!(lines.rest(), None);
        lines.push(b"e", Some(3)).unwrap();
        assert_eq!(lines.rest(), Some((b"e".to_vec(), Some(3))));
    }

    #[test]
    fn a_line_too_long_is_refused() {
        let mut lines = Lines::default();
        assert!(lines.push(&vec![b'x'; MAX_LINE + 1], Some(1)).is_err());
    }

    /// The process that writes counts, not the one that connected: a child
    /// writing on its parent's connection is itself.
    #[tokio::test]
    async fn the_writer_is_the_process_that_wrote() {
        use std::os::unix::process::CommandExt;
        let (ours, theirs) = std::os::unix::net::UnixStream::pair().unwrap();
        let ours = UnixStream::from_std({
            ours.set_nonblocking(true).unwrap();
            ours
        })
        .unwrap();
        pass_credentials(ours.as_raw_fd()).unwrap();
        let (reader, _writer) = ours.into_split();
        let fd = theirs.as_raw_fd();
        let mut child = std::process::Command::new("sh");
        child.args(["-c", "printf 'hi\\n' >&3"]);
        // SAFETY: dup2 is async-signal-safe.
        unsafe {
            child.pre_exec(move || {
                if libc::dup2(fd, 3) == -1 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut child = child.spawn().unwrap();
        let mut buffer = [0u8; 16];
        let (read, pid) = receive(&reader, &mut buffer).await.unwrap();
        assert_eq!(&buffer[..read], b"hi\n");
        assert_eq!(pid, Some(child.id()));
        assert_ne!(pid, Some(std::process::id()));
        child.wait().unwrap();
    }
}
