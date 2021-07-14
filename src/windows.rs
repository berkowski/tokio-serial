use std::ffi::OsStr;
use std::io::{self, Read, Write};
use std::mem;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{FromRawHandle, RawHandle};
use std::path::Path;
use std::pin::Pin;
use std::ptr;
use std::task::{Context, Poll};

use serialport::{COMPort, SerialPort, SerialPortBuilder};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::windows::named_pipe::NamedPipeClient;
use winapi::um::commapi::SetCommTimeouts;
use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::um::winbase::{COMMTIMEOUTS, FILE_FLAG_OVERLAPPED};
use winapi::um::winnt::{FILE_ATTRIBUTE_NORMAL, GENERIC_READ, GENERIC_WRITE, HANDLE};

#[derive(Debug)]
pub(crate) struct WindowsSerialStream {
    inner: COMPort,

    // [CF] Named pipes and COM ports are actually two entirely different things that hardly have anything in common.
    // The only thing they share is the opaque `HANDLE` type that can be fed into `CreateFileW`, `ReadFile`, `WriteFile`, etc.
    //
    // Both `mio` and `tokio` don't yet have any code to work on arbitrary HANDLEs.
    // But they have code for dealing with named pipes, and we (ab)use that here to work on COM ports.
    pipe: NamedPipeClient,
}

impl WindowsSerialStream {
    /// Opens a COM port at the specified path
    // [CF] Copied from https://github.com/berkowski/mio-serial/blob/38a3778324da5e312cfb31402bd89e52d0548a4c/src/lib.rs#L113-L166
    // See remarks in the code for important changes!
    pub fn open(builder: &SerialPortBuilder) -> crate::Result<Self> {
        let (path, baud, parity, data_bits, stop_bits, flow_control) = {
            let com_port = serialport::COMPort::open(builder)?;
            let name = com_port.name().ok_or(crate::Error::new(
                crate::ErrorKind::NoDevice,
                "Empty device name",
            ))?;
            let baud = com_port.baud_rate()?;
            let parity = com_port.parity()?;
            let data_bits = com_port.data_bits()?;
            let stop_bits = com_port.stop_bits()?;
            let flow_control = com_port.flow_control()?;

            let mut path = Vec::<u16>::new();
            path.extend(OsStr::new("\\\\.\\").encode_wide());
            path.extend(Path::new(&name).as_os_str().encode_wide());
            path.push(0);

            (path, baud, parity, data_bits, stop_bits, flow_control)
        };

        let handle = unsafe {
            CreateFileW(
                path.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                ptr::null_mut(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OVERLAPPED,
                0 as HANDLE,
            )
        };

        if handle == INVALID_HANDLE_VALUE {
            return Err(crate::Error::from(io::Error::last_os_error()));
        }
        let handle = unsafe { mem::transmute(handle) };

        // Construct NamedPipe and COMPort from Handle
        //
        // [CF] ATTENTION: First set the COM Port parameters, THEN create the NamedPipeClient.
        // If I do it the other way round (as mio-serial does), the runtime hangs
        // indefinitely when querying for read readiness!
        //
        let mut com_port = unsafe { serialport::COMPort::from_raw_handle(handle) };
        com_port.set_baud_rate(baud)?;
        com_port.set_parity(parity)?;
        com_port.set_data_bits(data_bits)?;
        com_port.set_stop_bits(stop_bits)?;
        com_port.set_flow_control(flow_control)?;
        Self::override_comm_timeouts(handle)?;

        let pipe = unsafe { NamedPipeClient::from_raw_handle(handle)? };

        Ok(Self {
            inner: com_port,
            pipe,
        })
    }

    pub fn get_ref(&self) -> &COMPort {
        &self.inner
    }

    pub fn get_mut(&mut self) -> &mut COMPort {
        &mut self.inner
    }

    pub async fn readable(&self) -> io::Result<()> {
        self.pipe.readable().await
    }

    pub async fn writable(&self) -> io::Result<()> {
        self.pipe.writable().await
    }

    /// Overrides timeout value set by serialport-rs so that the read end will
    /// never wake up with 0-byte payload.
    // [CF] Copied from https://github.com/berkowski/mio-serial/blob/38a3778324da5e312cfb31402bd89e52d0548a4c/src/lib.rs#L685-L702
    fn override_comm_timeouts(handle: RawHandle) -> io::Result<()> {
        let mut timeouts = COMMTIMEOUTS {
            // wait at most 1ms between two bytes (0 means no timeout)
            ReadIntervalTimeout: 1,
            // disable "total" timeout to wait at least 1 byte forever
            ReadTotalTimeoutMultiplier: 0,
            ReadTotalTimeoutConstant: 0,
            // write timeouts are just copied from serialport-rs
            WriteTotalTimeoutMultiplier: 0,
            WriteTotalTimeoutConstant: 0,
        };

        let r = unsafe { SetCommTimeouts(handle, &mut timeouts) };
        if r == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
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
