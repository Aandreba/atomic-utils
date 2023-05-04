use crate::locks::Lock;
use core::{
    ops::Deref,
    sync::atomic::{AtomicIsize, Ordering},
};
use crossbeam::queue::ArrayQueue;

/// Maximum amount of permits per [`Semaphore`]
pub const MAX_PERMITS: usize = isize::MAX as usize;

pub enum SemaphoreError {
    TooManyPermits,
}

pub struct Semaphore {
    permits: AtomicIsize,
    queue: ArrayQueue<Lock>,
}

pub struct SemaphorePermit<D: Deref<Target = Semaphore>> {
    parent: D,
    n: isize,
}

impl Semaphore {
    #[inline]
    pub fn try_acquire_by_deref<D: Deref<Target = Self>>(
        this: D,
    ) -> Result<Option<SemaphorePermit<D>>, SemaphoreError> {
        Self::try_acquire_many_by_deref(this, 1)
    }

    pub fn try_acquire_many_by_deref<D: Deref<Target = Self>>(
        this: D,
        n: usize,
    ) -> Result<Option<SemaphorePermit<D>>, SemaphoreError> {
        let Ok(n) = isize::try_from(n) else { return Err(SemaphoreError::TooManyPermits) };

        let prev = this.permits.fetch_sub(n, Ordering::AcqRel);
        if prev < n {
            this.permits.fetch_add(prev, Ordering::Release);
            return Ok(None);
        }

        return Ok(Some(SemaphorePermit { parent: this, n }));
    }
}

impl<D: Deref<Target = Semaphore>> Drop for SemaphorePermit<D> {
    #[inline]
    fn drop(&mut self) {
        self.parent.permits.fetch_add(self.n, Ordering::Release);
    }
}
