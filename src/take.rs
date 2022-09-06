use core::{cell::UnsafeCell, mem::{MaybeUninit, needs_drop}, sync::atomic::Ordering};
use crate::{InnerFlag, FALSE, TRUE};

/// Inverse of a `OnceCell`.
pub struct TakeCell<T> {
    taken: InnerFlag,
    v: UnsafeCell<MaybeUninit<T>>
}

impl<T> TakeCell<T> {
    #[inline(always)]
    pub const fn new (v: T) -> Self {
        Self {
            taken: InnerFlag::new(FALSE),
            v: UnsafeCell::new(MaybeUninit::new(v))
        }
    }

    #[inline(always)]
    pub const fn new_taken () -> Self {
        Self {
            taken: InnerFlag::new(TRUE),
            v: UnsafeCell::new(MaybeUninit::uninit())
        }
    }

    #[inline(always)]
    pub fn is_taken (&self) -> bool {
        self.taken.load(Ordering::Relaxed) == TRUE
    }

    #[inline]
    pub fn try_take (&self) -> Option<T> {
        if self.taken.compare_exchange(FALSE, TRUE, Ordering::AcqRel, Ordering::Acquire).is_ok() {
            unsafe {
                let v = &*self.v.get();
                return Some(v.assume_init_read())
            }
        }   
        
        None
    }

    #[inline]
    pub fn try_take_mut (&mut self) -> Option<T> {
        let taken = self.taken.get_mut();
        if *taken == FALSE {
            *taken = TRUE;

            unsafe {
                return Some(self.v.get_mut().assume_init_read())
            }
        }

        None
    }
}

impl<T> Drop for TakeCell<T> {
    #[inline(always)]
    fn drop(&mut self) {
        if needs_drop::<T>() && *self.taken.get_mut() == FALSE {
            unsafe {
                self.v.get_mut().assume_init_drop()
            }
        }
    }
}

unsafe impl<T: Send> Send for TakeCell<T> {}
unsafe impl<T: Sync> Sync for TakeCell<T> {}