use alloc::sync::{Arc, Weak};
use crate::{FillQueue};

#[cfg(feature = "std")]
type Lock = std::thread::Thread;
#[cfg(not(feature = "std"))]
type Lock = Arc<()>;

#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
/// A flag type that completes when marked or dropped
pub struct Flag {
    #[allow(unused)]
    inner: Arc<FlagQueue>
}

#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
/// Subscriber of a [`Flag`]
#[derive(Clone)]
pub struct Subscribe {
    inner: Weak<FlagQueue>
}

impl Flag {
    /// See [`Arc::into_raw`]
    #[inline(always)]
    pub unsafe fn into_raw (self) -> *const FillQueue<Lock> {
        Arc::into_raw(self.inner).cast()
    }

    /// See [`Arc::from_raw`]
    #[inline(always)]
    pub unsafe fn from_raw (ptr: *const FillQueue<Lock>) -> Self {
        Self { inner: Arc::from_raw(ptr.cast()) }
    }

    /// Mark this flag as completed, consuming it
    #[inline(always)]
    pub fn mark (self) {}
}

impl Subscribe {
    // Blocks the current thread until the flag gets marked.
    #[inline(always)]
    pub fn subscribe (&self) {
        #[cfg(feature = "std")] {
            if let Some(queue) = self.inner.upgrade() {
                queue.0.push(std::thread::current());
                drop(queue);
                std::thread::park();
            }
        }

        #[cfg(not(feature = "std"))] {
            if let Some(queue) = self.inner.upgrade() {
                let mut lock = Arc::new(());
                queue.0.push(lock.queue());
                drop(queue);
                
                loop {
                    match Arc::try_unwrap(lock) {
                        Ok(_) => break,
                        Err(e) => lock = e
                    }
                    // core::hint::spin_loop();
                }
            }
        }
    }
}

#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
/// Creates a new pair of [`Flag`] and [`Subscribe`]
pub fn flag () -> (Flag, Subscribe) {
    let flag = Arc::new(FlagQueue(FillQueue::new()));
    let sub = Arc::downgrade(&flag);
    (Flag { inner: flag }, Subscribe { inner: sub })
}

#[repr(transparent)]
struct FlagQueue (pub FillQueue<Lock>);

impl Drop for FlagQueue {
    #[inline(always)]
    fn drop(&mut self) {
        #[cfg(feature = "std")]
        self.0.chop_mut().for_each(|x| x.unpark());
        #[cfg(not(feature = "std"))]
        self.0.chop_mut().for_each(|x| x.store(crate::TRUE, core::sync::atomic::Ordering::Release));
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "futures")] {
        use core::{future::Future, task::{Waker, Poll}};
        use futures::future::FusedFuture;

        #[cfg_attr(docsrs, doc(cfg(all(feature = "alloc", feature = "futures"))))]
        /// Async flag that completes when marked or droped.
        pub struct AsyncFlag {
            inner: Arc<AsyncFlagQueue>
        }

        impl AsyncFlag {
            /// Creates a new flag
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
        #[derive(Clone)]
        pub struct AsyncSubscribe {
            inner: Option<Weak<AsyncFlagQueue>>
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
        struct AsyncFlagQueue (pub FillQueue<Waker>);

        impl Drop for AsyncFlagQueue {
            #[inline(always)]
            fn drop(&mut self) {
                self.0.chop_mut().for_each(Waker::wake);
            }
        }
    }
}