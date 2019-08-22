extern crate bytes;
extern crate tokio;
extern crate tokio_io;
extern crate tokio_serial;

use bytes::BytesMut;
use std::{env, io, str};
use tokio::codec::{Decoder, Encoder};

use tokio::prelude::*;

#[cfg(unix)]
const DEFAULT_TTY: &str = "/dev/ttyUSB0";
#[cfg(windows)]
const DEFAULT_TTY: &str = "COM1";

struct LineCodec;

impl Decoder for LineCodec {
    type Item = String;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let newline = src.as_ref().iter().position(|b| *b == b'\n');
        if let Some(n) = newline {
            let line = src.split_to(n + 1);
            return match str::from_utf8(line.as_ref()) {
                Ok(s) => Ok(Some(s.to_string())),
                Err(_) => Err(io::Error::new(io::ErrorKind::Other, "Invalid String")),
            };
        }
        Ok(None)
    }
}

impl Encoder for LineCodec {
    type Item = String;
    type Error = io::Error;

    fn encode(&mut self, _item: Self::Item, _dst: &mut BytesMut) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn main() {
    let mut args = env::args();
    let tty_path = args.nth(1).unwrap_or_else(|| DEFAULT_TTY.into());

    let settings = tokio_serial::SerialPortSettings::default();
    let mut port = tokio_serial::Serial::from_path(tty_path, &settings).unwrap();
    #[cfg(unix)]
    port.set_exclusive(false)
        .expect("Unable to set serial port exlusive");

    let framed = LineCodec.framed(port);

    let printer = framed.for_each(|s| {
        match s {
            Ok(data) => println!("{:x?}", data),
            Err(err) => println!("err: {:x?}", err),
        }
        tokio::future::ready(())
    });

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(printer);
}
