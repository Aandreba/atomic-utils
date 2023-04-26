use crate::locks::{lock, Lock};
use alloc::sync::{Arc, Weak};
use core::{cell::UnsafeCell, fmt::Debug};

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
    #[inline(always)]
    pub unsafe fn into_raw(self) -> *const () {
        Arc::into_raw(self.inner).cast()
    }

    /// See [`Arc::from_raw`]
    #[inline(always)]
    pub unsafe fn from_raw(ptr: *const ()) -> Self {
        Self {
            inner: Arc::from_raw(ptr.cast()),
        }
    }

    /// Mark this flag reference as completed, consuming it
    #[inline(always)]
    pub fn mark(self) {}

    /// Drops the flag without marking it as completed.
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
            #[inline(always)]
            pub unsafe fn into_raw (self) -> *const Option<Waker> {
                Arc::into_raw(self.inner).cast()
            }

            /// See [`Arc::from_raw`]
            #[inline(always)]
            pub unsafe fn from_raw (ptr: *const Option<Waker>) -> Self {
                Self { inner: Arc::from_raw(ptr.cast()) }
            }

            /// Marks this flag as complete, consuming it
            #[inline(always)]
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
                    } else {
                        self.inner = None;
                        return Poll::Ready(())
                    }
                }
                return Poll::Ready(())
            }
        }

        impl FusedFuture for AsyncSubscribe {
            #[inline(always)]
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
