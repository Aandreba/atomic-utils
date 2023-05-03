use crate::{InnerAtomicFlag, FALSE, TRUE};
use core::{
    cell::UnsafeCell,
    mem::{needs_drop, MaybeUninit},
    sync::atomic::Ordering,
};

/// Inverse of a `OnceCell`. It initializes with a value, which then can be raced by other threads to take.
///
/// Once the value is taken, it can never be taken again.
pub struct TakeCell<T> {
    taken: InnerAtomicFlag,
    v: UnsafeCell<MaybeUninit<T>>,
}

impl<T> TakeCell<T> {
    /// Creates a new [`TakeCell`]
    #[inline]
    pub const fn new(v: T) -> Self {
        Self {
            taken: InnerAtomicFlag::new(FALSE),
            v: UnsafeCell::new(MaybeUninit::new(v)),
        }
    }

    /// Creates a [`TakeCell`] that has already been taken
    #[inline]
    pub const fn new_taken() -> Self {
        Self {
            taken: InnerAtomicFlag::new(TRUE),
            v: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    /// Checks if the cell has alredy been taken
    #[inline]
    pub fn is_taken(&self) -> bool {
        self.taken.load(Ordering::Relaxed) == TRUE
    }

    /// Attempts to take the value from the cell, returning `None` if the value has already been taken
    #[inline]
    pub fn try_take(&self) -> Option<T> {
        if self
            .taken
            .compare_exchange(FALSE, TRUE, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            unsafe {
                let v = &*self.v.get();
                return Some(v.assume_init_read());
            }
        }
        None
    }

    /// Attempts to take the value from the cell through non-atomic operations, returning `None` if the value has already been taken
    ///
    /// # Safety
    /// This method is safe because the mutable reference indicates we are the only thread with access to the cell,
    /// so atomic operations aren't required.
    #[inline]
    pub fn try_take_mut(&mut self) -> Option<T> {
        let taken = self.taken.get_mut();
        if *taken == FALSE {
            *taken = TRUE;

            unsafe { return Some(self.v.get_mut().assume_init_read()) }
        }
        None
    }
}

impl<T> Drop for TakeCell<T> {
    #[inline]
    fn drop(&mut self) {
        if needs_drop::<T>() && *self.taken.get_mut() == FALSE {
            unsafe { self.v.get_mut().assume_init_drop() }
        }
    }
}

unsafe impl<T: Send> Send for TakeCell<T> {}
unsafe impl<T: Sync> Sync for TakeCell<T> {}

// Thanks ChatGPT!
#[cfg(test)]
mod tests {
    use super::TakeCell;

    #[test]
    fn test_normal_conditions() {
        let cell = TakeCell::new(42);
        assert_eq!(cell.is_taken(), false);
        assert_eq!(cell.try_take(), Some(42));
        assert_eq!(cell.is_taken(), true);
        assert_eq!(cell.try_take(), None);

        let mut cell = TakeCell::new(42);
        assert_eq!(cell.try_take_mut(), Some(42));
        assert_eq!(cell.try_take_mut(), None);
    }

    #[cfg(miri)]
    #[test]
    fn test_stressed_conditions() {
        use std::{
            sync::{Arc, Barrier},
            thread,
        };

        let cell = Arc::new(TakeCell::new(42));
        let barrier = Arc::new(Barrier::new(10));

        let mut handles = vec![];

        for _ in 0..10 {
            let c = Arc::clone(&cell);
            let b = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                b.wait();
                c.try_take()
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(cell.is_taken(), true);
        assert_eq!(cell.try_take(), None);
    }
}
