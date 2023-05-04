use crate::{
    locks::{lock, Lock},
    FillQueue,
};
use alloc::sync::{Arc, Weak};
use core::mem::ManuallyDrop;
use docfg::docfg;

/// A flag type that will be completed when all its references have been dropped or marked.
///
/// This flag drops loudly by default (a.k.a will complete when dropped),
/// but can be droped silently with [`silent_drop`](Flag::silent_drop)
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
#[derive(Debug, Clone)]
pub struct Flag {
    inner: Arc<FlagQueue>,
}

/// Subscriber of a [`Flag`]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
#[derive(Debug, Clone)]
pub struct Subscribe {
    inner: Weak<FlagQueue>,
}

impl Flag {
    /// See [`Arc::into_raw`]
    #[inline]
    pub unsafe fn into_raw(self) -> *const FillQueue<Lock> {
        Arc::into_raw(self.inner).cast()
    }

    /// See [`Arc::from_raw`]
    #[inline]
    pub unsafe fn from_raw(ptr: *const FillQueue<Lock>) -> Self {
        Self {
            inner: Arc::from_raw(ptr.cast()),
        }
    }

    /// Mark this flag as completed, consuming it
    #[inline]
    pub fn mark(self) {}

    /// Drops the flag without **notifying** it as completed.
    /// This method may leak memory.
    #[inline]
    pub fn silent_drop(self) {
        if let Ok(inner) = Arc::try_unwrap(self.inner) {
            inner.silent_drop()
        }
    }
}

impl Subscribe {
    /// Blocks the current thread until the flag gets marked.
    #[inline]
    pub fn wait(self) {
        if let Some(queue) = self.inner.upgrade() {
            let (waker, sub) = lock();
            queue.0.push(waker);
            drop(queue);
            sub.wait()
        }
    }

    /// Blocks the current thread until the flag gets marked or the timeout expires.
    #[docfg(feature = "std")]
    #[inline]
    pub fn wait_timeout(self, dur: core::time::Duration) {
        if let Some(queue) = self.inner.upgrade() {
            let (waker, sub) = lock();
            queue.0.push(waker);
            drop(queue);
            sub.wait_timeout(dur);
        }
    }
}

/// Creates a new pair of [`Flag`] and [`Subscribe`].
///
/// The flag will be completed when all references to [`Flag`] have been dropped or marked.
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub fn flag() -> (Flag, Subscribe) {
    let flag = Arc::new(FlagQueue(FillQueue::new()));
    let sub = Arc::downgrade(&flag);
    (Flag { inner: flag }, Subscribe { inner: sub })
}

#[repr(transparent)]
#[derive(Debug)]
struct FlagQueue(pub FillQueue<Lock>);

impl FlagQueue {
    #[inline]
    pub fn silent_drop(self) {
        let mut this = ManuallyDrop::new(self);
        this.0.chop_mut().for_each(Lock::silent_drop);
        unsafe { core::ptr::drop_in_place(&mut this) };
    }
}

impl Drop for FlagQueue {
    #[inline]
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
            let flag = Arc::new(AsyncFlagQueue(FillQueue::new()));
            let sub = Arc::downgrade(&flag);
            return (AsyncFlag { inner: flag }, AsyncSubscribe { inner: Some(sub) })
        }

        /// Async flag that will be completed when all references to [`Flag`] have been dropped or marked.
        #[cfg_attr(docsrs, doc(cfg(all(feature = "alloc", feature = "futures"))))]
        #[derive(Debug, Clone)]
        pub struct AsyncFlag {
            inner: Arc<AsyncFlagQueue>
        }

        impl AsyncFlag {
            /// See [`Arc::into_raw`]
            #[inline]
            pub unsafe fn into_raw (self) -> *const FillQueue<Waker> {
                Arc::into_raw(self.inner).cast()
            }

            /// See [`Arc::from_raw`]
            #[inline]
            pub unsafe fn from_raw (ptr: *const FillQueue<Waker>) -> Self {
                Self { inner: Arc::from_raw(ptr.cast()) }
            }

            /// Marks this flag as complete, consuming it
            #[inline]
            pub fn mark (self) {}

            /// Creates a new subscriber to this flag.
            #[inline]
            pub fn subscribe (&self) -> AsyncSubscribe {
                AsyncSubscribe {
                    inner: Some(Arc::downgrade(&self.inner))
                }
            }

            /// Drops the flag without **notifying** it as completed.
            /// This method may leak memory.
            #[inline]
            pub fn silent_drop (self) {
                if let Ok(inner) = Arc::try_unwrap(self.inner) {
                    inner.silent_drop()
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
            /// Creates a new subscriber that has already completed
            #[inline]
            pub fn marked () -> AsyncSubscribe {
                return Self { inner: None }
            }

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
                        queue.0.push(cx.waker().clone());
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

        #[derive(Debug)]
        struct AsyncFlagQueue (pub FillQueue<Waker>);

        impl AsyncFlagQueue {
            #[inline]
            pub fn silent_drop (self) {
                let mut this = ManuallyDrop::new(self);
                let _ = this.0.chop_mut();
                unsafe { core::ptr::drop_in_place(&mut this.0) }
            }
        }

        impl Drop for AsyncFlagQueue {
            #[inline]
            fn drop(&mut self) {
                self.0.chop_mut().for_each(Waker::wake);
            }
        }
    }
}

#[cfg(all(feature = "std", test))]
mod tests {
    use super::flag;
    use super::Flag;
    use core::time::Duration;
    use std::thread;
    use std::time::Instant;

    #[test]
    fn test_normal_conditions() {
        let (f, _) = flag();
        // Test marking the flag.
        f.mark();

        // Test waiting for the flag.
        let (f, s) = flag();
        let f = unsafe { Flag::from_raw(Flag::into_raw(f)) };

        thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            f.mark();
        });

        s.wait();
    }

    #[test]
    fn test_silent_drop() {
        let (f, s) = flag();

        let handle = thread::spawn(move || {
            let now = Instant::now();
            s.wait_timeout(Duration::from_millis(200));
            return now.elapsed();
        });

        std::thread::sleep(Duration::from_millis(100));
        f.silent_drop();

        let time = handle.join().unwrap();
        assert!(time >= Duration::from_millis(200), "{time:?}");
    }

    #[cfg(miri)]
    #[test]
    fn test_stressed_conditions() {
        let mut handles = Vec::new();
        let (f, s) = flag();

        for _ in 0..10 {
            let cloned_s = s.clone();
            let handle = thread::spawn(move || {
                for _ in 0..10 {
                    let cloned_s = cloned_s.clone();
                    cloned_s.wait();
                }
            });
            handles.push(handle);
        }

        thread::sleep(Duration::from_millis(100));

        for _ in 0..9 {
            f.clone().mark();
        }
        f.mark();

        for handle in handles {
            handle.join().unwrap();
        }
    }
}

#[cfg(all(feature = "futures", test))]
mod async_tests {
    use super::{async_flag, AsyncFlag};
    use core::time::Duration;
    use std::time::Instant;

    #[tokio::test]
    async fn test_async_normal_conditions() {
        let (f, s) = async_flag();
        assert_eq!(s.is_marked(), false);

        // Test marking the flag.
        f.mark();
        assert_eq!(s.is_marked(), true);

        // Test waiting for the flag.
        let (f, mut s) = async_flag();
        let f = unsafe { AsyncFlag::from_raw(AsyncFlag::into_raw(f)) };

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            f.mark();
        });

        (&mut s).await;
        assert_eq!(s.is_marked(), true);
    }

    #[tokio::test]
    async fn test_silent_drop() {
        let (f, s) = async_flag();

        let handle = tokio::spawn(tokio::time::timeout(
            Duration::from_millis(200),
            async move {
                let now = Instant::now();
                s.await;
                now.elapsed()
            },
        ));
        tokio::time::sleep(Duration::from_millis(100)).await;
        f.silent_drop();

        match handle.await.unwrap() {
            Ok(t) if t < Duration::from_millis(200) => panic!("{t:?}"),
            _ => {}
        }
    }

    #[tokio::test]
    async fn test_async_stressed_conditions() {
        let (f, s) = async_flag();
        let mut handles = Vec::new();

        for _ in 0..100 {
            let mut cloned_s = s.clone();
            let handle = tokio::spawn(async move {
                (&mut cloned_s).await;
                assert_eq!(cloned_s.is_marked(), true);
            });
            handles.push(handle);
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
        f.mark();

        for handle in handles {
            handle.await.unwrap();
        }
    }
}
