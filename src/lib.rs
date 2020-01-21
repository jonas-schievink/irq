//! Utilities for interrupt handling.
//!
//! This crate provides:
//!
//! * An API to register scoped interrupts, inspired by [`crossbeam::scope`].
//!
//!   After registering interrupts using the [`scoped_interrupts!`] macro, the [`scope`] function
//!   can be used to enter an "interrupt scope", in which interrupt handlers can be registered for
//!   the duration of the scope. This allows them to make use of data defined in the calling
//!   function.
//!
//! * A [`PriorityLock`] that allows sharing mutable data between interrupts at different
//!   priorities.
//!
//! # Examples
//!
//! Here is an example of how to use the Scoped Interrupts API with interrupts provided by an
//! [svd2rust]-generated Peripheral Access Crate:
//!
//! ```
//! use irq::{scoped_interrupts, handler, scope};
//! use mock_pac::interrupt;
//!
//! // Hook `INT0` and `INT1` using the `#[interrupt]` attribute imported above.
//! scoped_interrupts! {
//!     enum Interrupt {
//!         INT0,
//!         INT1,
//!     }
//!
//!     use #[interrupt];
//! }
//!
//! fn main() {
//!     // Define data to be used (via move or borrow) by the interrupt handlers.
//!     let mut i = 0;
//!     let shared = [0, 1, 2];
//!
//!     // Define handlers using the `handler!` macro.
//!     handler!(int0 = || i += shared[1]);
//!     handler!(int1 = || println!("{}", shared[2]));
//!
//!     // Create a scope and register the handlers.
//!     scope(|scope| {
//!         scope.register(Interrupt::INT0, int0);
//!         scope.register(Interrupt::INT1, int1);
//!     });
//! }
//! ```
//!
//! [`crossbeam::scope`]: https://docs.rs/crossbeam/0.7.3/crossbeam/fn.scope.html
//! [`scoped_interrupts!`]: macro.scoped_interrupts.html
//! [`scope`]: fn.scope.html
//! [`PriorityLock`]: struct.PriorityLock.html
//! [svd2rust]: https://github.com/rust-embedded/svd2rust

#![doc(html_root_url = "https://docs.rs/irq/0.1.0")]
// Deny a few warnings in doctests, since rustdoc `allow`s many warnings by default
#![doc(test(attr(deny(unused_imports, unused_must_use))))]
#![warn(missing_debug_implementations, rust_2018_idioms)]
#![cfg_attr(not(test), no_std)]

mod lock;
mod readme;

pub use lock::*;

use core::fmt;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicUsize, Ordering};

/// Hooks interrupts and makes them available to the [`scope`] API.
///
/// In order to hook the interrupts, you need to provide a macro to apply to the interrupt veneers.
/// This is generally architecture- or even MCU-specific. On Cortex-M devices, this should usually
/// be the `#[interrupt]` macro exported by the device-specific PAC.
///
/// It is not necessary to hook *all* interrupts. Only those that should be made available to the
/// [`scope`] API are required. Since every hooked interrupt comes with a cost in code and data
/// size, it is advisable to only hook the interrupts needed by the application.
///
/// # Examples
///
/// In this example, an [svd2rust]-generated Peripheral Access Crate `mock_pac` provides the
/// interrupts that can be hooked using the `#[interrupt]` macro:
///
/// ```
/// # use irq::scoped_interrupts;
/// # use mock_pac::interrupt;
/// #
/// scoped_interrupts! {
///     enum Interrupt {
///         INT0,
///     }
///
///     use #[interrupt];
/// }
///
/// # fn main() {}  // macro must be called outside a function
/// ```
///
/// Also refer to `examples/mock-pac.rs` for a standalone version with more comments.
///
/// [svd2rust]: https://github.com/rust-embedded/svd2rust
/// [`scope`]: fn.scope.html
#[macro_export]
macro_rules! scoped_interrupts {
    (
        $( #[$enum_attr:meta] )*
        $v:vis enum $name:ident {
            $(
                $interrupt:ident
            ),+

            $(,)?
        }

        use #[$hook_attr:meta];
    ) => {
        // Step 1: Declare an Actual Enum like that.
        $( #[$enum_attr] )*
        $v enum $name {
            $(
                $interrupt,
            )+
        }

        // Step 2: Hook all the interrupts and put veneers in place.

        // Extra module needed to avoid name collisions.
        pub(crate) mod statics {
            $(
                #[allow(bad_style)]
                pub(crate) static $interrupt: $crate::HandlerAddr = $crate::HandlerAddr::new();
            )+
        }

        // Now invoke the provided macro on each veneer.
        $(
            #[$hook_attr]
            #[allow(bad_style, dead_code)]
            unsafe fn $interrupt() {
                let handler = self::statics::$interrupt.load();
                if handler == 0 {
                    // XXX this might be expensive
                    panic!(concat!(
                        "no handler registered for ",
                        ::core::stringify!($interrupt)
                    ));
                } else {
                    let handler = handler as *mut $crate::Handler<'_>;

                    // Soundness:
                    // - Relies on the user-provided interface to manage the handler lifetime
                    //   (which is dangling here).
                    // - Relies on interrupts not being reentrant
                    (*handler).invoke();
                }
            }
        )+

        // Step 3: Implement the `Interrupt` trait.
        unsafe impl $crate::Interrupt for $name {
            unsafe fn register(self, handler: &mut $crate::Handler<'_>) {
                match self {
                    $(
                        Self::$interrupt => {
                            self::statics::$interrupt.store(handler as *mut _ as usize);
                        }
                    )+
                }
            }

            fn deregister_all() {
                // Safety: We store 0, which disables the interrupt, which is always safe.
                unsafe {
                    $(
                        self::statics::$interrupt.store(0);
                    )+
                }
            }
        }
    };
}

/// Defines a closure-based interrupt handler that can use stack-local data.
///
/// This is a convenience macro that creates a [`&mut Handler`][`Handler`] variable that can be
/// passed to [`Scope::register`].
///
/// # Examples
///
/// ```
/// # use irq::handler;
/// let mut i = 0;
/// handler!(my_handler = || i += 1);
/// ```
///
/// [`Handler`]: struct.Handler.html
/// [`Scope::register`]: struct.Scope.html#method.register
#[macro_export]
macro_rules! handler {
    ($name:ident = $e:expr) => {
        let mut closure = $e;
        let $name = &mut $crate::Handler::new(&mut closure);
    };
}

/// Creates a scope in which interrupt handlers using stack-local data can be registered.
///
/// When this function returns, all interrupts will be deregistered again.
pub fn scope<'env, I, F, R>(f: F) -> R
where
    I: Interrupt,
    F: FnOnce(&Scope<'env, I>) -> R,
{
    let scope = Scope { _p: PhantomData };

    let result = f(&scope);

    // Drop the scope, deregistering all interrupt handlers. This is required for soundness: Any
    // handler passed to `scope` only lives as long as `'env`, which may end right after this
    // function returns.
    drop(scope);

    result
}

/// An interrupt scope created by the [`scope`] function.
///
/// [`scope`]: fn.scope.html
#[allow(missing_debug_implementations)]
pub struct Scope<'env, I: Interrupt> {
    // Make `'env` invariant
    // CFAIL ^
    _p: PhantomData<(I, &'env mut &'env ())>,
}

impl<'env, I: Interrupt> Scope<'env, I> {
    /// Registers an interrupt handler for the duration of this scope.
    ///
    /// Once the enclosing [`scope`] call returns, all interrupts that were registered using this
    /// method will be deregistered again.
    ///
    /// # Parameters
    ///
    /// * `interrupt`: The interrupt to handle. This must be a variant of an enum generated by the
    ///   [`scoped_interrupts!`] macro.
    /// * `handler`: The handler closure to hook up to the interrupt. For convenience, this can be
    ///   created using the [`handler!`] macro.
    ///
    /// [`scope`]: fn.scope.html
    /// [`scoped_interrupts!`]: macro.scoped_interrupts.html
    /// [`handler!`]: macro.handler.html
    pub fn register(&self, interrupt: I, handler: &'env mut Handler<'env>) {
        unsafe {
            interrupt.register(handler);
        }
    }
}

impl<'env, I: Interrupt> Drop for Scope<'env, I> {
    fn drop(&mut self) {
        I::deregister_all();
    }
}

/// Wraps a closure used as an interrupt handler.
///
/// A `Handler` needs to be passed to [`Scope::register`] to do anything.
///
/// [`Scope::register`]: struct.Scope.html#method.register
pub struct Handler<'a> {
    f: &'a mut dyn FnMut(),
}

impl<'a> Handler<'a> {
    /// Creates a new interrupt handler wrapper given a closure.
    #[inline(always)]
    pub fn new<F>(f: &'a mut F) -> Self
    where
        F: FnMut(),
    {
        Self { f }
    }

    /// Invokes the interrupt handler closure.
    #[inline(always)]
    pub fn invoke(&mut self) {
        (self.f)();
    }
}

impl<'a> fmt::Debug for Handler<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "handler@{:p}", self as *const _)
    }
}

/// Private API for use by the `scoped_interrupts!` macro. Do not use.
#[doc(hidden)]
pub struct HandlerAddr {
    addr: AtomicUsize,
}

impl HandlerAddr {
    #[inline(always)]
    pub const fn new() -> Self {
        Self {
            addr: AtomicUsize::new(0),
        }
    }

    #[inline(always)]
    pub fn load(&self) -> usize {
        self.addr.load(Ordering::Acquire)
    }

    #[inline(always)]
    pub unsafe fn store(&self, addr: usize) {
        self.addr.store(addr, Ordering::Release)
    }
}

impl fmt::Debug for HandlerAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "active handler@{:p}", self.load() as *const ())
    }
}

/// Trait for interrupt enums generated by [`scoped_interrupts!`].
///
/// # Safety
///
/// This trait is unsafe to implement. Safely implementing it requires correctly implementing its
/// methods. In particular, `deregister_all` must, in fact, deregister all registered handlers.
///
/// [`scoped_interrupts!`]: macro.scoped_interrupts.html
pub unsafe trait Interrupt {
    /// Registers a `handler` to handle interrupts of type `self`.
    ///
    /// # Safety
    ///
    /// This is only safe to call if the caller ensures that the handler is not invoked after its
    /// lifetime expires.
    unsafe fn register(self, handler: &mut Handler<'_>);

    /// Deregisters all interrupts that were registered using `register`.
    ///
    /// This must reset the global interrupt state to its default/startup/reset values, where no
    /// interrupt handlers are registered.
    fn deregister_all();
}

#[cfg(test)]
mod tests {
    use super::Interrupt as _;
    use super::*;
    use std::panic::catch_unwind;

    scoped_interrupts! {
        enum Interrupt {
            Int0,
            Int1,
        }

        use #[no_mangle];
    }

    struct Test {}

    #[derive(Debug)]
    struct Panicked {}

    impl Test {
        fn raise_interrupt(&mut self, int: Interrupt) -> Result<(), Panicked> {
            catch_unwind(|| match int {
                Interrupt::Int0 => unsafe { Int0() },
                Interrupt::Int1 => unsafe { Int1() },
            })
            .map_err(|_| Panicked {})
        }
    }

    impl Drop for Test {
        fn drop(&mut self) {
            // Reset
            Interrupt::deregister_all();
        }
    }

    fn test(f: impl FnOnce(&mut Test)) {
        // Lock a mutex for each test, ensuring that they run sequentially. This is required since
        // they mutate shared state.
        // Miri detects this as a leak (which I guess is kinda true), so don't do this on Miri. It
        // doesn't support threads anyways.
        #[cfg(not(miri))]
        let _guard = {
            use once_cell::sync::OnceCell;
            use std::sync::Mutex;

            static MUTEX: OnceCell<Mutex<()>> = OnceCell::new();
            MUTEX
                .get_or_init(|| Mutex::new(()))
                .lock()
                .unwrap_or_else(|e| e.into_inner()) // drink the poison
        };

        let mut test = Test {};

        f(&mut test);
    }

    #[test]
    fn not_registered() {
        test(|test| {
            test.raise_interrupt(Interrupt::Int0).unwrap_err();
            test.raise_interrupt(Interrupt::Int1).unwrap_err();
            test.raise_interrupt(Interrupt::Int0).unwrap_err();
            test.raise_interrupt(Interrupt::Int1).unwrap_err();
        });
    }

    #[test]
    fn simple() {
        test(|test| {
            let mut i = 0;

            // Having to declare (store) the handler *outside* the call to `scope` is a natural
            // property of this approach, since it might be called anytime until `scope` returns.
            let mut closure = || {
                i += 1;
            };
            let mut handler = Handler::new(&mut closure);

            // (this is verbose, normally one would use `handler!`)

            scope(|scope| {
                // CFAIL: non-'env closure or handler are unsound
                scope.register(Interrupt::Int0, &mut handler);

                // Test that the handler is called when the interrupt is raised.
                test.raise_interrupt(Interrupt::Int0).unwrap();
            });

            assert_eq!(i, 1);

            // Test that the end of the scope deregisters the interrupt.
            test.raise_interrupt(Interrupt::Int0).unwrap_err();
            assert_eq!(i, 1);
        });
    }

    #[test]
    fn handler_sharing_data() {
        test(|test| {
            let shared = vec![0, 1, 2];

            handler!(handler0 = || println!("{:?}", shared));
            handler!(handler1 = || println!("{:?}", shared));

            scope(|scope| {
                scope.register(Interrupt::Int0, handler0);
                scope.register(Interrupt::Int1, handler1);

                test.raise_interrupt(Interrupt::Int0).unwrap();
                test.raise_interrupt(Interrupt::Int1).unwrap();
            });
        })
    }
}
