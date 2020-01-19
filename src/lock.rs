use core::cell::UnsafeCell;
use core::fmt;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

// TODO: What about sharing data between 2 interrupts on the same priority level?

/// A lock that allows sharing data between two interrupts at different priorities.
///
/// This is a general spinlock-like implementation that works even on architectures without
/// compare-and-swap instructions. This is accomplished by making use of [*Peterson's Algorithm*].
///
/// # Drawbacks
///
/// Being a general architecture-independent implementation means that it also comes with some
/// drawbacks due to not knowing anything about the target platform:
///
/// * It is limited to 2 parties sharing data. [*Peterson's Algorithm*] requires storage
///   proportional to the number of parties competing for exclusive access. With const generics it
///   might be possible to make this a compile-time parameter instead.
/// * Locking from an interrupt can fail irrecoverably. This is a fundamental limitation of trying
///   to ensure exclusive access via blocking mutexes in the presence of interrupts, and would also
///   occur when using any other generic solution (like a "real" spinlock). User code must handle a
///   failure to acquire a resource in an interrupt handler gracefully.
///
/// # Alternatives
///
/// If the drawbacks listed above are unacceptable (which is not unlikely), consider using one of
/// these alternatives for sharing data between interrupts:
///
/// * Lock-free datastructures such as those provided by [heapless].
/// * Atomics and read-modify-write operations from `core::sync::atomic` (if your target supports
///   them).
/// * A Mutex implementation that turns off interrupts (when targeting a single-core MCU).
/// * A hardware-provided Mutex peripheral (when targeting a multi-core MCU).
/// * The [Real-Time For the Masses][RTFM] framework.
///
/// [*Peterson's Algorithm*]: https://en.wikipedia.org/wiki/Peterson%27s_algorithm
/// [heapless]: https://docs.rs/heapless
/// [RTFM]: https://github.com/rtfm-rs/
#[derive(Debug)]
pub struct PriorityLock<T> {
    // TODO: Optimize memory usage when we have atomic CAS
    wants_to_enter: [AtomicBool; 2],
    turn: AtomicU8,
    data: UnsafeCell<T>,
}

impl<T> PriorityLock<T> {
    /// Creates a new lock protecting `data`.
    ///
    /// If `data` consists of zeroes, the resulting `PriorityLock` will also be zero-initialized
    /// and can be placed in `.bss` by the compiler.
    pub const fn new(data: T) -> Self {
        Self {
            wants_to_enter: [AtomicBool::new(false), AtomicBool::new(false)],
            turn: AtomicU8::new(0),
            data: UnsafeCell::new(data),
        }
    }

    /// Splits this lock into its low- and high-priority halfs.
    ///
    /// The low-priority half provides a [`lock`] method for acquiring the lock, and is meant to be
    /// used from a lower-priority context than the high-priority half (eg. a low-priority
    /// interrupt or the application's idle loop). The high-priority half provides a [`try_lock`]
    /// method for acquiring the lock, which may fail when preempting code holding the low-priority
    /// half of the lock.
    ///
    /// [`lock`]: struct.LockHalf.html#method.lock
    /// [`try_lock`]: struct.LockHalf.html#method.try_lock
    pub fn split<'a>(&'a mut self) -> (LockHalf<'a, T, PLow>, LockHalf<'a, T, PHigh>) {
        let low = LockHalf {
            lock: self,
            _p: PhantomData,
        };
        let high = LockHalf {
            lock: self,
            _p: PhantomData,
        };
        (low, high)
    }

    fn try_acquire_raw(&self, index: u8) -> Result<(), ()> {
        // Algorithm according to https://en.wikipedia.org/wiki/Peterson%27s_algorithm

        // TODO: check what happens when recursively self-locking

        let other_index = (index + 1) % 2;

        // We want to enter.
        self.wants_to_enter[usize::from(index)].store(true, Ordering::Release);
        // Give the other lock owner a chance to run.
        self.turn.store(other_index, Ordering::Release);

        // Does the other owner want to enter, and did they not give us a chance to run (by setting
        // turn to our number)?
        if self.wants_to_enter[usize::from(other_index)].load(Ordering::Acquire)
            && self.turn.load(Ordering::Acquire) == other_index
        {
            // We did not acquire the lock. Restore our flag since we no longer want to enter.
            self.wants_to_enter[usize::from(index)].store(false, Ordering::Release);

            Err(())
        } else {
            Ok(())
        }
    }

    fn block_acquire_raw(&self, index: u8) {
        let other_index = (index + 1) % 2;

        // We want to enter.
        self.wants_to_enter[usize::from(index)].store(true, Ordering::Release);
        // Give the other lock owner a chance to run.
        self.turn.store(other_index, Ordering::Release);

        // Does the other owner want to enter, and did they not give us a chance to run (by setting
        // turn to our number)?
        while self.wants_to_enter[usize::from(other_index)].load(Ordering::Acquire)
            && self.turn.load(Ordering::Acquire) == other_index
        {}
    }

    /// Safety: Unlocking an index not owned by the caller is unsound.
    unsafe fn unlock(&self, index: u8) {
        self.wants_to_enter[usize::from(index)].store(false, Ordering::Release);
    }
}

mod sealed {
    pub trait Sealed {}
}

/// Trait implemented by the lock priority types [`PHigh`] and [`PLow`].
///
/// This trait is an internal API and should not be used by user code. It cannot be implemented by
/// user-defined types.
///
/// [`PHigh`]: enum.PHigh.html
/// [`PLow`]: enum.PLow.html
pub trait LockPriority: sealed::Sealed {
    #[doc(hidden)]
    const INDEX: u8;
}

/// Type marker indicating the high-priority half of a [`PriorityLock`].
///
/// [`PriorityLock`]: struct.PriorityLock.html
#[derive(Debug)]
pub enum PHigh {}

/// Type marker indicating the low-priority half of a [`PriorityLock`].
///
/// [`PriorityLock`]: struct.PriorityLock.html
#[derive(Debug)]
pub enum PLow {}

impl sealed::Sealed for PHigh {}
impl sealed::Sealed for PLow {}

impl LockPriority for PLow {
    const INDEX: u8 = 0;
}
impl LockPriority for PHigh {
    const INDEX: u8 = 1;
}

/// Error indicating that a lock could not be acquired in a high-priority context.
///
/// With a normal lock used from an interrupt handler, this would be a deadlock.
///
/// **Note**: User code *must* handle this error in an application-specific manner! Just calling
/// `.unwrap()` is just as brittle as deadlocking.
#[allow(missing_debug_implementations)]
pub struct Deadlock {}
// (intentionally doesn't implement `Debug` so that `.unwrap()` cannot be called directly)

/// One half of a [`PriorityLock`].
///
/// This can be obtained via [`PriorityLock::split`].
///
/// [`PriorityLock`]: struct.PriorityLock.html
/// [`PriorityLock::split`]: struct.PriorityLock.html#method.split
#[derive(Debug)]
pub struct LockHalf<'a, T, P: LockPriority> {
    lock: &'a PriorityLock<T>,
    _p: PhantomData<P>,
}

impl<'a, T> LockHalf<'a, T, PLow> {
    /// Acquires the lock, granting access to `T`.
    ///
    /// This is meant to be called from a low-priority context and may be preempted by code owning
    /// the high-priority half of the lock. If the lock is already taken, this will block until it
    /// is released again.
    pub fn lock(&mut self) -> LockGuard<'a, T, PLow> {
        // This must take `&mut self` for soundness.

        self.lock.block_acquire_raw(0);
        LockGuard {
            lock: self.lock,
            _p: PhantomData,
        }
    }
}

impl<'a, T> LockHalf<'a, T, PHigh> {
    /// Tries to acquire the lock, granting access to `T`.
    ///
    /// This is meant to be called from a high-priority context that may preempt code owning the
    /// low-priority half of the lock.
    ///
    /// # Errors
    ///
    /// This operation can fail when the low-priority code already holds the lock, and is being
    /// preempted by the code calling `try_lock`. **There is no general way to recover from this**.
    /// If this is an issue, consider using a different way of sharing data between interrupts (see
    /// the [`PriorityLock`] documentation for guidance).
    ///
    /// [`PriorityLock`]: struct.PriorityLock.html
    pub fn try_lock(&mut self) -> Result<LockGuard<'a, T, PHigh>, Deadlock> {
        // This must take `&mut self` for soundness.

        self.lock.try_acquire_raw(1).map_err(|_| Deadlock {})?;
        Ok(LockGuard {
            lock: self.lock,
            _p: PhantomData,
        })
    }
}

/// A guard keeping a lock acquired until it is dropped.
pub struct LockGuard<'a, T, P: LockPriority> {
    lock: &'a PriorityLock<T>,
    _p: PhantomData<P>,
}

impl<'a, T, P: LockPriority> Deref for LockGuard<'a, T, P> {
    type Target = T;

    fn deref(&self) -> &T {
        // Safety: If the lock algorithm is correct, we have unique access to `T` here.
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T, P: LockPriority> DerefMut for LockGuard<'a, T, P> {
    fn deref_mut(&mut self) -> &mut T {
        // Safety: If the lock algorithm is correct, we have unique access to `T` here.
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<'a, T, P: LockPriority> Drop for LockGuard<'a, T, P> {
    fn drop(&mut self) {
        // Safety: We unlock only our own half of the lock, and don't access `T` anymore.
        unsafe {
            self.lock.unlock(P::INDEX);
        }
    }
}

impl<'a, T: fmt::Debug, P: LockPriority> fmt::Debug for LockGuard<'a, T, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, T: fmt::Display, P: LockPriority> fmt::Display for LockGuard<'a, T, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple() {
        let mut lock = PriorityLock::new(0u32);
        let (mut low, mut high) = lock.split();

        let mut low_guard = low.lock();
        *low_guard += 1;
        assert!(high.try_lock().is_err());
        drop(low_guard);

        let mut high_guard = high.try_lock().map_err(drop).unwrap();
        assert_eq!(*high_guard, 1);
        *high_guard += 1;
    }
}
