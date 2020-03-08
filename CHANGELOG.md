# Changelog

## Unreleased

* Raising an interrupt that isn't registered now only panics in debug builds,
  and goes into an infinite loop in release builds.

## [0.2.2 - 2020-02-16](https://github.com/jonas-schievink/irq/releases/tag/v0.2.2)

* Slight documentation improvements.

## [0.2.1 - 2020-01-26](https://github.com/jonas-schievink/irq/releases/tag/v0.2.1)

* Add a `Send` bound to `Handler::new`. This is a soundness fix.

## [0.2.0 - 2020-01-21](https://github.com/jonas-schievink/irq/releases/tag/v0.2.0)

* Improve readme and documentation.
* Do not require a user-defined macro to work.

## [0.1.0 - 2020-01-19](https://github.com/jonas-schievink/irq/releases/tag/v0.1.0)

Initial release.
