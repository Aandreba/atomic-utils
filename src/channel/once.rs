use crate::flag::mpsc::*;
use alloc::sync::{Arc, Weak};
use core::cell::UnsafeCell;

struct Inner<T> {
    v: UnsafeCell<Option<T>>,
}
unsafe impl<T: Send> Send for Inner<T> {}
unsafe impl<T: Sync> Sync for Inner<T> {}

/// A channel sender that can only send a single value  
pub struct Sender<T> {
    inner: Weak<Inner<T>>,
    flag: Flag,
}

/// A channel receiver that can only receive a single value  
pub struct Receiver<T> {
    inner: Arc<Inner<T>>,
    sub: Subscribe,
}

impl<T> Sender<T> {
    /// Sends the value through the channel.
    #[inline]
    pub fn send(self, t: T) {
        let _ = self.try_send(t);
    }

    /// Attempts to send the value through the channel, returning `Ok` if successfull, and `Err(t)` otherwise.
    ///
    /// # Errors
    /// This method returns an error if the channel has already been used or closed.
    pub fn try_send(self, t: T) -> Result<(), T> {
        if let Some(inner) = self.inner.upgrade() {
            unsafe { *inner.v.get() = Some(t) };
            self.flag.mark();
            return Ok(());
        }
        return Err(t);
    }
}

impl<T> Receiver<T> {
    /// Blocks the current thread until the value is received.
    /// If [`Sender`] is dropped before it sends the value, this method returns `None`.
    #[inline]
    pub fn wait(self) -> Option<T> {
        self.sub.wait();
        return unsafe { &mut *self.inner.v.get() }.take();
    }
}

/// Creates a new single-value channel
pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let inner = Arc::new(Inner {
        v: UnsafeCell::new(None),
    });
    let (flag, sub) = crate::flag::mpsc::flag();

    return (
        Sender {
            inner: Arc::downgrade(&inner),
            flag,
        },
        Receiver { inner, sub },
    );
}

cfg_if::cfg_if! {
    if #[cfg(feature = "futures")] {
        /// An asynchronous channel sender that can only send a single value
        #[cfg_attr(docsrs, doc(cfg(all(feature = "alloc", feature = "futures"))))]
        pub struct AsyncSender<T> {
            inner: Weak<Inner<T>>,
            flag: AsyncFlag
        }

        pin_project_lite::pin_project! {
            /// An asynchronous channel receiver that can only receive a single value
            #[cfg_attr(docsrs, doc(cfg(all(feature = "alloc", feature = "futures"))))]
            pub struct AsyncReceiver<T> {
                inner: Arc<Inner<T>>,
                #[pin]
                sub: AsyncSubscribe
            }
        }

        impl<T> AsyncSender<T> {
            /// Sends the value through the channel.
            #[inline]
            pub fn send (self, t: T) {
                let _ = self.try_send(t);
            }

            /// Attempts to send the value through the channel, returning `Ok` if successfull, and `Err(t)` otherwise.
            ///
            /// # Errors
            /// This method returns an error if the channel has already been used or closed.
            pub fn try_send(self, t: T) -> Result<(), T> {
                if let Some(inner) = self.inner.upgrade() {
                    unsafe { *inner.v.get() = Some(t) };
                    self.flag.mark();
                    return Ok(());
                }
                return Err(t);
            }
        }

        impl<T> futures::Future for AsyncReceiver<T> {
            type Output = Option<T>;

            #[inline]
            fn poll(self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
                let this = self.project();
                if this.sub.poll(cx).is_ready() {
                    return core::task::Poll::Ready(unsafe { &mut *this.inner.v.get() }.take())
                }
                return core::task::Poll::Pending
            }
        }

        impl<T> futures::future::FusedFuture for AsyncReceiver<T> {
            #[inline]
            fn is_terminated(&self) -> bool {
                self.sub.is_terminated()
            }
        }

        /// Creates a new async and single-value channel
        pub fn async_channel<T>() -> (AsyncSender<T>, AsyncReceiver<T>) {
            let inner = Arc::new(Inner {
                v: UnsafeCell::new(None),
            });
            let (flag, sub) = crate::flag::mpsc::async_flag();

            return (
                AsyncSender {
                    inner: Arc::downgrade(&inner),
                    flag,
                },
                AsyncReceiver { inner, sub },
            );
        }
    }
}
