use crate::{
    locks::{lock, Lock},
    FillQueue,
};
use alloc::sync::{Arc, Weak};

/// Creates a new notifier and a listener to it.
pub fn notify () -> (Notify, Listener) {
    let inner = Arc::new(Inner {
        wakers: FillQueue::new(),
    });

    let listener = Listener { inner: Arc::downgrade(&inner) };
    return (
        Notify { inner },
        listener
    )
}

#[derive(Debug)]
struct Inner {
    wakers: FillQueue<Lock>,
}

/// Synchronous notifier. This structure can be used not block threads until desired,
/// at which point all waiting threads can be awaken with [`notify_all`](Notify::notify_all).
/// 
/// This structure drops loudly by default (a.k.a it will awake blocked threads when dropped),
/// but can be droped silently via [`silent_drop`](Notify::loud_drop)
#[derive(Debug, Clone)]
pub struct Notify {
    inner: Arc<Inner>,
}

#[derive(Debug, Clone)]
pub struct Listener {
    inner: Weak<Inner>,
}

impl Notify {
    #[inline]
    pub fn listeners(&self) -> usize {
        return Arc::weak_count(&self.inner);
    }

    #[inline]
    pub fn notify_all(&self) {
        self.inner.wakers.chop().for_each(Lock::wake)
    }

    #[inline]
    pub fn listen(&self) -> Listener {
        return Listener {
            inner: Arc::downgrade(&self.inner),
        };
    }

    /// Drops the notifier without awaking blocked threads.
    /// This method may leak memory.
    #[inline]
    pub fn silent_drop (self) {
        if let Ok(mut inner) = Arc::try_unwrap(self.inner) {
            inner.wakers.chop_mut().for_each(Lock::silent_drop);
        }
    }
}

impl Listener {
    #[inline]
    pub fn listeners(&self) -> usize {
        return Weak::weak_count(&self.inner);
    }

    #[inline]
    pub fn recv(&self) {
        let _ = self.try_recv();
    }

    #[inline]
    pub fn try_recv(&self) -> bool {
        if let Some(inner) = self.inner.upgrade() {
            let (lock, sub) = lock();
            inner.wakers.push(lock);
            sub.wait();
            return true;
        }
        return false;
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "futures")] {
        use futures::{FutureExt, Stream};
        use crate::flag::mpsc::{AsyncFlag, AsyncSubscribe, async_flag};
        use core::task::Poll;
        use futures::stream::FusedStream;

        #[derive(Debug)]
        struct AsyncInner {
            wakers: FillQueue<AsyncFlag>,
        }

        /// Synchronous notifier. This structure can be used not block tasks until desired,
        /// at which point all waiting tasks can be awaken with [`notify_all`](AsyncNotify::notify_all).
        /// 
        /// This structure drops loudly by default (a.k.a it will awake blocked tasks when dropped),
        /// but can be droped silently via [`silent_drop`](AsyncNotify::silent_drop)
        #[derive(Debug, Clone)]
        pub struct AsyncNotify {
            inner: Arc<AsyncInner>,
        }

        #[derive(Debug)]
        pub struct AsyncListener {
            inner: Option<Weak<AsyncInner>>,
            sub: Option<AsyncSubscribe>
        }

        impl AsyncNotify {
            #[inline]
            pub fn listeners(&self) -> usize {
                return Arc::weak_count(&self.inner);
            }

            #[inline]
            pub fn notify_all(&self) {
                self.inner.wakers.chop().for_each(AsyncFlag::mark)
            }

            #[inline]
            pub fn listen(&self) -> AsyncListener {
                return AsyncListener {
                    inner: Some(Arc::downgrade(&self.inner)),
                    sub: None
                };
            }

            /// Drops the notifier without awaking blocked tasks.
            /// This method may leak memory.
            #[inline]
            pub fn silent_drop (self) {
                if let Ok(mut inner) = Arc::try_unwrap(self.inner) {
                    inner.wakers.chop_mut().for_each(AsyncFlag::silent_drop);
                }
            }
        }

        impl AsyncListener {
            #[inline]
            pub fn listeners(&self) -> usize {
                return match self.inner {
                    Some(ref inner) => Weak::weak_count(inner),
                    None => 0
                }
            }
        }

        impl Stream for AsyncListener {
            type Item = ();

            #[inline]
            fn poll_next(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Option<Self::Item>> {
                if self.sub.is_none() {
                    if let Some(inner) = self.inner.as_ref().and_then(Weak::upgrade) {
                        let (flag, sub) = async_flag();
                        inner.wakers.push(flag);
                        self.sub = Some(sub);
                    } else {
                        self.inner = None;
                        return core::task::Poll::Ready(None)
                    }
                }

                let sub = unsafe { self.sub.as_mut().unwrap_unchecked() };
                return match sub.poll_unpin(cx) {
                    Poll::Ready(_) => {
                        self.sub = None;
                        return Poll::Ready(Some(()))
                    },
                    Poll::Pending => Poll::Pending
                }
            }

            #[inline]
            fn size_hint(&self) -> (usize, Option<usize>) {
                match (&self.inner, &self.sub) {
                    (None, None) => (0, Some(0)),
                    (Some(inner), None) if inner.upgrade().is_none() => (0, Some(0)),
                    (None, Some(_)) => (1, Some(1)),
                    (Some(inner), Some(_)) if inner.upgrade().is_none() => (1, Some(1)),
                    (Some(_), Some(_)) => (1, None),
                    _ => (0, None)
                }
            }
        }

        impl FusedStream for AsyncListener {
            #[inline]
            fn is_terminated(&self) -> bool {
                match (&self.inner, &self.sub) {
                    (_, Some(_)) => false,
                    (None, None) => true,
                    (Some(inner), None) => inner.upgrade().is_none(),
                }
            }
        }

        impl Clone for AsyncListener {
            #[inline]
            fn clone(&self) -> Self {
                return Self {
                    inner: self.inner.clone(),
                    sub: None
                }
            }
        }
    }
}
