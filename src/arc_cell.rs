#[cfg(feature = "alloc_api")]
use alloc::alloc::*;
#[cfg(feature = "alloc_api")]
use core::mem::ManuallyDrop;

use alloc::boxed::Box;
use core::sync::atomic::{AtomicPtr, Ordering};
use docfg::docfg;

#[derive(Debug)]
pub struct AtomicCell<T, #[cfg(feature = "alloc_api")] A: Allocator = Global> {
    inner: AtomicPtr<T>,
    #[cfg(feature = "alloc_api")]
    alloc: ManuallyDrop<A>,
}

#[docfg(feature = "alloc_api")]
impl<T, A: Allocator> AtomicCell<T, A> {
    #[inline]
    pub fn new_in(t: impl Into<Option<T>>, alloc: A) -> Self {
        Self::new_boxed_in(match t.into() {
            Some(x) => Ok(Box::new_in(x, alloc)),
            None => Err(alloc),
        })
    }

    #[inline]
    pub fn new_boxed_in(t: Result<Box<T, A>, A>) -> Self {
        match t {
            Ok(t) => {
                let (ptr, alloc) = Box::into_raw_with_allocator(t);
                Self {
                    inner: AtomicPtr::new(ptr),
                    alloc: ManuallyDrop::new(alloc),
                }
            }
            Err(alloc) => Self {
                inner: AtomicPtr::new(core::ptr::null_mut()),
                alloc: ManuallyDrop::new(alloc),
            },
        }
    }

    #[inline]
    pub fn allocator(&self) -> &A {
        core::ops::Deref::deref(&self.alloc)
    }

    #[inline]
    pub fn take_in(&self) -> Option<Box<T, &A>> {
        self.replace_in(None)
    }

    #[inline]
    pub fn replace_in(&self, new: impl Into<Option<T>>) -> Option<Box<T, &A>> {
        let new = match new.into() {
            Some(new) => Box::into_raw(Box::new_in(new, core::ops::Deref::deref(&self.alloc))),
            None => core::ptr::null_mut(),
        };

        let prev = self.inner.swap(new, Ordering::AcqRel);
        if prev.is_null() {
            return None;
        }

        return unsafe { Some(Box::from_raw_in(prev, core::ops::Deref::deref(&self.alloc))) };
    }
}

impl<T> AtomicCell<T> {
    #[inline]
    pub fn new(t: impl Into<Option<T>>) -> Self {
        Self::new_boxed(t.into().map(Box::new))
    }

    #[inline]
    pub fn new_boxed(t: impl Into<Option<Box<T>>>) -> Self {
        match t.into() {
            Some(t) => Self {
                inner: AtomicPtr::new(Box::into_raw(t)),
                #[cfg(feature = "alloc_api")]
                alloc: ManuallyDrop::new(Global),
            },
            None => Self {
                inner: AtomicPtr::new(core::ptr::null_mut()),
                #[cfg(feature = "alloc_api")]
                alloc: ManuallyDrop::new(Global),
            },
        }
    }

    #[inline]
    pub fn replace(&self, new: impl Into<Option<T>>) -> Option<T> {
        self.replace_boxed(new.into().map(Box::new)).map(|x| *x)
    }

    #[inline]
    pub fn take(&self) -> Option<T> {
        self.take_boxed().map(|x| *x)
    }

    #[inline]
    pub fn replace_boxed(&self, new: impl Into<Option<Box<T>>>) -> Option<Box<T>> {
        let new = match new.into() {
            Some(new) => Box::into_raw(new),
            None => core::ptr::null_mut(),
        };

        let prev = self.inner.swap(new, Ordering::AcqRel);
        if prev.is_null() {
            return None;
        }
        return unsafe { Some(Box::from_raw(prev)) };
    }

    #[inline]
    pub fn take_boxed(&self) -> Option<Box<T>> {
        self.replace_boxed(None)
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "alloc_api")] {
        impl<T, A: Allocator> AtomicCell<T, A> {
            #[inline]
            pub fn get_mut (&mut self) -> Option<&mut T> {
                let ptr = *self.inner.get_mut();
                if ptr.is_null() { return None }
                return unsafe { Some(&mut *ptr) }
            }

            #[inline]
            pub fn is_some (&self) -> bool {
                return !self.is_none()
            }

            #[inline]
            pub fn is_none (&self) -> bool {
                return self.inner.load(Ordering::Relaxed).is_null()
            }
        }

        impl<T, A: Allocator> Drop for AtomicCell<T, A> {
            fn drop(&mut self) {
                unsafe {
                    let ptr = *self.inner.get_mut();
                    if ptr.is_null() {
                        ManuallyDrop::drop(&mut self.alloc);
                    } else {
                        let _ = Box::from_raw_in(ptr, ManuallyDrop::take(&mut self.alloc));
                    }
                }
            }
        }
    } else {
        impl<T> AtomicCell<T> {
            #[inline]
            pub fn get_mut (&mut self) -> Option<&mut T> {
                let ptr = *self.inner.get_mut();
                if ptr.is_null() { return None }
                return unsafe { Some(&mut *ptr) }
            }

            #[inline]
            pub fn is_some (&self) -> bool {
                return !self.is_none()
            }

            #[inline]
            pub fn is_none (&self) -> bool {
                return self.inner.load(Ordering::Relaxed).is_null()
            }
        }

        impl<T> Drop for AtomicCell<T> {
            fn drop(&mut self) {
                unsafe {
                    let ptr = *self.inner.get_mut();
                    if !ptr.is_null() {
                        let _ = Box::from_raw(ptr);
                    }
                }
            }
        }
    }
}

// Thanks ChatGPT!
#[cfg(test)]
mod tests {
    use super::AtomicCell;

    #[test]
    fn create_and_take() {
        let cell = AtomicCell::<i32>::new(Some(42));
        assert_eq!(cell.take(), Some(42));
        assert!(cell.is_none());
    }

    #[test]
    fn create_empty_and_take() {
        let cell = AtomicCell::<i32>::new(None);
        assert!(cell.is_none());
        assert_eq!(cell.take(), None);
    }

    #[test]
    fn replace() {
        let cell = AtomicCell::<i32>::new(Some(42));
        let old_value = cell.replace(Some(13));
        assert_eq!(old_value, Some(42));
        assert_eq!(cell.take(), Some(13));
    }

    #[test]
    fn replace_with_none() {
        let cell = AtomicCell::<i32>::new(Some(42));
        let old_value = cell.replace(None);
        assert_eq!(old_value, Some(42));
        assert!(cell.is_none());
    }

    #[test]
    fn is_some_and_is_none() {
        let cell = AtomicCell::<i32>::new(Some(42));
        assert!(cell.is_some());
        assert!(!cell.is_none());
        cell.take();
        assert!(!cell.is_some());
        assert!(cell.is_none());
    }

    // Tests for custom allocator functionality
    #[cfg(feature = "alloc_api")]
    mod custom_allocator {
        use super::*;
        use alloc::alloc::Global;
        use alloc::alloc::{Allocator, Layout};
        use core::{alloc::AllocError, ptr::NonNull};

        #[derive(Debug, Clone, Copy)]
        pub struct DummyAllocator;

        unsafe impl Allocator for DummyAllocator {
            fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
                Global.allocate(layout)
            }

            unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
                Global.deallocate(ptr, layout)
            }
        }

        #[test]
        fn create_and_take_with_allocator() {
            let cell = AtomicCell::<i32, DummyAllocator>::new_in(Some(42), DummyAllocator);
            assert_eq!(cell.take_in().map(|x| *x), Some(42));
            assert!(cell.is_none());
        }

        #[test]
        fn create_empty_and_take_with_allocator() {
            let cell = AtomicCell::<i32, DummyAllocator>::new_in(None, DummyAllocator);
            assert!(cell.is_none());
            assert_eq!(cell.take_in(), None);
        }

        #[test]
        fn replace_with_allocator() {
            let cell = AtomicCell::<i32, DummyAllocator>::new_in(Some(42), DummyAllocator);
            let old_value = cell.replace_in(Some(13));
            assert_eq!(old_value.map(|x| *x), Some(42));
            assert_eq!(cell.take_in().map(|x| *x), Some(13));
        }

        #[test]
        fn replace_with_none_with_allocator() {
            let cell = AtomicCell::<i32, DummyAllocator>::new_in(Some(42), DummyAllocator);
            let old_value = cell.replace_in(None);
            assert_eq!(old_value, Some(Box::new_in(42, cell.allocator())));
            assert!(cell.is_none());
        }
    }

    #[cfg(miri)]
    mod miri {
        // Add other imports from previous tests
        use crate::cell::AtomicCell;
        use std::sync::Arc;
        use std::thread;

        const NUM_THREADS: usize = 10;
        const NUM_ITERATIONS: usize = 1000;

        fn stress_test_body(cell: &AtomicCell<Option<i32>>) {
            for _ in 0..NUM_ITERATIONS {
                cell.replace(Some(42));
                cell.take();
            }
        }

        #[test]
        fn miri_stress_test() {
            let cell = Arc::new(AtomicCell::new(Some(0)));
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
