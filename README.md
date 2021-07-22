# tokio-serial

An implementation of  serialport I/O for Tokio, an async framework for rust.

[![Build status](https://github.com/berkowski/tokio-serial/actions/workflows/github-ci.yml/badge.svg)](https://github.com/berkowski/tokio-serial/actions)
[![crates.io](http://shields.io/crates/v/tokio-serial)](https://crates.io/crates/tokio-serial)
[![docs.rs](https://docs.rs/tokio-serial/badge.svg)](https://docs.rs/tokio-serial)


## NOTICE
This crate is no longer actively maintained (see [#40](https://github.com/berkowski/tokio-serial/issues/40)) and is
open for adoption.  Create an issue if interested.

## Usage

Add `tokio-serial` to you `Cargo.toml`:

```toml
[dependencies]
tokio-serial = "5.4.0-beta2"
```

## Tests
Useful tests for serial ports require... serial ports, and serial ports are not often provided by online CI providers.
As so, automated build testing are really only check whether the code compiles, not whether it works.

Integration tests are in the `tests/` directory and typically require two serial ports to run.
The names of the serial ports can be configured at run time by setting the `TEST_PORT_NAMES` environment variable
to a semi-colon delimited string with the two serial port names.  The default values are:

- For Unix: `TEST_PORT_NAMES=/dev/ttyUSB0;/dev/ttyUSB1`
- For Windows: `TEST_PORT_NAMES=COM1;COM2`

**IMPORTANT** To prevent multiple tests from talking to the same ports at the same time make sure to limit the number
of test threads to 1 using:

```sh
cargo test -j1 -- --test-threads=1
```
## Resources

[tokio.rs](https://tokio.rs)
[serialport-rs](https://gitlab.com/susurrus/serialport-rs)
