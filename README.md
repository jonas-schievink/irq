# IRQ – Utilities for writing Interrupt Handlers

[![crates.io](https://img.shields.io/crates/v/irq.svg)](https://crates.io/crates/irq)
[![docs.rs](https://docs.rs/irq/badge.svg)](https://docs.rs/irq/)
[![Build Status](https://travis-ci.org/jonas-schievink/irq.svg?branch=master)](https://travis-ci.org/jonas-schievink/irq)

This crate provides utilities for handling interrupts on embedded devices.

Please refer to the [changelog](CHANGELOG.md) to see what changed in the last
releases.

## Features

* Dynamically and atomically registered, zero-allocation interrupt handlers.
* Allows moving data into interrupt handlers, and sharing data between handlers.
* Completely platform agnostic, does not require atomic swap operations (works
  on eg. thumbv6 targets).

## Usage

Add an entry to your `Cargo.toml`:

```toml
[dependencies]
irq = "0.1.0"
```

Check the [API Documentation](https://docs.rs/irq/) for how to use the
crate's functionality. A small example showcasing the Scoped Interrupts API is
provided below:

```rust
use irq::{scoped_interrupts, handler, scope};

// This macro is invoked by `scoped_interrupts!` to declare an interrupt
// handler. It needs to expand to code that makes `$f` act as an interrupt
// handler for interrupt `$name`.
macro_rules! hook {
    (
        interrupt = $name:ident;
        function = $f:item;
    ) => {
        #[interrupt]
        $f
    };
}

// Hook `INT0` and `INT1` using the `hook!` macro above.
scoped_interrupts! {
    enum Interrupt {
        INT0,
        INT1,
    }

    use mock_pac::interrupt;

    with hook!(...)
}

fn main() {
    // Define data to be used (via move or borrow) by the interrupt handlers.
    let mut i = 0;
    let shared = [0, 1, 2];

    // Define handlers using the `handler!` macro.
    handler!(int0 = || i += shared[1]);
    handler!(int1 = || println!("{}", shared[2]));

    // Create a scope and register the handlers.
    scope(|scope| {
        scope.register(Interrupt::INT0, int0);
        scope.register(Interrupt::INT1, int1);

        // The interrupts stay registered for the duration of this closure.
        // This is a good place for the application's idle loop.
    });
}
```

## Rust version support

This crate targets stable Rust. No guarantees are made beyond that, so the
minimum supported version might be bumped as needed.

The MSRV is also explicitly tested against in [.travis.yml](.travis.yml).
