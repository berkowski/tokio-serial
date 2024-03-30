use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process, time,
};
use tokio_serial::SerialPortBuilderExt;

#[cfg(unix)]
const DEFAULT_TEST_PORT_NAMES: &str = concat!(
    env!("CARGO_TARGET_TMPDIR"),
    "/ttyUSB0;",
    env!("CARGO_TARGET_TMPDIR"),
    "/ttyUSB1"
);
#[cfg(not(unix))]
const DEFAULT_TEST_PORT_NAMES: &str = "COM10;COM11";

struct Fixture {
    #[cfg(unix)]
    process: process::Child,
    pub port_a: &'static str,
    pub port_b: &'static str,
}

#[cfg(unix)]
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(id) = self.process.id() {
            log::trace!("stopping socat process (id: {})...", id);

            self.process.start_kill().ok();
            std::thread::sleep(Duration::from_millis(250));
            log::trace!("removing link: {}", self.port_a);
            std::fs::remove_file(self.port_a).ok();
            log::trace!("removing link: {}", self.port_b);
            std::fs::remove_file(self.port_b).ok();
        }
    }
}

impl Fixture {
    #[cfg(unix)]
    pub async fn new(port_a: &'static str, port_b: &'static str) -> Self {
        let args = [
            format!("PTY,link={port_a}"),
            format!("PTY,link={port_b}"),
        ];
        log::trace!("starting process: socat {} {}", args[0], args[1]);

        let process = process::Command::new("socat")
            .args(&args)
            .spawn()
            .expect("unable to spawn socat process");
        log::trace!(".... done! (pid: {:?})", process.id().unwrap());

        time::sleep(Duration::from_millis(500)).await;

        Self {
            process,
            port_a,
            port_b,
        }
    }

    #[cfg(not(unix))]
    pub async fn new(port_a: &'static str, port_b: &'static str) -> Self {
        Self { port_a, port_b }
    }
}

async fn setup_virtual_serial_ports() -> Fixture {
    let port_names: Vec<&str> = std::option_env!("TEST_PORT_NAMES")
        .unwrap_or(DEFAULT_TEST_PORT_NAMES)
        .split(';')
        .collect();

    assert_eq!(port_names.len(), 2);
    Fixture::new(port_names[0], port_names[1]).await
}

#[tokio::test]
async fn send_recv() {
    env_logger::init();

    let fixture = setup_virtual_serial_ports().await;

    let mut sender = tokio_serial::new(fixture.port_a, 9600)
        .open_native_async()
        .expect("unable to open serial port");
    let mut receiver = tokio_serial::new(fixture.port_b, 9600)
        .open_native_async()
        .expect("unable to open serial port");

    log::trace!("sending test message");
    let message = b"This is a test message";
    sender
        .write_all(message)
        .await
        .expect("unable to write test message");

    log::trace!("receiving test message");
    let mut buf = [0u8; 32];
    let n = receiver
        .read_exact(&mut buf[..message.len()])
        .await
        .expect("unable to read test message");

    log::trace!("checking test message");
    assert_eq!(&buf[..n], message);
}
