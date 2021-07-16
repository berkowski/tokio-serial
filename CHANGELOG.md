# Change log

All notable changes to this project will be documented in this file.

The format is based on http://keepachangelog.com/[Keep a Changelog]
and this project adheres to http://semver.org/[Semantic Versioning].

[5.4.0-beta1] UNRELEASED

Major release drawing in updates to `tokio` and `mio-serial` (and the upstream `serialport-rs`)

### Changed
- Bumped [tokio](https://github.com/tokio-rs/tokio) to 1.0
- Bumped [mio-serial](https://github.com/berkowski/mio-serial) to 4.0.0-beta2

### Contributions
- [#35](https://github.com/berkowski/tokio-serial/pull/35) by [georgmu](https://github.com/georgmu) found an early bug in the AsyncRead trait impl
- [#39](https://github.com/berkowski/tokio-serial/pull/39) and [#41](https://github.com/berkowski/tokio-serial/pull/41) by [ColinFinck](https://github.com/ColinFinck) took it upon himself to push windows support for Tokio 1.X
  and did the vast majority of the initial work and paved the way
  
## [4.3.3] 2019-11-24
### Changed
* @Darksonn bumped tokio dependencies to version 0.2.0 and cleaned up some dependencies in PR [#24](https://github.com/berkowski/tokio-serial/pull/24)

## [4.3.3-alpha.6] 2019-11-15
### Changed
* @Darksonn bumped tokio dependencies to version 0.2.0-alpha.6 in PR [#23](https://github.com/berkowski/tokio-serial/pull/23)
* Updated README.md to include latest tokio-serial version numbers for both tokio 0.1 and 0.2-alpha based libraries.

## [4.3.3-alpha.2] 2019-08-26
### Changed
* Bumped to tokio dependencies to version 0.2
  Majority of work done by @12101111 and @will-w in PR's [#19](https://github.com/berkowski/tokio-serial/pull/19)
  and [#21](https://github.com/berkowski/tokio-serial/pull/21) respectively
* @D1plo1d bumped the tokio dependency to 0.2.0-alpha.2 in [#22](https://github.com/berkowski/tokio-serial/pull/21)



## [3.3.0] 2019-08-23
* Bumped [mio-serial](https://gitlab.com/berkowski/mio-serial) to 3.3.0 
* Switched to "2018" edition

## [3.2.14] 2019-06-01
### Changed
* Bumped [mio-serial](https://gitlab.com/berkowski/mio-serial) to 3.2.14 (tracking mio version 0.14)

### changed
* Merged [#17](https://github.com/berkowski/tokio-serial/pull/17) @nanpuyue updated the printline example.

## [3.2] 2019-01-12
### Changed
* Bumped [serialport-rs](https://gitlab.com/susurrus/serialport-rs) to 3.2

## [3.1.1] 2019-01-12
### Changed
* Merged [#16](https://github.com/berkowski/tokio-serial/pull/16) @yuja fixed feature flags

## [3.1.0] - 2018-11-10
### changed
* Bumped `mio-serial` dependency to 3.1

## [3.0.0] - 2018-10-06
### changed
* Bumped `mio-serial` dependency to 3.0

## [0.8.0] - 2018-04-27
### changed
* Migrated to tokio 0.1 with https://github.com/berkowski/tokio-serial/pull/9[#9] and
  https://github.com/berkowski/tokio-serial/pull/10[#10] Thanks, https://github.com/lnicola[lnicola]!
* Bumped `mio-serial` dependency to 0.8

## [0.7.0] - UNRELEASED
### added
* Windows support (through mio-serial 0.7)
* Appveyor testing support

### changed
* Bumped `mio-serial` dependency to 0.7


## [0.6.0] - 2017-11-28
### added
* Re-exporting `mio_serial::Error` (itself a re-export of `serialport::Error`)

### changed
* Bumped `mio-serial` dependency to 0.6

## [0.5.0] - 2017-05-18
### added
* Added `trust` CI
* https://github.com/berkowski/tokio-serial/pull/1[#1] provided `AsyncRead` and
  `AsyncWrite` impls.  Thanks https://github.com/lexxvir[lexxvir]!

### changed
* Bumped `mio-serial` dependency to 0.5  Future releases will
  track `mio-serial` versions.
