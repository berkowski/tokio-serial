use std::convert::TryFrom;
use std::io::{self, Read, Write};
use std::os::unix::io::{AsRawFd, RawFd};
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::ready;
use nix::libc;
use nix::sys::termios::{self, SetArg, SpecialCharacterIndices};
use serialport::{SerialPortBuilder, TTYPort};
use tokio::io::unix::AsyncFd;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

#[derive(Debug)]
pub(crate) struct UnixSerialStream {
    inner: AsyncFd<TTYPort>,
}

impl UnixSerialStream {
    pub fn open(builder: &SerialPortBuilder) -> crate::Result<Self> {
        let tty = TTYPort::open(builder)?;
        Self::try_from(tty)
    }

    pub fn pair() -> crate::Result<(Self, Self)> {
        let (primary, secondary) = TTYPort::pair()?;
        let primary = Self::try_from(primary)?;
        let secondary = Self::try_from(secondary)?;

        Ok((primary, secondary))
    }

    pub fn get_ref(&self) -> &TTYPort {
        self.inner.get_ref()
    }

    pub fn get_mut(&mut self) -> &mut TTYPort {
        self.inner.get_mut()
    }

    pub async fn readable(&self) -> io::Result<()> {
        let _ = self.inner.readable().await?;
        Ok(())
    }

    pub async fn writable(&self) -> io::Result<()> {
        let _ = self.inner.writable().await?;
        Ok(())
    }
}

// [CF] Copied from https://github.com/berkowski/mio-serial/blob/bd5c81959fa9a77691faed637761ec96311e9115/src/unix.rs#L370-L390
impl TryFrom<TTYPort> for UnixSerialStream {
    type Error = crate::Error;

    fn try_from(tty: TTYPort) -> crate::Result<Self> {
        let mut t = termios::tcgetattr(tty.as_raw_fd()).map_err(map_nix_error)?;

        // Set VMIN = 1 to block until at least one character is received.
        t.control_chars[SpecialCharacterIndices::VMIN as usize] = 1;
        termios::tcsetattr(tty.as_raw_fd(), SetArg::TCSANOW, &t).map_err(map_nix_error)?;

        // Set the O_NONBLOCK flag.
        let flags = unsafe { libc::fcntl(tty.as_raw_fd(), libc::F_GETFL) };
        if flags < 0 {
            return Err(io::Error::last_os_error().into());
        }

        let error =
            unsafe { libc::fcntl(tty.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) };
        if error != 0 {
            return Err(io::Error::last_os_error().into());
        }

        let inner = AsyncFd::new(tty)?;
        Ok(Self { inner })
    }
}

impl AsRawFd for UnixSerialStream {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

impl Read for UnixSerialStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        read_from_fd(self.as_raw_fd(), buf)
    }
}

impl Write for UnixSerialStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        write_to_fd(self.as_raw_fd(), buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        flush_fd(self.as_raw_fd())
    }
}

impl AsyncRead for UnixSerialStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        loop {
            let mut guard = ready!(self.inner.poll_read_ready(cx))?;

            match guard.try_io(|inner| read_from_fd(inner.as_raw_fd(), buf.initialize_unfilled())) {
                Ok(Ok(bytes_read)) => {
                    buf.advance(bytes_read);
                    return Poll::Ready(Ok(()));
                }
                Ok(Err(err)) => {
                    return Poll::Ready(Err(err));
                }
                Err(_would_block) => continue,
            }
        }
    }
}

impl AsyncWrite for UnixSerialStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        loop {
            let mut guard = ready!(self.inner.poll_write_ready(cx))?;

            match guard.try_io(|inner| write_to_fd(inner.as_raw_fd(), buf)) {
                Ok(result) => return Poll::Ready(result),
                Err(_would_block) => continue,
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        loop {
            let mut guard = ready!(self.inner.poll_write_ready(cx))?;
            match guard.try_io(|inner| flush_fd(inner.as_raw_fd())) {
                Ok(_) => return Poll::Ready(Ok(())),
                Err(_would_block) => continue,
            }
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let _ = self.poll_flush(cx)?;
        Ok(()).into()
    }
}

fn map_nix_error(e: nix::Error) -> crate::Error {
    crate::Error {
        kind: crate::ErrorKind::Io(io::ErrorKind::Other),
        description: e.to_string(),
    }
}

macro_rules! uninterruptibly {
    ($e:expr) => {{
        loop {
            match $e {
                Err(ref error) if error.kind() == io::ErrorKind::Interrupted => {}
                res => break res,
            }
        }
    }};
}

// [CF] Copied from https://github.com/berkowski/mio-serial/blob/bd5c81959fa9a77691faed637761ec96311e9115/src/unix.rs#L442-L455
fn read_from_fd(fd: RawFd, buf: &mut [u8]) -> io::Result<usize> {
    uninterruptibly!(match unsafe {
        libc::read(
            fd,
            buf.as_ptr() as *mut libc::c_void,
            buf.len() as libc::size_t,
        )
    } {
        x if x >= 0 => Ok(x as usize),
        _ => Err(io::Error::last_os_error()),
    })
}

// [CF] Copied from https://github.com/berkowski/mio-serial/blob/bd5c81959fa9a77691faed637761ec96311e9115/src/unix.rs#L457-L479
fn write_to_fd(fd: RawFd, buf: &[u8]) -> io::Result<usize> {
    uninterruptibly!(match unsafe {
        libc::write(
            fd,
            buf.as_ptr() as *const libc::c_void,
            buf.len() as libc::size_t,
        )
    } {
        x if x >= 0 => Ok(x as usize),
        _ => Err(io::Error::last_os_error()),
    })
}

fn flush_fd(fd: RawFd) -> io::Result<()> {
    uninterruptibly!(termios::tcdrain(fd).map_err(|errno| io::Error::from(errno)))
}
