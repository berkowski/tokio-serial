use futures::ready;
use tokio::io::unix::AsyncFd;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use std::io::{self, Read, Write};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

#[cfg(feature = "codec")]
pub mod frame;

/// Serial port I/O struct.
#[derive(Debug)]
pub struct TTYPort {
    io: AsyncFd<mio_serial::TTYPort>,
}

impl TTYPort {
    /// Open serial port from a provided path, using the default reactor.
    pub fn open(builder: &crate::SerialPortBuilder) -> crate::Result<Self> {
        let port = mio_serial::TTYPort::open(builder)?;

        Ok(Self {
            io: AsyncFd::new(port)?,
        })
    }

    /// Create a pair of pseudo serial terminals using the default reactor
    ///
    /// ## Returns
    /// Two connected, unnamed `Serial` objects.
    ///
    /// ## Errors
    /// Attempting any IO or parameter settings on the slave tty after the master
    /// tty is closed will return errors.
    ///
    #[cfg(unix)]
    pub fn pair() -> crate::Result<(Self, Self)> {
        let (master, slave) = mio_serial::TTYPort::pair()?;

        let master = TTYPort {
            io: AsyncFd::new(master)?,
        };
        let slave = TTYPort {
            io: AsyncFd::new(slave)?,
        };
        Ok((master, slave))
    }

    /// Sets the exclusivity of the port
    ///
    /// If a port is exclusive, then trying to open the same device path again
    /// will fail.
    ///
    /// See the man pages for the tiocexcl and tiocnxcl ioctl's for more details.
    ///
    /// ## Errors
    ///
    /// * `Io` for any error while setting exclusivity for the port.
    #[cfg(unix)]
    pub fn set_exclusive(&mut self, exclusive: bool) -> crate::Result<()> {
        self.io.get_mut().set_exclusive(exclusive)
    }

    /// Returns the exclusivity of the port
    ///
    /// If a port is exclusive, then trying to open the same device path again
    /// will fail.
    #[cfg(unix)]
    pub fn exclusive(&self) -> bool {
        self.io.get_ref().exclusive()
    }

    /// Read bytes on the serial port. On success, returns the number of bytes read.
    ///
    /// The function must be called with valid byte array `buf` of sufficient
    /// size to hold the message bytes. If a message is too long to fit in the
    /// supplied buffer, excess bytes may be discarded.
    ///
    pub async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut guard = self.io.readable_mut().await?;
        guard
            .try_io(|io| io.get_ref().read(buf))
            .unwrap_or(Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "read would block",
            )))
    }

    /// Attempts to ready bytes on the serial port.
    ///
    /// Note that on multiple calls to a `poll_*` method in the read direction, only the
    /// `Waker` from the `Context` passed to the most recent call will be scheduled to
    /// receive a wakeup.
    ///
    /// # Return value
    ///
    /// The function returns:
    ///
    /// * `Poll::Pending` if the socket is not ready to read
    /// * `Poll::Ready(Ok(()))` reads data `ReadBuf` if the socket is ready
    /// * `Poll::Ready(Err(e))` if an error is encountered.
    ///
    /// # Errors
    ///
    /// This function may encounter any standard I/O error except `WouldBlock`.
    pub fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        loop {
            let mut guard = ready!(self.io.poll_read_ready(cx))?;

            match guard.try_io(|io| io.get_ref().read(buf.initialize_unfilled())) {
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

    /// Try to read bytes on the serial port.  On success returns the number of bytes read.
    ///
    /// The function must be called with valid byte array `buf` of sufficient
    /// size to hold the message bytes. If a message is too long to fit in the
    /// supplied buffer, excess bytes may be discarded.
    ///
    /// When there is no pending data, `Err(io::ErrorKind::WouldBlock)` is
    /// returned. This function is usually paired with `readable()`.
    pub fn try_read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.io.get_mut().read(buf)
    }

    /// Wait for the port to become readable.
    ///
    /// This function is usually paired with `try_read()`.
    ///
    /// The function may complete without the socket being readable. This is a
    /// false-positive and attempting a `try_read()` will return with
    /// `io::ErrorKind::WouldBlock`.
    pub async fn readable(&self) -> io::Result<()> {
        let _ = self.io.readable().await?;
        Ok(())
    }

    /// Write bytes on the serial port. On success, returns the number of bytes written.
    pub async fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut guard = self.io.writable_mut().await?;
        guard
            .try_io(|io| io.get_ref().write(buf))
            .unwrap_or(Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "write would block",
            )))
    }

    /// Attempts to send data on the serial port
    ///
    /// Note that on multiple calls to a `poll_*` method in the send direction,
    /// only the `Waker` from the `Context` passed to the most recent call will
    /// be scheduled to receive a wakeup.
    ///
    /// # Return value
    ///
    /// The function returns:
    ///
    /// * `Poll::Pending` if the socket is not available to write
    /// * `Poll::Ready(Ok(n))` `n` is the number of bytes sent
    /// * `Poll::Ready(Err(e))` if an error is encountered.
    ///
    /// # Errors
    ///
    /// This function may encounter any standard I/O error except `WouldBlock`.
    pub fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        loop {
            let mut guard = ready!(self.io.poll_write_ready(cx))?;

            match guard.try_io(|io| io.get_ref().write(buf)) {
                Ok(result) => return Poll::Ready(result),
                Err(_would_block) => continue,
            }
        }
    }

    /// Try to write bytes on the serial port.  On success returns the number of bytes written.
    ///
    /// When the write would block, `Err(io::ErrorKind::WouldBlock)` is
    /// returned. This function is usually paired with `writable()`.
    pub fn try_write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.io.get_mut().write(buf)
    }

    /// Wait for the port to become writable.
    ///
    /// This function is usually paired with `try_write()`.
    ///
    /// The function may complete without the socket being readable. This is a
    /// false-positive and attempting a `try_write()` will return with
    /// `io::ErrorKind::WouldBlock`.
    pub async fn writable(&self) -> io::Result<()> {
        let _ = self.io.writable().await?;
        Ok(())
    }
}

impl AsyncRead for TTYPort {
    #[inline(always)]
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.poll_read(cx, buf)
    }
}

impl AsyncWrite for TTYPort {
    #[inline(always)]
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        loop {
            let mut guard = ready!(self.io.poll_write_ready(cx))?;
            match guard.try_io(|io| io.get_ref().flush()) {
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

impl crate::SerialPort for TTYPort {
    #[inline(always)]
    fn name(&self) -> Option<String> {
        self.io.get_ref().name()
    }

    #[inline(always)]
    fn baud_rate(&self) -> crate::Result<u32> {
        self.io.get_ref().baud_rate()
    }

    #[inline(always)]
    fn data_bits(&self) -> crate::Result<crate::DataBits> {
        self.io.get_ref().data_bits()
    }

    #[inline(always)]
    fn flow_control(&self) -> crate::Result<crate::FlowControl> {
        self.io.get_ref().flow_control()
    }

    #[inline(always)]
    fn parity(&self) -> crate::Result<crate::Parity> {
        self.io.get_ref().parity()
    }

    #[inline(always)]
    fn stop_bits(&self) -> crate::Result<crate::StopBits> {
        self.io.get_ref().stop_bits()
    }

    #[inline(always)]
    fn timeout(&self) -> Duration {
        Duration::from_secs(0)
    }

    #[inline(always)]
    fn set_baud_rate(&mut self, baud_rate: u32) -> crate::Result<()> {
        self.io.get_mut().set_baud_rate(baud_rate)
    }

    #[inline(always)]
    fn set_data_bits(&mut self, data_bits: crate::DataBits) -> crate::Result<()> {
        self.io.get_mut().set_data_bits(data_bits)
    }

    #[inline(always)]
    fn set_flow_control(&mut self, flow_control: crate::FlowControl) -> crate::Result<()> {
        self.io.get_mut().set_flow_control(flow_control)
    }

    #[inline(always)]
    fn set_parity(&mut self, parity: crate::Parity) -> crate::Result<()> {
        self.io.get_mut().set_parity(parity)
    }

    #[inline(always)]
    fn set_stop_bits(&mut self, stop_bits: crate::StopBits) -> crate::Result<()> {
        self.io.get_mut().set_stop_bits(stop_bits)
    }

    #[inline(always)]
    fn set_timeout(&mut self, _: Duration) -> crate::Result<()> {
        Ok(())
    }

    #[inline(always)]
    fn write_request_to_send(&mut self, level: bool) -> crate::Result<()> {
        self.io.get_mut().write_request_to_send(level)
    }

    #[inline(always)]
    fn write_data_terminal_ready(&mut self, level: bool) -> crate::Result<()> {
        self.io.get_mut().write_data_terminal_ready(level)
    }

    #[inline(always)]
    fn read_clear_to_send(&mut self) -> crate::Result<bool> {
        self.io.get_mut().read_clear_to_send()
    }

    #[inline(always)]
    fn read_data_set_ready(&mut self) -> crate::Result<bool> {
        self.io.get_mut().read_data_set_ready()
    }

    #[inline(always)]
    fn read_ring_indicator(&mut self) -> crate::Result<bool> {
        self.io.get_mut().read_ring_indicator()
    }

    #[inline(always)]
    fn read_carrier_detect(&mut self) -> crate::Result<bool> {
        self.io.get_mut().read_carrier_detect()
    }

    #[inline(always)]
    fn bytes_to_read(&self) -> crate::Result<u32> {
        self.io.get_ref().bytes_to_read()
    }

    #[inline(always)]
    fn bytes_to_write(&self) -> crate::Result<u32> {
        self.io.get_ref().bytes_to_write()
    }

    #[inline(always)]
    fn clear(&self, buffer_to_clear: crate::ClearBuffer) -> crate::Result<()> {
        self.io.get_ref().clear(buffer_to_clear)
    }

    #[inline(always)]
    fn try_clone(&self) -> crate::Result<Box<dyn crate::SerialPort>> {
        Err(crate::Error::new(
            crate::ErrorKind::Io(std::io::ErrorKind::Other),
            "Cannot clone Tokio handles",
        ))
    }

    #[inline(always)]
    fn set_break(&self) -> crate::Result<()> {
        self.io.get_ref().set_break()
    }

    #[inline(always)]
    fn clear_break(&self) -> crate::Result<()> {
        self.io.get_ref().clear_break()
    }
}

impl Read for TTYPort {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.io.get_mut().read(buf)
    }
}

impl Write for TTYPort {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.io.get_mut().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.io.get_mut().flush()
    }
}

#[cfg(unix)]
use std::os::unix::io::{AsRawFd, RawFd};
#[cfg(unix)]
impl AsRawFd for TTYPort {
    fn as_raw_fd(&self) -> RawFd {
        self.io.get_ref().as_raw_fd()
    }
}
