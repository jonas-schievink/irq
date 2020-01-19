//! This example demonstrates how to use an svd2rust-generated PAC with this crate.

use irq::{handler, scope, scoped_interrupts};

/// We need to provide a `hook!` macro that knows how to hook an interrupt handler.
///
/// In this case, this is done by putting cortex-m-rt's `#[interrupt]` macro on the veneer function.
/// We have to import the `#[interrupt]` attribute *inside* our macro because the code ends up in a
/// submodule.
macro_rules! hook {
    (
        interrupt = $name:ident;
        function = $f:item;
    ) => {
        #[interrupt]
        $f
    };
}

scoped_interrupts! {
    enum Interrupt {
        INT0,
    }

    use mock_pac::interrupt;

    with hook!(...)
}

fn main() {
    let mut i = 0;
    handler!(int0 = move || i += 1);

    scope(|scope| {
        scope.register(Interrupt::INT0, int0);

        loop {
            // Idle loop
            break;
        }
    });
}
