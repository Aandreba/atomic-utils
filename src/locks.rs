#[cfg(not(feature = "nightly"))]
use core::marker::PhantomData;
use core::{fmt::Debug, mem::ManuallyDrop};

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        /// A synchronization primitive that can be used to coordinate threads.
        ///
        /// `Lock` is a type that represents a lock, which can be used to ensure that only one thread
        /// can access a shared resource at a time.
        ///
        /// # Example
        ///
        /// ```
        /// use utils_atomics::{Lock, lock};
        ///
        /// let (lock, lock_sub) = lock();
        /// std::thread::spawn(move || {
        ///     // Do some work with the shared resource
        ///     lock.wake();
        /// });
        ///
        /// // Do some work with the shared resource
        /// lock_sub.wait();
        /// ```
        #[derive(Debug)]
        #[repr(transparent)]
        pub struct Lock (std::thread::Thread);

        /// A helper type used for coordination with the `Lock`.
        ///
        /// `LockSub` is used in conjunction with a `Lock` to provide a way to wait for the lock to be
        /// released.
        #[derive(Debug)]
        pub struct LockSub ((), #[cfg(not(feature = "nightly"))] PhantomData<*mut ()>);

        impl Lock {
            /// Transforms the `Lock` into a raw mutable pointer.
            #[inline]
            pub fn into_raw (self) -> *mut () {
                static_assertions::assert_eq_align!(Lock, *mut ());
                return unsafe { core::mem::transmute(self) }
            }

            /// Constructs a `Lock` from a raw mutable pointer.
            ///
            /// # Safety
            ///
            /// This function is unsafe because it assumes the provided pointer is valid and points to a
            /// `Lock`.
            #[inline]
            pub unsafe fn from_raw (raw: *mut ()) -> Self {
                static_assertions::assert_eq_align!(Lock, *mut ());
                return Self(core::mem::transmute(raw))
            }

            /// Drops the `Lock` without waking up the waiting threads.
            /// This method currently leaks memory when the `std` feature is disabled.
            #[inline]
            pub fn silent_drop (self) {
                let mut this = ManuallyDrop::new(self);
                unsafe { core::ptr::drop_in_place(&mut this.0) }
            }
        }

        impl LockSub {
            /// Blocks the current thread until the associated `Lock` is dropped.
            ///
            /// # Example
            ///
            /// ```
            /// use utils_atomics::{Lock, lock};
            ///
            /// let (lock, lock_sub) = lock();
            /// std::thread::spawn(move || {
            ///     // Do some work with the shared resource
            ///     lock.wake();
            /// });
            ///
            /// // Do some work with the shared resource
            /// lock_sub.wait();
            /// ```
            #[allow(clippy::unused_self)]
            #[inline]
            pub fn wait (self) {
                std::thread::park();
            }

            /// Blocks the current thread for a specified duration or until the associated `Lock` is dropped,
            /// whichever comes first.
            ///
            /// # Example
            ///
            /// ```
            /// use utils_atomics::{Lock, lock};
            /// use core::time::Duration;
            /// use std::time::Instant;
            ///
            /// let (lock, lock_sub) = lock();
            /// let handle = std::thread::spawn(move || {
            ///     // Do some work with the shared resource
            ///     std::thread::sleep(Duration::from_secs(3));
            ///     lock.wake();
            /// });
            ///
            /// let start = Instant::now();
            /// lock_sub.wait_timeout(Duration::from_secs(2));
            /// assert!(start.elapsed() >= Duration::from_secs(2));
            /// handle.join().unwrap();
            /// ```
            #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
            #[allow(clippy::unused_self)]
            #[inline]
            pub fn wait_timeout (self, dur: core::time::Duration) {
                std::thread::park_timeout(dur);
            }
        }

        impl Drop for Lock {
            #[inline]
            fn drop (&mut self) {
                self.0.unpark();
            }
        }

        /// Acquires a `Lock` and its corresponding `LockSub` for coordinating access to a shared resource.
        ///
        /// # Example
        ///
        /// ```
        /// use utils_atomics::{Lock, lock};
        ///
        /// let (lock, lock_sub) = lock();
        /// std::thread::spawn(move || {
        ///     // Do some work with the shared resource
        ///     lock.wake();
        /// });
        ///
        /// // Do some work with the shared resource
        /// lock_sub.wait();
        /// ```
        #[inline]
        pub fn lock () -> (Lock, LockSub) {
            return (Lock(std::thread::current()), LockSub((), #[cfg(not(feature = "nightly"))] PhantomData))
        }
    } else {
        use alloc::sync::Arc;

        /// A synchronization primitive that can be used to coordinate threads.
        ///
        /// `Lock` is a type that represents a lock, which can be used to ensure that only one thread
        /// can access a shared resource at a time.
        ///
        /// # Example
        ///
        /// ```
        /// use utils_atomics::{Lock, lock};
        ///
        /// let (lock, lock_sub) = lock();
        /// std::thread::spawn(move || {
        ///     // Do some work with the shared resource
        ///     lock.wake();
        /// });
        ///
        /// // Do some work with the shared resource
        /// lock_sub.wait();
        /// ```
        #[derive(Debug)]
        #[repr(transparent)]
        pub struct Lock (alloc::sync::Arc<()>);

        /// A helper type used for coordination with the `Lock`.
        ///
        /// `LockSub` is used in conjunction with a `Lock` to provide a way to wait for the lock to be
        /// released.
        #[derive(Debug)]
        pub struct LockSub (alloc::sync::Arc<()>, #[cfg(not(feature = "nightly"))] PhantomData<*mut ()>);

        impl Lock {
            /// Transforms the `Lock` into a raw mutable pointer.
            #[inline]
            pub fn into_raw (self) -> *mut () {
                let this = ManuallyDrop::new(self);
                return unsafe { Arc::into_raw(core::ptr::read(&this.0)).cast_mut() }
            }

            /// Constructs a `Lock` from a raw mutable pointer.
            ///
            /// # Safety
            ///
            /// This function is unsafe because it assumes the provided pointer is valid and points to a
            /// `Lock`.
            #[inline]
            pub unsafe fn from_raw (raw: *mut ()) -> Self {
                return Self(Arc::from_raw(raw.cast_const()))
            }

            /// Drops the `Lock` without waking up the waiting threads.
            /// This method currently leaks memory when the `std` feature is disabled.
            #[inline]
            pub fn silent_drop (self) {
                core::mem::forget(self);
            }
        }

        impl LockSub {
            /// Blocks the current thread until the associated `Lock` is dropped.
            ///
            /// # Example
            ///
            /// ```
            /// use utils_atomics::{Lock, lock};
            ///
            /// let (lock, lock_sub) = lock();
            /// let handle = std::thread::spawn(move || {
            ///     // Do some work with the shared resource
            ///     lock.wake();
            /// });
            ///
            /// // Do some work with the shared resource
            /// lock_sub.wait();
            /// handle.join().unwrap();
            /// ```
            #[inline]
            pub fn wait (self) {
                let mut this = self.0;
                loop {
                    match alloc::sync::Arc::try_unwrap(this) {
                        Ok(_) => return,
                        Err(e) => this = e
                    }
                    core::hint::spin_loop()
                }
            }
        }

        /// Acquires a `Lock` and its corresponding `LockSub` for coordinating access to a shared resource.
        ///
        /// # Example
        ///
        /// ```
        /// use utils_atomics::{Lock, lock};
        ///
        /// let (lock, lock_sub) = lock();
        /// std::thread::spawn(move || {
        ///     // Do some work with the shared resource
        ///     lock.wake();
        /// });
        ///
        /// // Do some work with the shared resource
        /// lock_sub.wait();
        /// ```
        #[inline]
        pub fn lock () -> (Lock, LockSub) {
            let lock = alloc::sync::Arc::new(());
            return (Lock(lock.clone()), LockSub(lock, #[cfg(not(feature = "nightly"))] PhantomData))
        }

        impl Drop for Lock {
            #[inline]
            fn drop (&mut self) {}
        }
    }
}

impl Lock {
    /// Wakes up the waiting threads associated with the `Lock`.
    ///
    /// # Example
    ///
    /// ```
    /// use utils_atomics::{Lock, lock};
    ///
    /// let (lock, lock_sub) = lock();
    /// std::thread::spawn(move || {
    ///     // Do some work with the shared resource
    ///     lock.wake();
    /// });
    ///
    /// // Do some work with the shared resource
    /// lock_sub.wait();
    /// ```
    #[allow(clippy::unused_self)]
    #[inline]
    pub fn wake(self) {}
}

cfg_if::cfg_if! {
    if #[cfg(feature = "nightly")] {
        impl !Send for LockSub {}
    } else {
        unsafe impl Sync for LockSub {}
    }
}
