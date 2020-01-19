# **I**nterrupt **R**e**q**uest

[![crates.io](https://img.shields.io/crates/v/irq.svg)](https://crates.io/crates/irq)
[![docs.rs](https://docs.rs/irq/badge.svg)](https://docs.rs/irq/)
[![Build Status](https://travis-ci.org/jonas-schievink/irq.svg?branch=master)](https://travis-ci.org/jonas-schievink/irq)

TODO: Briefly describe the crate here (eg. "This crate provides ...").

Please refer to the [changelog](CHANGELOG.md) to see what changed in the last
releases.

## Features

* Dynamically and atomically registered, zero-allocation interrupt handlers.
* Allows moving data into interrupt handlers, and sharing data between handlers.
* Completely platform agnostic, does not require atomic swap operations (works on eg. thumbv6 targets).

## Usage

Add an entry to your `Cargo.toml`:

```toml
[dependencies]
irq = "0.0.0"
```

Check the [API Documentation](https://docs.rs/irq/) for how to use the
crate's functionality.

## Rust version support

This crate supports the 3 latest stable Rust releases. Bumping the minimum
supported Rust version (MSRV) is not considered a breaking change as long as
these 3 versions are still supported.

The MSRV is also explicitly tested against in [.travis.yml](.travis.yml).
