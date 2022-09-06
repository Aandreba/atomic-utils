cfg_if::cfg_if! {
    if #[cfg(feature = "futures")] {
        use core::{future::Future, task::{Waker, Poll}, mem::ManuallyDrop};
        use alloc::sync::{Arc, Weak};
        use futures::future::FusedFuture;
        use crate::{FillQueue};

        /// Async flag that completes when marked.
        pub struct AsyncFlag {
            wakers: Weak<FlagQueue>
        }

        impl AsyncFlag {
            pub fn new () -> Self {
                let _wakers = Arc::new(FlagQueue(FillQueue::new()));
                let wakers = Arc::downgrade(&_wakers);
                core::mem::forget(_wakers);
                
                Self { wakers }
            }

            #[inline(always)]
            pub fn mark (self) {
                let this = ManuallyDrop::new(self);
                unsafe {
                    Arc::decrement_strong_count(Weak::into_raw(core::ptr::read(&this.wakers)));
                }
            }

            #[inline(always)]
            pub fn subscribe (&self) -> Subscribe {
                Subscribe {
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

        #[derive(Clone)]
        pub struct Subscribe {
            inner: Option<Weak<FlagQueue>>
        }

        impl Future for Subscribe {
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

        impl FusedFuture for Subscribe {
            #[inline(always)]
            fn is_terminated(&self) -> bool {
                self.inner.is_none()
            }
        }

        struct FlagQueue (pub FillQueue<Waker>);

        impl Drop for FlagQueue {
            #[inline(always)]
            fn drop(&mut self) {
                self.0.chop_mut().for_each(Waker::wake);
            }
        }
    }
}