#[cfg(feature = "std")]
pub(super) type Lock = std::thread::Thread;
#[cfg(not(feature = "std"))]
pub(super) type Lock = alloc::sync::Arc<()>;

/// Multiple producer - Multiple consumer flag
pub mod mpmc;

/// Single producer - Single consumer flag
pub mod spsc;

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