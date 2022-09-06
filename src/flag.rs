use core::{mem::ManuallyDrop};
use alloc::sync::{Arc, Weak};
use crate::{FillQueue};

#[cfg(feature = "std")]
type Lock = std::thread::Thread;
#[cfg(not(feature = "std"))]
type Lock = Arc<()>;

/// A flag type that completes when marked or dropped
pub struct Flag {
    inner: Weak<FlagQueue>
}

/// Subscriber of a [`Flag`]
#[derive(Clone)]
pub struct Subscribe {
    inner: Weak<FlagQueue>
}

impl Flag {
    /// Mark this flag as completed, consuming it
    #[inline(always)]
    pub fn mark (self) {
        let this = ManuallyDrop::new(self);
        unsafe {
            Arc::decrement_strong_count(Weak::into_raw(core::ptr::read(&this.inner)));
        }
    }
}

impl Drop for Flag {
    #[inline(always)]
    fn drop(&mut self) {
        unsafe {
            Arc::decrement_strong_count(Weak::into_raw(self.inner.clone()));
        }
    }
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

/// Creates a new pair of [`Flag`] and [`Subscribe`]
pub fn flag () -> (Flag, Subscribe) {
    let _wakers = Arc::new(FlagQueue(FillQueue::new()));
    let wakers = Arc::downgrade(&_wakers);
    core::mem::forget(_wakers);
    (Flag { inner: wakers.clone() }, Subscribe { inner: wakers })
}

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

        /// Async flag that completes when marked or droped.
        pub struct AsyncFlag {
            wakers: Weak<AsyncFlagQueue>
        }

        impl AsyncFlag {
            /// Creates a new flag
            pub fn new () -> Self {
                let _wakers = Arc::new(AsyncFlagQueue(FillQueue::new()));
                let wakers = Arc::downgrade(&_wakers);
                core::mem::forget(_wakers);
                
                Self { wakers }
            }

            /// Marks this flag as complete, consuming it
            #[inline(always)]
            pub fn mark (self) {
                let this = ManuallyDrop::new(self);
                unsafe {
                    Arc::decrement_strong_count(Weak::into_raw(core::ptr::read(&this.wakers)));
                }
            }

            /// Creates a new subscriber to this flag.
            #[inline(always)]
            pub fn subscribe (&self) -> AsyncSubscribe {
                AsyncSubscribe {
                    inner: Some(self.wakers.clone())
                }
            }
        }

        impl Drop for AsyncFlag {
            #[inline(always)]
            fn drop(&mut self) {
                unsafe {
                    Arc::decrement_strong_count(Weak::into_raw(self.wakers.clone()));
                }
            }
        }

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

        struct AsyncFlagQueue (pub FillQueue<Waker>);

        impl Drop for AsyncFlagQueue {
            #[inline(always)]
            fn drop(&mut self) {
                self.0.chop_mut().for_each(Waker::wake);
            }
        }
    }
}