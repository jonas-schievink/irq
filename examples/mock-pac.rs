//! This example demonstrates how to use an svd2rust-generated PAC with this crate.

use irq::{handler, scope, scoped_interrupts};
use mock_pac::interrupt;

scoped_interrupts! {
    enum Interrupt {
        INT0,
    }

    use #[interrupt];
}

fn main() {
    let mut i = 0;
    handler!(
        int0 = move || {
            if i == 0 {
                i = 1;
            }
        }
    );

    scope(|scope| {
        scope.register(Interrupt::INT0, int0);

        loop {
            // Idle loop
            break;
        }
    });
}
