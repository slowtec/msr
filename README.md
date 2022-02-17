# msr

A [Rust](https://www.rust-lang.org) library for industrial automation.

[![Crates.io version](https://img.shields.io/crates/v/msr.svg)](https://crates.io/crates/msr)
[![Docs.rs](https://docs.rs/msr/badge.svg)](https://docs.rs/msr/)
[![Security audit](https://github.com/slowtec/msr/actions/workflows/security-audit.yaml/badge.svg)](https://github.com/slowtec/msr/actions/workflows/security-audit.yaml)
[![Continuous integration](https://github.com/slowtec/msr/actions/workflows/continuous-integration.yaml/badge.svg)](https://github.com/slowtec/msr/actions/workflows/continuous-integration.yaml)
[![Apache 2.0 licensed](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](./LICENSE-APACHE)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE-MIT)

## DISCLAIMER

**_Version 0.3.x is an experimental release for early prototyping. Breaking changes might occur even between minor releases._**

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
msr = "0.3"
```

## Development

For executing common development tasks install [cargo just](https://github.com/casey/just)
and run it without arguments to print the list of predefined _recipes_:

```shell
cargo install just
just

```

## License

Copyright (c) 2018 - 2021, [slowtec GmbH](https://www.slowtec.de)

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or
   <http://opensource.org/licenses/MIT>)

at your option.
