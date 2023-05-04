use alloc::sync::Arc;
use core::sync::atomic::{AtomicPtr, Ordering};

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        #[derive(Debug)]
        pub struct ArcCell<T> {
            inner: std::sync::RwLock<AtomicPtr<T>>,
        }

        impl<T> ArcCell<T> {
            #[inline]
            pub fn new(t: impl Into<Option<T>>) -> Self {
                Self::new_arced(t.into().map(Arc::new))
            }

            #[inline]
            pub fn new_arced(t: impl Into<Option<Arc<T>>>) -> Self {
                match t.into() {
                    Some(t) => Self {
                        inner: std::sync::RwLock::new(AtomicPtr::new(Arc::into_raw(t).cast_mut())),
                    },
                    None => Self {
                        inner: std::sync::RwLock::new(AtomicPtr::new(core::ptr::null_mut())),
                    },
                }
            }

            #[inline]
            pub fn replace(&self, new: impl Into<Option<T>>) -> Option<Arc<T>> {
                self.replace_arced(new.into().map(Arc::new))
            }

            #[inline]
            pub fn take(&self) -> Option<Arc<T>> {
                self.replace_arced(None)
            }

            #[inline]
            pub fn replace_arced(&self, new: impl Into<Option<Arc<T>>>) -> Option<Arc<T>> {
                let new = match new.into() {
                    Some(new) => Arc::into_raw(new).cast_mut(),
                    None => core::ptr::null_mut(),
                };

                let inner = match self.inner.read() {
                    Ok(x) => x,
                    Err(e) => e.into_inner()
                };

                let prev = inner.swap(new, Ordering::AcqRel);
                drop(inner);

                if prev.is_null() {
                    return None;
                }

                return unsafe { Some(Arc::from_raw(prev)) };
            }
        }

        impl<T> ArcCell<T> {
            #[inline]
            pub fn is_some(&self) -> bool {
                return !self.is_none();
            }

            #[inline]
            pub fn is_none(&self) -> bool {
                let inner = match self.inner.read() {
                    Ok(x) => x,
                    Err(e) => e.into_inner()
                };
                return inner.load(Ordering::Relaxed).is_null();
            }
        }

        #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
        impl<T> Clone for ArcCell<T> {
            fn clone(&self) -> Self {
                let mut inner = match self.inner.write() {
                    Ok(x) => x,
                    Err(e) => e.into_inner()
                };

                let ptr = *inner.get_mut();
                if !ptr.is_null() {  }
                drop(inner);

                let res = Self {
                    inner: self.inner.clone(),
                    clone_lock: std::sync::RwLock::new(()),
                };
                drop(guard);
                return res;
            }
        }

        impl<T> Drop for ArcCell<T> {
            fn drop(&mut self) {
                unsafe {
                    let ptr = *self.inner.get_mut();
                    if !ptr.is_null() {
                        let _ = Arc::from_raw(ptr);
                    }
                }
            }
        }
    } else {
        #[derive(Debug)]
        pub struct ArcCell<T> {
            inner: AtomicPtr<T>,
        }

        impl<T> ArcCell<T> {
            #[inline]
            pub fn new(t: impl Into<Option<T>>) -> Self {
                Self::new_arced(t.into().map(Arc::new))
            }

            #[inline]
            pub fn new_arced(t: impl Into<Option<Arc<T>>>) -> Self {
                match t.into() {
                    Some(t) => Self {
                        inner: AtomicPtr::new(Arc::into_raw(t).cast_mut()),
                    },
                    None => Self {
                        inner: AtomicPtr::new(core::ptr::null_mut()),
                    },
                }
            }

            #[inline]
            pub fn replace(&self, new: impl Into<Option<T>>) -> Option<Arc<T>> {
                self.replace_arced(new.into().map(Arc::new))
            }

            #[inline]
            pub fn take(&self) -> Option<Arc<T>> {
                self.replace_arced(None)
            }

            #[inline]
            pub fn replace_arced(&self, new: impl Into<Option<Arc<T>>>) -> Option<Arc<T>> {
                let new = match new.into() {
                    Some(new) => Arc::into_raw(new).cast_mut(),
                    None => core::ptr::null_mut(),
                };

                let prev = self.inner.swap(new, Ordering::AcqRel);
                if prev.is_null() {
                    return None;
                }
                return unsafe { Some(Arc::from_raw(prev)) };
            }
        }

        impl<T> ArcCell<T> {
            #[inline]
            pub fn is_some(&self) -> bool {
                return !self.is_none();
            }

            #[inline]
            pub fn is_none(&self) -> bool {
                return self.inner.load(Ordering::Relaxed).is_null();
            }
        }

        impl<T> Drop for ArcCell<T> {
            fn drop(&mut self) {
                unsafe {
                    let ptr = *self.inner.get_mut();
                    if !ptr.is_null() {
                        let _ = Arc::from_raw(ptr);
                    }
                }
            }
        }

    }
}

unsafe impl<T> Send for ArcCell<T> where Arc<T>: Send {}
unsafe impl<T> Sync for ArcCell<T> where Arc<T>: Sync {}

// Thanks ChatGPT!
#[cfg(test)]
mod tests {
    use alloc::sync::Arc;

    use super::ArcCell;

    #[test]
    fn create_and_take() {
        let cell = ArcCell::<i32>::new(Some(42));
        assert_eq!(cell.take(), Some(Arc::new(42)));
        assert!(cell.is_none());
    }

    #[test]
    fn create_empty_and_take() {
        let cell = ArcCell::<i32>::new(None);
        assert!(cell.is_none());
        assert_eq!(cell.take(), None);
    }

    #[test]
    fn replace() {
        let cell = ArcCell::<i32>::new(Some(42));
        let old_value = cell.replace(Some(13));
        assert_eq!(old_value, Some(Arc::new(42)));
        assert_eq!(cell.take(), Some(Arc::new(13)));
    }

    #[test]
    fn replace_with_none() {
        let cell = ArcCell::<i32>::new(Some(42));
        let old_value = cell.replace(None);
        assert_eq!(old_value, Some(Arc::new(42)));
        assert!(cell.is_none());
    }

    #[test]
    fn is_some_and_is_none() {
        let cell = ArcCell::<i32>::new(Some(42));
        assert!(cell.is_some());
        assert!(!cell.is_none());
        cell.take();
        assert!(!cell.is_some());
        assert!(cell.is_none());
    }

    // #[cfg(all(feature = "std", miri))]
    mod miri {
        // Add other imports from previous tests
        use crate::arc_cell::ArcCell;
        use std::sync::Arc;
        use std::thread;

        const NUM_THREADS: usize = 10;
        const NUM_ITERATIONS: usize = 1000;

        fn stress_test_body(cell: &ArcCell<Option<i32>>) {
            for _ in 0..NUM_ITERATIONS {
                cell.replace(Some(42));
                cell.take();
            }
        }

        #[test]
        fn miri_stress_test() {
            let cell = Arc::new(ArcCell::new(Some(0)));
            let mut handles = Vec::with_capacity(NUM_THREADS);

            for _ in 0..NUM_THREADS {
                let cloned_cell = Arc::clone(&cell);
                let handle = thread::spawn(move || {
                    stress_test_body(&cloned_cell);
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap();
            }

            assert!(cell.is_none());
        }
    }
}
