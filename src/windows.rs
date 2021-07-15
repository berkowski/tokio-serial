use std::io::{self, Read, Write};
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};
use std::os::windows::io::{AsRawHandle, RawHandle};
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::windows::named_pipe::NamedPipeClient;

#[derive(Debug)]
pub(crate) struct WindowsSerialStream {
    // [EG] We use `ManuallyDrop` because both `COMPort` and `NamedPipeClient` take
    // ownership of the raw handle. To avoid closing the handle twice when
    // the `Serial` is dropped, we explicitly do **not** drop one of the
    // owned instances. Yes, it is a hack.
    inner: ManuallyDrop<mio_serial::SerialStream>,

    // [CF] Named pipes and COM ports are actually two entirely different things that hardly have anything in common.
    // The only thing they share is the opaque `HANDLE` type that can be fed into `CreateFileW`, `ReadFile`, `WriteFile`, etc.
    //
    // Both `mio` and `tokio` don't yet have any code to work on arbitrary HANDLEs.
    // But they have code for dealing with named pipes, and we (ab)use that here to work on COM ports.
    pipe: NamedPipeClient,
}

impl WindowsSerialStream {
    pub fn new(port: mio_serial::SerialStream) -> crate::Result<Self> {
        let handle = port.as_raw_handle();
        let inner = ManuallyDrop::new(port);
        let pipe = unsafe { NamedPipeClient::from_raw_handle(handle)? };

        Ok(Self { inner, pipe })
    }

    /// Get a reference to the underlying mio-serial::SerialStream object.
    pub fn get_ref(&self) -> &mio_serial::SerialStream {
        self.inner.deref()
    }

    /// Get a mutable reference to the underlying mio-serial::SerialStream object.
    pub fn get_mut(&mut self) -> &mut mio_serial::SerialStream {
        self.inner.deref_mut()
    }

    pub async fn readable(&self) -> io::Result<()> {
        self.pipe.readable().await
    }

    pub async fn writable(&self) -> io::Result<()> {
        self.pipe.writable().await
    }
}

impl AsRawHandle for WindowsSerialStream {
    fn as_raw_handle(&self) -> RawHandle {
        self.inner.as_raw_handle()
    }
}

impl Read for WindowsSerialStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.pipe.try_read(buf)
    }
}

impl Write for WindowsSerialStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.pipe.try_write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl AsyncRead for WindowsSerialStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().pipe).poll_read(cx, buf)
    }
}

impl AsyncWrite for WindowsSerialStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().pipe).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().pipe).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().pipe).poll_shutdown(cx)
    }
}
