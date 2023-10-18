use crate::{
    locks::{lock, Lock},
    FillQueue,
};
use alloc::sync::{Arc, Weak};

/// Creates a new notifier and a listener to it.
pub fn notify() -> (Notify, Listener) {
    let inner = Arc::new(Inner {
        wakers: FillQueue::new(),
    });

    let listener = Listener {
        inner: Arc::downgrade(&inner),
    };
    return (Notify { inner }, listener);
}

#[derive(Debug)]
struct Inner {
    wakers: FillQueue<Lock>,
}

/// Synchronous notifier. This structure can be used not block threads until desired,
/// at which point all waiting threads can be awaken with [`notify_all`](Notify::notify_all).
///
/// This structure drops loudly by default (a.k.a it will awake blocked threads when dropped),
/// but can be droped silently via [`silent_drop`](Notify::silent_drop)
#[derive(Debug, Clone)]
pub struct Notify {
    inner: Arc<Inner>,
}

#[derive(Debug, Clone)]
pub struct Listener {
    inner: Weak<Inner>,
}

impl Notify {
    pub unsafe fn into_raw(self) -> *const () {
        Arc::into_raw(self.inner).cast()
    }

    pub unsafe fn from_raw(ptr: *const ()) -> Self {
        Self {
            inner: Arc::from_raw(ptr.cast()),
        }
    }

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
    pub fn silent_drop(self) {
        if let Ok(mut inner) = Arc::try_unwrap(self.inner) {
            inner.wakers.chop_mut().for_each(Lock::silent_drop);
        }
    }
}

impl Listener {
    pub unsafe fn into_raw(self) -> *const () {
        Weak::into_raw(self.inner).cast()
    }

    pub unsafe fn from_raw(ptr: *const ()) -> Self {
        Self {
            inner: Weak::from_raw(ptr.cast()),
        }
    }

    #[inline]
    pub fn listeners(&self) -> usize {
        return Weak::weak_count(&self.inner);
    }

    #[inline]
    pub fn recv(&self) {
        let _: bool = self.try_recv();
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

        /// Creates a new async notifier and a listener to it.
        pub fn async_notify() -> (AsyncNotify, AsyncListener) {
            let inner = Arc::new(AsyncInner {
                wakers: FillQueue::new(),
            });

            let listener = AsyncListener {
                inner: Some(Arc::downgrade(&inner)),
                sub: None
            };

            return (AsyncNotify { inner }, listener);
        }

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
            pub unsafe fn into_raw(self) -> *const () {
                Arc::into_raw(self.inner).cast()
            }

            pub unsafe fn from_raw(ptr: *const ()) -> Self {
                Self {
                    inner: Arc::from_raw(ptr.cast()),
                }
            }

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

            fn poll_next(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Option<Self::Item>> {
                if let Some(ref mut sub) = self.sub {
                    return match sub.poll_unpin(cx) {
                        Poll::Ready(_) => {
                            self.sub = None;
                            Poll::Ready(Some(()))
                        },
                        Poll::Pending => Poll::Pending
                    }
                } else if let Some(inner) = self.inner.as_ref().and_then(Weak::upgrade) {
                    let (flag, sub) = async_flag();
                    inner.wakers.push(flag);
                    self.sub = Some(sub);
                    return self.poll_next(cx)
                }

                self.inner = None;
                return core::task::Poll::Ready(None)
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

// Thanks ChatGPT!
#[cfg(all(feature = "std", test))]
mod tests {
    use super::notify;
    use std::{
        thread::{self},
        time::Duration,
    };

    #[test]
    fn test_basic_functionality() {
        let (notify, listener) = notify();
        assert_eq!(notify.listeners(), 1);

        let listener2 = notify.listen();
        assert_eq!(notify.listeners(), 2);

        let handle = thread::spawn(move || {
            listener2.recv();
        });

        thread::sleep(Duration::from_millis(100));
        notify.notify_all();
        handle.join().unwrap();

        assert_eq!(notify.listeners(), 1);
        drop(listener);
    }

    #[test]
    fn test_multi_threaded() {
        use std::sync::{Arc, Barrier};
        use std::thread::JoinHandle;

        let (notify, listener) = notify();
        let barrier = Arc::new(Barrier::new(11));
        let mut handles = vec![];

        for _ in 0..10 {
            let barrier_clone = Arc::clone(&barrier);
            let listener_clone = listener.clone();
            handles.push(thread::spawn(move || {
                barrier_clone.wait();
                listener_clone.recv();
            }));
        }

        barrier.wait();
        thread::sleep(Duration::from_millis(100));
        notify.notify_all();

        handles
            .into_iter()
            .map(JoinHandle::join)
            .for_each(Result::unwrap);

        assert_eq!(listener.listeners(), 1);
    }
}

#[cfg(all(feature = "futures", test))]
mod async_tests {
    use crate::notify::async_notify;
    use core::time::Duration;
    use futures::stream::StreamExt;

    #[tokio::test]
    async fn test_basic_functionality_async_tokio() {
        let (notify, listener) = async_notify();
        assert_eq!(notify.listeners(), 1);

        let mut listener2 = notify.listen();
        let handle = tokio::spawn(async move {
            assert_eq!(listener2.next().await, Some(()));
        });

        tokio::time::sleep(Duration::from_millis(100)).await;
        notify.notify_all();

        drop(listener);
        handle.await.unwrap();
        assert_eq!(notify.listeners(), 0);
    }

    #[tokio::test]
    async fn test_multi_task_async_tokio() {
        let (notify, listener) = async_notify();
        let mut handles = vec![];

        for _ in 0..10 {
            let mut listener_clone = listener.clone();
            let handle = tokio::spawn(async move {
                assert_eq!(listener_clone.next().await, Some(()));
            });

            handles.push(handle);
        }

        drop(listener);
        tokio::time::sleep(Duration::from_millis(100)).await;
        notify.notify_all();

        let _ = futures::future::try_join_all(handles).await.unwrap();
        assert_eq!(notify.listeners(), 0);
    }
}
