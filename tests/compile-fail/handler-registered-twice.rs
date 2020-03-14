//! Tests that a handler can only be registered once for the duration of a `scope`.
//!
//! This is required for safety since otherwise the handler could preempt itself and duplicate
//! mutable references during the second invocation.

use irq::{handler, scope, scoped_interrupts};
use mock_pac::interrupt;

scoped_interrupts! {
    enum Interrupt {
        INT0,
        INT1,
    }

    use #[interrupt];
}

fn main() {
    let mut i = 0;
    handler!(int0 = move || i += 1);

    scope(|scope| {
        scope.register(Interrupt::INT0, int0);
        scope.register(Interrupt::INT1, int0);

        loop {
            // Idle loop
            break;
        }
    });
}
