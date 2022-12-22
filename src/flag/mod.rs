#[cfg(feature = "std")]
pub(super) type Lock = std::thread::Thread;
#[cfg(not(feature = "std"))]
pub(super) type Lock = alloc::sync::Arc<()>;

/// Multiple producer - Multiple consumer flag
pub mod mpmc;

/// Multiple producer - Single consumer flag. Can also be used as a SPSC flag
pub mod mpsc;

/// Single producer - Single consumer flag.
#[deprecated(since = "0.4.1", note = "use `mpsc` instead")]
pub mod spsc {
    pub use super::mpsc::*;
}

// Legacy
pub use mpmc::*;

#[inline]
pub(super) fn lock_new () -> Lock {
    #[cfg(feature = "std")]
    return std::thread::current();
    #[cfg(not(feature = "std"))]
    return alloc::sync::Arc::new(())
}

#[inline]
pub(super) fn lock_wake (#[allow(unused)] lock: Lock) {
    #[cfg(feature = "std")]
    lock.unpark();
} 