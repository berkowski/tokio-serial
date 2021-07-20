use std::{mem, time::Duration};
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncWriteExt},
    process, time,
};
use tokio_serial::SerialPortBuilderExt;

struct SocatFixture {
    process: process::Child,
    port_a: String,
    port_b: String,
}

cfg_if::cfg_if! {
    if #[cfg(unix)] {
        const DEFAULT_TEST_PORT_NAMES: &str = "/dev/ttyUSB0;/dev/ttyUSB1";
    }
    else {
        const DEFAULT_TEST_PORT_NAMES: &str = "COM10;COM11";
    }
}

impl Drop for SocatFixture {
    fn drop(&mut self) {
        if let Some(id) = self.process.id() {
            log::trace!("stopping socat process (id: {})...", id);
            self.process.start_kill().ok();
            std::thread::sleep(Duration::from_millis(250));
            log::trace!("removing link: {}", self.port_a.as_str());
            std::fs::remove_file(self.port_a.as_str()).ok();
            log::trace!("removing link: {}", self.port_b.as_str());
            std::fs::remove_file(self.port_b.as_str()).ok();
        }
    }
}

impl SocatFixture {
    pub async fn new(port_a: &str, port_b: &str) -> Self {
        let args = [
            format!("PTY,link={}", port_a),
            format!("PTY,link={}", port_b),
        ];
        log::trace!("starting process: socat {} {}", args[0], args[1]);

        let child = process::Command::new("socat")
            .args(&args)
            .spawn()
            .expect("unable to spawn socat process");
        log::trace!(".... done! (pid: {:?})", child.id().unwrap());

        time::sleep(Duration::from_millis(500)).await;

        Self {
            process: child,
            port_a: port_a.to_owned(),
            port_b: port_b.to_owned(),
        }
    }
}

#[tokio::test]
async fn send_recv() {
    env_logger::init();

    let port_names: Vec<&str> = std::option_env!("TEST_PORT_NAMES")
        .unwrap_or(DEFAULT_TEST_PORT_NAMES)
        .split(';')
        .collect();

    let port_a = port_names[0];
    let port_b = port_names[1];

    assert_eq!(port_names.len(), 2, "expected two port names");

    #[cfg(unix)]
    let fixture = SocatFixture::new(port_a, port_b).await;

    let mut sender = tokio_serial::new(port_a, 9600)
        .open_native_async()
        .expect("unable to open serial port");
    let mut receiver = tokio_serial::new(port_b, 9600)
        .open_native_async()
        .expect("unable to open serial port");

    log::trace!("sending test message");
    let message = b"This is a test message";
    sender
        .write(message)
        .await
        .expect("unable to write test message");

    log::trace!("receiving test message");
    let mut buf = [0u8; 32];
    let n = receiver
        .read(&mut buf[..])
        .await
        .expect("unable to read test message");

    log::trace!("checking test message");
    assert_eq!(&buf[..n], message);

    mem::drop(fixture);

    log::trace!("removing /tmp/ttyS10");
    fs::remove_file("/tmp/ttyS10").await.ok();
    log::trace!("removing /tmp/ttyS11");
    fs::remove_file("/tmp/ttyS11").await.ok();
}
