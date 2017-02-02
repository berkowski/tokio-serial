# tokio-serial

An implementation of  serialport I/O for Tokio

[![crates.io](http://meritbadge.herokuapp.com/tokio-serial)](https://crates.io/crates/tokio-serial)
[![docs.rs](https://docs.rs/tokio-serial/badge.svg)](https://docs.rs/tokio-serial)

**Note:** At the moment this is unix only.  No windows COM port yet.

## Usage

Add `tokio-serial` to you `Cargo.toml`:

```toml
[dependencies]
tokio-serial = "0.1"
```

Then add this to your crate root:

```rust
extern crate tokio_serial;
```
