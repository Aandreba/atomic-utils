use crate::locks::{lock, Lock};
use alloc::sync::{Arc, Weak};
use core::{cell::UnsafeCell, fmt::Debug};
use docfg::docfg;

/// Creates a new pair of [`Flag`] and [`Subscribe`]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub fn flag() -> (Flag, Subscribe) {
    let waker = FlagWaker {
        waker: UnsafeCell::new(None),
    };

    let flag = Arc::new(waker);
    let sub = Arc::downgrade(&flag);
    (Flag { inner: flag }, Subscribe { inner: sub })
}

/// A flag type that completes when all it's references are marked or dropped.
///
/// This flag drops loudly by default (a.k.a will complete when dropped),
/// but can be droped silently with [`silent_drop`](Flag::silent_drop)
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
#[derive(Debug, Clone)]
pub struct Flag {
    #[allow(unused)]
    inner: Arc<FlagWaker>,
}

/// Subscriber of a [`Flag`]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
#[derive(Debug)]
pub struct Subscribe {
    inner: Weak<FlagWaker>,
}

impl Flag {
    /// See [`Arc::into_raw`]
    #[inline]
    pub unsafe fn into_raw(self) -> *const () {
        Arc::into_raw(self.inner).cast()
    }

    /// See [`Arc::from_raw`]
    #[inline]
    pub unsafe fn from_raw(ptr: *const ()) -> Self {
        Self {
            inner: Arc::from_raw(ptr.cast()),
        }
    }

    /// Mark this flag reference as completed, consuming it
    #[inline]
    pub fn mark(self) {}

    /// Drops the flag without **notifying** it as completed.
    /// This method may leak memory.
    #[inline]
    pub fn silent_drop(self) {
        if let Ok(inner) = Arc::try_unwrap(self.inner) {
            if let Some(inner) = inner.waker.into_inner() {
                inner.silent_drop();
            }
        }
    }
}

impl Subscribe {
    /// Returns `true` if the flag has been fully marked, and `false` otherwise
    #[inline]
    pub fn is_marked(&self) -> bool {
        return self.inner.strong_count() == 0;
    }

    // Blocks the current thread until the flag gets fully marked.
    #[inline]
    pub fn wait(self) {
        if let Some(queue) = self.inner.upgrade() {
            let (lock, sub) = lock();
            unsafe { *queue.waker.get() = Some(lock) }
            drop(queue);
            sub.wait();
        }
    }

    // Blocks the current thread until the flag gets fully marked or the timeout expires
    #[docfg(feature = "std")]
    #[inline]
    pub fn wait_timeout(self, dur: core::time::Duration) {
        if let Some(queue) = self.inner.upgrade() {
            let (lock, sub) = lock();
            unsafe { *queue.waker.get() = Some(lock) }
            drop(queue);
            sub.wait_timeout(dur);
        }
    }
}

struct FlagWaker {
    waker: UnsafeCell<Option<Lock>>,
}

impl Debug for FlagWaker {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FlagWaker").finish_non_exhaustive()
    }
}

unsafe impl Send for FlagWaker where Lock: Send {}
unsafe impl Sync for FlagWaker where Lock: Sync {}

cfg_if::cfg_if! {
    if #[cfg(feature = "futures")] {
        use core::{future::Future, task::{Waker, Poll}};
        use futures::future::FusedFuture;

        /// Creates a new pair of [`AsyncFlag`] and [`AsyncSubscribe`]
        #[cfg_attr(docsrs, doc(cfg(all(feature = "alloc", feature = "futures"))))]
        #[inline]
        pub fn async_flag () -> (AsyncFlag, AsyncSubscribe) {
            let waker = AsyncFlagWaker {
                waker: UnsafeCell::new(None)
            };

            let flag = Arc::new(waker);
            let sub = Arc::downgrade(&flag);
            (AsyncFlag { inner: flag }, AsyncSubscribe { inner: Some(sub) })
        }

        /// Async flag that completes when all it's references are marked or droped.
        ///
        /// This flag drops loudly by default (a.k.a will complete when dropped),
        /// but can be droped silently with [`silent_drop`](Flag::silent_drop)
        #[cfg_attr(docsrs, doc(cfg(all(feature = "alloc", feature = "futures"))))]
        #[derive(Debug, Clone)]
        pub struct AsyncFlag {
            inner: Arc<AsyncFlagWaker>
        }

        impl AsyncFlag {
            /// See [`Arc::into_raw`]
            #[inline]
            pub unsafe fn into_raw (self) -> *const Option<Waker> {
                Arc::into_raw(self.inner).cast()
            }

            /// See [`Arc::from_raw`]
            #[inline]
            pub unsafe fn from_raw (ptr: *const Option<Waker>) -> Self {
                Self { inner: Arc::from_raw(ptr.cast()) }
            }

            /// Marks this flag as complete, consuming it
            #[inline]
            pub fn mark (self) {}

            /// Drops the flag without marking it as completed.
            /// This method may leak memory.
            #[inline]
            pub fn silent_drop (self) {
                if let Ok(inner) = Arc::try_unwrap(self.inner) {
                    inner.silent_drop();
                }
            }
        }

        #[cfg_attr(docsrs, doc(cfg(all(feature = "alloc", feature = "futures"))))]
        /// Subscriber of an [`AsyncFlag`]
        #[derive(Debug)]
        pub struct AsyncSubscribe {
            inner: Option<Weak<AsyncFlagWaker>>
        }

        impl AsyncSubscribe {
            /// Returns `true` if the flag has been marked, and `false` otherwise
            #[inline]
            pub fn is_marked (&self) -> bool {
                return !crate::is_some_and(self.inner.as_ref(), |x| x.strong_count() > 0)
            }
        }

        impl Future for AsyncSubscribe {
            type Output = ();

            #[inline]
            fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
                if let Some(ref queue) = self.inner {
                    if let Some(queue) = queue.upgrade() {
                        // SAFETY: If we upgraded, we are the only thread with access to the value,
                        //         since the only other owner of the waker is it's destructor.
                        unsafe { *queue.waker.get() = Some(cx.waker().clone()) };
                        return Poll::Pending;
                    }

                    self.inner = None;
                    return Poll::Ready(())
                }
                return Poll::Ready(())
            }
        }

        impl FusedFuture for AsyncSubscribe {
            #[inline]
            fn is_terminated(&self) -> bool {
                self.inner.is_none()
            }
        }

        struct AsyncFlagWaker {
            waker: UnsafeCell<Option<Waker>>
        }

        impl AsyncFlagWaker {
            #[inline]
            pub fn silent_drop (self) {
                let mut this = core::mem::ManuallyDrop::new(self);
                unsafe { core::ptr::drop_in_place(&mut this.waker) }
            }
        }

        impl Drop for AsyncFlagWaker {
            #[inline]
            fn drop(&mut self) {
                if let Some(waker) = self.waker.get_mut().take() {
                    waker.wake()
                }
            }
        }

        impl Debug for AsyncFlagWaker {
            #[inline]
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.debug_struct("AsyncFlagWaker").finish_non_exhaustive()
            }
        }

        unsafe impl Send for AsyncFlagWaker where Option<Waker>: Send {}
        unsafe impl Sync for AsyncFlagWaker where Option<Waker>: Sync {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "std")]
    use std::thread;

    #[test]
    fn test_flag_creation() {
        let (flag, subscribe) = flag();
        assert!(!subscribe.is_marked());
        drop(flag);
    }

    #[test]
    fn test_flag_mark() {
        let (flag, subscribe) = flag();
        flag.mark();
        assert!(subscribe.is_marked());
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_flag_silent_drop() {
        use core::time::Duration;
        use std::time::Instant;

        let (flag, subscribe) = flag();

        let handle = thread::spawn(move || {
            thread::sleep(std::time::Duration::from_millis(100));
            flag.silent_drop();
        });

        let now = Instant::now();
        subscribe.wait_timeout(std::time::Duration::from_millis(200));
        let elapsed = now.elapsed();

        handle.join().unwrap();
        assert!(elapsed >= Duration::from_millis(200), "{elapsed:?}");
    }

    #[test]
    fn test_subscribe_wait() {
        let (flag, subscribe) = flag();

        let handle = thread::spawn(move || {
            thread::sleep(std::time::Duration::from_millis(100));
            flag.mark();
        });

        subscribe.wait();
        handle.join().unwrap();
    }

    #[cfg(miri)]
    #[test]
    fn test_flag_stress() {
        const THREADS: usize = 10;
        const ITERATIONS: usize = 100;

        for _ in 0..ITERATIONS {
            let (flag, subscribe) = flag();
            let mut handles = Vec::with_capacity(THREADS);

            for _ in 0..THREADS {
                let flag_clone = flag.clone();
                let handle = std::thread::spawn(move || {
                    flag_clone.mark();
                });
                handles.push(handle);
            }

            subscribe.wait();

            for handle in handles {
                handle.join().unwrap();
            }
        }
    }

    #[cfg(feature = "futures")]
    mod async_tests {
        use super::*;

        #[test]
        fn test_async_flag_creation() {
            let (async_flag, async_subscribe) = async_flag();
            assert!(!async_subscribe.is_marked());
            drop(async_flag);
        }

        #[test]
        fn test_async_flag_mark() {
            let (async_flag, async_subscribe) = async_flag();
            async_flag.mark();
            assert!(async_subscribe.is_marked());
        }

        #[test]
        fn test_async_flag_silent_drop() {
            let (async_flag, async_subscribe) = async_flag();
            async_flag.silent_drop();
            assert!(!async_subscribe.is_marked());
        }

        #[tokio::test]
        async fn test_async_subscribe_wait() {
            let (async_flag, async_subscribe) = async_flag();
            let async_flag_clone = async_flag.clone();

            let handle = tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                async_flag_clone.mark();
            });

            // Wait for the async_flag_clone to be marked
            handle.await.unwrap();

            // Wait for the async_subscribe to complete
            async_subscribe.await;
        }
    }
}
