//! Bindings for serial port I/O and futures
//!
//! This crate provides bindings between `mio_serial`, a mio crate for
//! serial port I/O, and `futures`.  The API is very similar to the
//! bindings in `mio_serial`
//!
#![deny(missing_docs)]
#![warn(rust_2018_idioms)]

// Re-export serialport types and traits from mio_serial
pub use mio_serial::{
    available_ports, new, ClearBuffer, DataBits, Error, ErrorKind, FlowControl, Parity, SerialPort,
    SerialPortBuilder, SerialPortInfo, SerialPortType, StopBits, UsbPortInfo,
};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use std::convert::TryFrom;
use std::io::{Read, Result as IoResult, Write};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

#[cfg(feature = "codec")]
pub mod frame;

#[cfg(unix)]
mod os_prelude {
    pub use futures::ready;
    pub use tokio::io::unix::AsyncFd;
}

#[cfg(windows)]
mod os_prelude {
    pub use std::mem;
    pub use std::ops::{Deref, DerefMut};
    pub use std::os::windows::prelude::*;
    pub use tokio::net::windows::named_pipe;
}

use crate::os_prelude::*;

/// A type for results generated by interacting with serial ports.
pub type Result<T> = mio_serial::Result<T>;

/// Async serial port I/O
///
/// Reading and writing to a `SerialStream` is usually done using the
/// convenience methods found on the [`tokio::io::AsyncReadExt`] and [`tokio::io::AsyncWriteExt`]
/// traits.
///
/// [`AsyncReadExt`]: trait@tokio::io::AsyncReadExt
/// [`AsyncWriteExt`]: trait@tokio::io::AsyncWriteExt
///
#[derive(Debug)]
pub struct SerialStream {
    #[cfg(unix)]
    inner: AsyncFd<mio_serial::SerialStream>,
    // Named pipes and COM ports are actually two entirely different things that hardly have anything in common.
    // The only thing they share is the opaque `HANDLE` type that can be fed into `CreateFileW`, `ReadFile`, `WriteFile`, etc.
    //
    // Both `mio` and `tokio` don't yet have any code to work on arbitrary HANDLEs.
    // But they have code for dealing with named pipes, and we (ab)use that here to work on COM ports.
    #[cfg(windows)]
    inner: named_pipe::NamedPipeClient,
    // The com port is kept around for serialport related methods
    #[cfg(windows)]
    com: mem::ManuallyDrop<mio_serial::SerialStream>,
}

impl SerialStream {
    /// Open serial port from a provided path, using the default reactor.
    pub fn open(builder: &crate::SerialPortBuilder) -> crate::Result<Self> {
        let port = mio_serial::SerialStream::open(builder)?;

        #[cfg(unix)]
        {
            Ok(Self {
                inner: AsyncFd::new(port)?,
            })
        }

        #[cfg(windows)]
        {
            let handle = port.as_raw_handle();
            // Keep the com port around to use for serialport related things
            let com = mem::ManuallyDrop::new(port);
            Ok(Self {
                inner: unsafe { named_pipe::NamedPipeClient::from_raw_handle(handle)? },
                com,
            })
        }
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
        let (master, slave) = mio_serial::SerialStream::pair()?;

        let master = SerialStream {
            inner: AsyncFd::new(master)?,
        };
        let slave = SerialStream {
            inner: AsyncFd::new(slave)?,
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
        self.inner.get_mut().set_exclusive(exclusive)
    }

    /// Returns the exclusivity of the port
    ///
    /// If a port is exclusive, then trying to open the same device path again
    /// will fail.
    #[cfg(unix)]
    pub fn exclusive(&self) -> bool {
        self.inner.get_ref().exclusive()
    }

    /// Borrow a reference to the underlying mio-serial::SerialStream object.
    #[inline(always)]
    fn borrow(&self) -> &mio_serial::SerialStream {
        #[cfg(unix)]
        {
            self.inner.get_ref()
        }
        #[cfg(windows)]
        {
            self.com.deref()
        }
    }

    /// Borrow a mutable reference to the underlying mio-serial::SerialStream object.
    #[inline(always)]
    fn borrow_mut(&mut self) -> &mut mio_serial::SerialStream {
        #[cfg(unix)]
        {
            self.inner.get_mut()
        }
        #[cfg(windows)]
        {
            self.com.deref_mut()
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
    pub fn try_read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        #[cfg(unix)]
        {
            self.inner.get_mut().read(buf)
        }
        #[cfg(windows)]
        {
            self.inner.try_read(buf)
        }
    }

    /// Wait for the port to become readable.
    ///
    /// This function is usually paired with `try_read()`.
    ///
    /// The function may complete without the socket being readable. This is a
    /// false-positive and attempting a `try_read()` will return with
    /// `io::ErrorKind::WouldBlock`.
    pub async fn readable(&self) -> IoResult<()> {
        let _ = self.inner.readable().await?;
        Ok(())
    }

    /// Try to write bytes on the serial port.  On success returns the number of bytes written.
    ///
    /// When the write would block, `Err(io::ErrorKind::WouldBlock)` is
    /// returned. This function is usually paired with `writable()`.
    pub fn try_write(&mut self, buf: &[u8]) -> IoResult<usize> {
        #[cfg(unix)]
        {
            self.inner.get_mut().write(buf)
        }
        #[cfg(windows)]
        {
            self.inner.try_write(buf)
        }
    }

    /// Wait for the port to become writable.
    ///
    /// This function is usually paired with `try_write()`.
    ///
    /// The function may complete without the socket being readable. This is a
    /// false-positive and attempting a `try_write()` will return with
    /// `io::ErrorKind::WouldBlock`.
    pub async fn writable(&self) -> IoResult<()> {
        let _ = self.inner.writable().await?;
        Ok(())
    }
}

#[cfg(unix)]
impl AsyncRead for SerialStream {
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
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<IoResult<()>> {
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

#[cfg(unix)]
impl AsyncWrite for SerialStream {
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
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<IoResult<usize>> {
        loop {
            let mut guard = ready!(self.inner.poll_write_ready(cx))?;

            match guard.try_io(|inner| inner.get_ref().write(buf)) {
                Ok(result) => return Poll::Ready(result),
                Err(_would_block) => continue,
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        loop {
            let mut guard = ready!(self.inner.poll_write_ready(cx))?;
            match guard.try_io(|inner| inner.get_ref().flush()) {
                Ok(_) => return Poll::Ready(Ok(())),
                Err(_would_block) => continue,
            }
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        let _ = self.poll_flush(cx)?;
        Ok(()).into()
    }
}

#[cfg(windows)]
impl AsyncRead for SerialStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<IoResult<()>> {
        let mut self_ = self;
        Pin::new(&mut self_.inner).poll_read(cx, buf)
    }
}

#[cfg(windows)]
impl AsyncWrite for SerialStream {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<IoResult<usize>> {
        let mut self_ = self;
        Pin::new(&mut self_.inner).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        let mut self_ = self;
        Pin::new(&mut self_.inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        let mut self_ = self;
        Pin::new(&mut self_.inner).poll_shutdown(cx)
    }
}

impl crate::SerialPort for SerialStream {
    #[inline(always)]
    fn name(&self) -> Option<String> {
        self.borrow().name()
    }

    #[inline(always)]
    fn baud_rate(&self) -> crate::Result<u32> {
        self.borrow().baud_rate()
    }

    #[inline(always)]
    fn data_bits(&self) -> crate::Result<crate::DataBits> {
        self.borrow().data_bits()
    }

    #[inline(always)]
    fn flow_control(&self) -> crate::Result<crate::FlowControl> {
        self.borrow().flow_control()
    }

    #[inline(always)]
    fn parity(&self) -> crate::Result<crate::Parity> {
        self.borrow().parity()
    }

    #[inline(always)]
    fn stop_bits(&self) -> crate::Result<crate::StopBits> {
        self.borrow().stop_bits()
    }

    #[inline(always)]
    fn timeout(&self) -> Duration {
        Duration::from_secs(0)
    }

    #[inline(always)]
    fn set_baud_rate(&mut self, baud_rate: u32) -> crate::Result<()> {
        self.borrow_mut().set_baud_rate(baud_rate)
    }

    #[inline(always)]
    fn set_data_bits(&mut self, data_bits: crate::DataBits) -> crate::Result<()> {
        self.borrow_mut().set_data_bits(data_bits)
    }

    #[inline(always)]
    fn set_flow_control(&mut self, flow_control: crate::FlowControl) -> crate::Result<()> {
        self.borrow_mut().set_flow_control(flow_control)
    }

    #[inline(always)]
    fn set_parity(&mut self, parity: crate::Parity) -> crate::Result<()> {
        self.borrow_mut().set_parity(parity)
    }

    #[inline(always)]
    fn set_stop_bits(&mut self, stop_bits: crate::StopBits) -> crate::Result<()> {
        self.borrow_mut().set_stop_bits(stop_bits)
    }

    #[inline(always)]
    fn set_timeout(&mut self, _: Duration) -> crate::Result<()> {
        Ok(())
    }

    #[inline(always)]
    fn write_request_to_send(&mut self, level: bool) -> crate::Result<()> {
        self.borrow_mut().write_request_to_send(level)
    }

    #[inline(always)]
    fn write_data_terminal_ready(&mut self, level: bool) -> crate::Result<()> {
        self.borrow_mut().write_data_terminal_ready(level)
    }

    #[inline(always)]
    fn read_clear_to_send(&mut self) -> crate::Result<bool> {
        self.borrow_mut().read_clear_to_send()
    }

    #[inline(always)]
    fn read_data_set_ready(&mut self) -> crate::Result<bool> {
        self.borrow_mut().read_data_set_ready()
    }

    #[inline(always)]
    fn read_ring_indicator(&mut self) -> crate::Result<bool> {
        self.borrow_mut().read_ring_indicator()
    }

    #[inline(always)]
    fn read_carrier_detect(&mut self) -> crate::Result<bool> {
        self.borrow_mut().read_carrier_detect()
    }

    #[inline(always)]
    fn bytes_to_read(&self) -> crate::Result<u32> {
        self.borrow().bytes_to_read()
    }

    #[inline(always)]
    fn bytes_to_write(&self) -> crate::Result<u32> {
        self.borrow().bytes_to_write()
    }

    #[inline(always)]
    fn clear(&self, buffer_to_clear: crate::ClearBuffer) -> crate::Result<()> {
        self.borrow().clear(buffer_to_clear)
    }

    /// Cloning SerialStream is not supported.
    ///
    /// # Errors
    /// Always returns `ErrorKind::Other` with a message.
    #[inline(always)]
    fn try_clone(&self) -> crate::Result<Box<dyn crate::SerialPort>> {
        Err(crate::Error::new(
            crate::ErrorKind::Io(std::io::ErrorKind::Other),
            "Cannot clone Tokio handles",
        ))
    }

    #[inline(always)]
    fn set_break(&self) -> crate::Result<()> {
        self.borrow().set_break()
    }

    #[inline(always)]
    fn clear_break(&self) -> crate::Result<()> {
        self.borrow().clear_break()
    }
}

impl Read for SerialStream {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.try_read(buf)
    }
}

impl Write for SerialStream {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.try_write(buf)
    }

    fn flush(&mut self) -> IoResult<()> {
        self.borrow_mut().flush()
    }
}

#[cfg(unix)]
impl TryFrom<serialport::TTYPort> for SerialStream {
    type Error = Error;

    fn try_from(value: serialport::TTYPort) -> std::result::Result<Self, Self::Error> {
        let port = mio_serial::SerialStream::try_from(value)?;
        Ok(Self {
            inner: AsyncFd::new(port)?,
        })
    }
}

#[cfg(unix)]
mod sys {
    use super::SerialStream;
    use std::os::unix::io::{AsRawFd, RawFd};
    impl AsRawFd for SerialStream {
        fn as_raw_fd(&self) -> RawFd {
            self.inner.as_raw_fd()
        }
    }
}

#[cfg(windows)]
mod io {
    use super::SerialStream;
    use std::os::windows::io::{AsRawHandle, RawHandle};
    impl AsRawHandle for SerialStream {
        fn as_raw_handle(&self) -> RawHandle {
            self.inner.as_raw_handle()
        }
    }
}

/// An extension trait for serialport::SerialPortBuilder
///
/// This trait adds one method to SerialPortBuilder:
///
/// - open_native_async
///
/// This method mirrors the `open_native` method of SerialPortBuilder
pub trait SerialPortBuilderExt {
    /// Open a platform-specific interface to the port with the specified settings
    fn open_native_async(self) -> Result<SerialStream>;
}

impl SerialPortBuilderExt for SerialPortBuilder {
    /// Open a platform-specific interface to the port with the specified settings
    fn open_native_async(self) -> Result<SerialStream> {
        SerialStream::open(&self)
    }
}
