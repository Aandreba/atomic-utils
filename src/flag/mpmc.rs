use alloc::sync::{Arc, Weak};
use crate::{FillQueue, locks::{Lock, lock}};

/// A flag type that will be completed when all references to [`Flag`] have been dropped or marked.
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
#[derive(Debug, Clone)]
pub struct Flag {
    _inner: Arc<FlagQueue>
}

/// Subscriber of a [`Flag`]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
#[derive(Debug, Clone)]
pub struct Subscribe {
    inner: Weak<FlagQueue>
}

impl Flag {
    /// See [`Arc::into_raw`]
    #[inline(always)]
    pub unsafe fn into_raw (self) -> *const FillQueue<Lock> {
        Arc::into_raw(self._inner).cast()
    }

    /// See [`Arc::from_raw`]
    #[inline(always)]
    pub unsafe fn from_raw (ptr: *const FillQueue<Lock>) -> Self {
        Self { _inner: Arc::from_raw(ptr.cast()) }
    }

    /// Mark this flag as completed, consuming it
    #[inline(always)]
    pub fn mark (self) {}
}

impl Subscribe {
    // Blocks the current thread until the flag gets marked.
    #[deprecated(since = "0.4.0", note = "use `wait` instead")]
    #[inline]
    pub fn subscribe (&self) {
        self.wait()
    }

    /// Returns `true` if the flag has been marked, and `false` otherwise
    #[inline]
    pub fn is_marked (&self) -> bool {
        return self.inner.strong_count() == 0
    }

    // Blocks the current thread until the flag gets marked.
    #[inline]
    pub fn wait (&self) {
        if let Some(queue) = self.inner.upgrade() {
            let (waker, sub) = lock();
            queue.0.push(waker);
            sub.wait()
        }
    }
}

/// Creates a new pair of [`Flag`] and [`Subscribe`].
/// 
/// The flag will be completed when all references to [`Flag`] have been dropped or marked.
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub fn flag () -> (Flag, Subscribe) {
    let flag = Arc::new(FlagQueue(FillQueue::new()));
    let sub = Arc::downgrade(&flag);
    (Flag { _inner: flag }, Subscribe { inner: sub })
}

#[repr(transparent)]
#[derive(Debug)]
struct FlagQueue (pub FillQueue<Lock>);

impl Drop for FlagQueue {
    #[inline(always)]
    fn drop(&mut self) {
        self.0.chop_mut().for_each(Lock::wake);
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "futures")] {
        use core::{future::Future, task::{Waker, Poll}};
        use futures::future::FusedFuture;

        /// Creates a new pair of [`AsyncFlag`] and [`AsyncSubscribe`]
        #[cfg_attr(docsrs, doc(cfg(all(feature = "alloc", feature = "futures"))))]
        #[inline]
        pub fn async_flag () -> (AsyncFlag, AsyncSubscribe) {
            #[allow(deprecated)]
            let flag = AsyncFlag::new();
            let sub = flag.subscribe();
            return (flag, sub)
        }

        /// Async flag that will be completed when all references to [`Flag`] have been dropped or marked.
        #[cfg_attr(docsrs, doc(cfg(all(feature = "alloc", feature = "futures"))))]
        #[derive(Debug, Clone)]
        pub struct AsyncFlag {
            inner: Arc<AsyncFlagQueue>
        }

        impl AsyncFlag {
            /// Creates a new flag
            #[deprecated(since = "0.4.0", note = "use `async_flag` instead")]
            #[inline(always)]
            pub fn new () -> Self {
                Self { inner: Arc::new(AsyncFlagQueue(FillQueue::new())) }
            }

            /// See [`Arc::into_raw`]
            #[inline(always)]
            pub unsafe fn into_raw (self) -> *const FillQueue<Waker> {
                Arc::into_raw(self.inner).cast()
            }

            /// See [`Arc::from_raw`]
            #[inline(always)]
            pub unsafe fn from_raw (ptr: *const FillQueue<Waker>) -> Self {
                Self { inner: Arc::from_raw(ptr.cast()) }
            }

            /// Marks this flag as complete, consuming it
            #[inline(always)]
            pub fn mark (self) {}

            /// Creates a new subscriber to this flag.
            #[inline(always)]
            pub fn subscribe (&self) -> AsyncSubscribe {
                AsyncSubscribe {
                    inner: Some(Arc::downgrade(&self.inner))
                }
            }
        }

        #[cfg_attr(docsrs, doc(cfg(all(feature = "alloc", feature = "futures"))))]
        /// Subscriber of an [`AsyncFlag`]
        #[derive(Debug, Clone)]
        pub struct AsyncSubscribe {
            inner: Option<Weak<AsyncFlagQueue>>
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

            #[inline(always)]
            fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
                if let Some(ref queue) = self.inner {
                    if let Some(queue) = queue.upgrade() {
                        queue.0.push(cx.waker().clone());
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

        #[repr(transparent)]
        #[derive(Debug)]
        struct AsyncFlagQueue (pub FillQueue<Waker>);

        impl Drop for AsyncFlagQueue {
            #[inline(always)]
            fn drop(&mut self) {
                self.0.chop_mut().for_each(Waker::wake);
            }
        }
    }
}