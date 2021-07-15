use std::io::{self, Read, Write};
use std::os::unix::io::{AsRawFd, RawFd};
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::ready;
use tokio::io::unix::AsyncFd;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

#[derive(Debug)]
pub(crate) struct UnixSerialStream {
    inner: AsyncFd<mio_serial::SerialStream>,
}

impl UnixSerialStream {
    pub fn new(port: mio_serial::SerialStream) -> crate::Result<Self> {
        let inner = AsyncFd::new(port)?;
        Ok(Self { inner })
    }

    pub fn pair() -> crate::Result<(Self, Self)> {
        let (primary, secondary) = mio_serial::SerialStream::pair()?;
        let primary = Self::new(primary)?;
        let secondary = Self::new(secondary)?;

        Ok((primary, secondary))
    }

    /// Get a reference to the underlying mio-serial::SerialStream object.
    pub fn get_ref(&self) -> &mio_serial::SerialStream {
        self.inner.get_ref()
    }

    /// Get a mutable reference to the underlying mio-serial::SerialStream object.
    pub fn get_mut(&mut self) -> &mut mio_serial::SerialStream {
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

impl AsRawFd for UnixSerialStream {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

impl Read for UnixSerialStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.get_mut().read(buf)
    }
}

impl Write for UnixSerialStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.get_mut().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.get_mut().flush()
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

            match guard.try_io(|inner| inner.get_ref().read(buf.initialize_unfilled())) {
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

            match guard.try_io(|inner| inner.get_ref().write(buf)) {
                Ok(result) => return Poll::Ready(result),
                Err(_would_block) => continue,
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        loop {
            let mut guard = ready!(self.inner.poll_write_ready(cx))?;

            match guard.try_io(|inner| inner.get_ref().flush()) {
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
