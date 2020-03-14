//! Test that no handlers can be registered that are defined *inside* the scope they would be
//! registered for.

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

    scope(|scope| {
        handler!(int0 = move || i += 1);
        scope.register(Interrupt::INT0, int0);

        loop {
            // Idle loop
            break;
        }
    });
}
