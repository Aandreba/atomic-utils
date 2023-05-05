#[cfg(feature = "alloc_api")]
use alloc::alloc::*;
#[cfg(feature = "alloc_api")]
use core::mem::ManuallyDrop;

use alloc::boxed::Box;
use core::sync::atomic::{AtomicPtr, Ordering};
use docfg::docfg;

/// An atomic cell that can be safely shared between threads and can contain an optional value.
///
/// `AtomicCell` provides methods to store, replace, and take values atomically, ensuring safe access
/// and modification across multiple threads.
///
/// # Example
///
/// ```rust
/// use utils_atomics::AtomicCell;
///
/// let mut atomic_cell = AtomicCell::new(Some(42));
///
/// std::thread::scope(|s| {
///     // Spawn a thread that replaces the value inside the AtomicCell
///     s.spawn(|| {
///         let prev_value = atomic_cell.replace(Some(24));
///         assert_eq!(prev_value, Some(42));
///     });
/// });
///
/// // Check that the value was replaced
/// assert_eq!(atomic_cell.get_mut().copied(), Some(24));
/// ```
#[derive(Debug)]
pub struct AtomicCell<T, #[cfg(feature = "alloc_api")] A: Allocator = Global> {
    inner: AtomicPtr<T>,
    #[cfg(feature = "alloc_api")]
    alloc: ManuallyDrop<A>,
}

#[docfg(feature = "alloc_api")]
impl<T, A: Allocator> AtomicCell<T, A> {
    /// Constructs a new `AtomicCell` containing an optional value t and an allocator alloc.
    ///
    /// If the value is `Some(x)`, it is boxed using the allocator.
    /// If the value is `None`, an empty `AtomicCell` is created with the allocator.
    ///
    /// # Example
    /// ```rust
    /// #![feature(allocator_api)]
    ///
    /// use utils_atomics::AtomicCell;
    /// use std::alloc::System;
    ///
    /// let atomic_cell = AtomicCell::<i32, _>::new_in(Some(42), System);
    /// ```
    #[inline]
    pub fn new_in(t: impl Into<Option<T>>, alloc: A) -> Self {
        Self::new_boxed_in(match t.into() {
            Some(x) => Ok(Box::new_in(x, alloc)),
            None => Err(alloc),
        })
    }

    /// Constructs a new `AtomicCell` from a boxed value or an allocator.
    ///
    /// If the input is `Ok(t)`, the `AtomicCell` contains the boxed value, and the allocator is extracted from the box.
    /// If the input is `Err(alloc)`, an empty `AtomicCell` is created with the allocator.
    ///
    /// # Example
    /// ```rust
    /// #![feature(allocator_api)]
    /// extern crate alloc;
    ///
    /// use utils_atomics::AtomicCell;
    /// use std::alloc::System;
    /// use alloc::boxed::Box;
    ///    
    /// let atomic_cell = AtomicCell::new_boxed_in(Ok(Box::new_in(42, System)));
    /// ```
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

    /// Returns a reference to the allocator associated with the `AtomicCell`.
    ///
    /// # Example
    /// ```rust
    /// #![feature(allocator_api)]
    ///
    /// use utils_atomics::AtomicCell;
    /// use std::alloc::System;
    ///
    /// let atomic_cell = AtomicCell::<i32, System>::new_in(Some(42), System);
    /// let allocator = atomic_cell.allocator();
    /// ```
    #[inline]
    pub fn allocator(&self) -> &A {
        core::ops::Deref::deref(&self.alloc)
    }

    /// Takes the value out of the `AtomicCell`, leaving it empty.
    ///
    /// Returns an optional boxed value with a reference to the allocator.
    /// If the `AtomicCell` is empty, returns `None`.
    ///
    /// # Example
    /// ```rust
    /// #![feature(allocator_api)]
    ///
    /// use utils_atomics::AtomicCell;
    /// use std::alloc::System;
    ///
    /// let atomic_cell = AtomicCell::new_in(Some(42), System);
    /// let taken_value = atomic_cell.take_in();
    /// assert_eq!(taken_value, Some(Box::new_in(42, &System)))
    /// ```
    #[inline]
    pub fn take_in(&self) -> Option<Box<T, &A>> {
        self.replace_in(None)
    }

    /// Replaces the value inside the `AtomicCell` with a new optional value.
    ///
    /// If the new value is `Some(new)`, it is boxed using the allocator.
    /// If the new value is `None`, the `AtomicCell` is emptied.
    ///
    /// Returns the old value as an optional boxed value with a reference to the allocator.
    /// If the `AtomicCell` was empty, returns None.
    ///
    /// # Example
    /// ```rust
    /// #![feature(allocator_api)]

    ///
    /// use utils_atomics::AtomicCell;
    /// use std::alloc::System;
    ///
    /// let atomic_cell = AtomicCell::new_in(Some(42), System);
    /// let old_value = atomic_cell.replace_in(Some(24));
    /// assert_eq!(old_value, Some(Box::new_in(42, atomic_cell.allocator())));
    /// assert_eq!(atomic_cell.take(), Some(24));
    /// ```
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
    /// Constructs a new `AtomicCell` containing an optional value `t`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use utils_atomics::AtomicCell;
    ///
    /// let atomic_cell = AtomicCell::<i32>::new(Some(42));
    /// ```
    #[inline]
    pub fn new(t: impl Into<Option<T>>) -> Self {
        Self::new_boxed(t.into().map(Box::new))
    }

    /// Constructs a new `AtomicCell` from an optional boxed value `t`.
    ///
    /// # Example
    ///
    /// ```rust
    /// extern crate alloc;
    ///
    /// use utils_atomics::AtomicCell;
    /// use alloc::boxed::Box;
    ///
    /// let atomic_cell = AtomicCell::new_boxed(Some(Box::new(42)));
    /// ```
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

    /// Replaces the value inside the `AtomicCell` with a new optional value `new`.
    /// Returns the old value as an optional value. If the `AtomicCell` was empty, returns `None`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use utils_atomics::AtomicCell;
    ///
    /// let atomic_cell = AtomicCell::<i32>::new(Some(42));
    /// let old_value = atomic_cell.replace(Some(24));
    /// ```
    #[inline]
    pub fn replace(&self, new: impl Into<Option<T>>) -> Option<T> {
        self.replace_boxed(new.into().map(Box::new)).map(|x| *x)
    }

    /// Replaces the value inside the `AtomicCell` with a new optional boxed value `new`.
    /// Returns the old value as an optional boxed value. If the `AtomicCell` was empty, returns `None`.
    ///
    /// # Example
    ///
    /// ```rust
    /// extern crate alloc;
    ///
    /// use utils_atomics::AtomicCell;
    /// use alloc::boxed::Box;
    ///
    /// let atomic_cell = AtomicCell::new_boxed(Some(Box::new(42)));
    /// let old_value = atomic_cell.replace_boxed(Some(Box::new(24)));
    /// ```
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

    /// Takes the value out of the `AtomicCell`, leaving it empty.
    /// Returns an optional boxed value. If the `AtomicCell` is empty, returns `None`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use utils_atomics::AtomicCell;
    ///
    /// let atomic_cell = AtomicCell::new_boxed(Some(Box::new(42)));
    /// let taken_value = atomic_cell.take_boxed();
    /// ```
    #[inline]
    pub fn take_boxed(&self) -> Option<Box<T>> {
        self.replace_boxed(None)
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "alloc_api")] {
        impl<T, A: Allocator> AtomicCell<T, A> {
            /// Takes the value out of the `AtomicCell`, leaving it empty.
            /// Returns an optional value. If the `AtomicCell` is empty, returns `None`.
            ///
            /// # Examples
            ///
            /// ```
            /// use utils_atomics::AtomicCell;
            ///
            /// let atomic_cell = AtomicCell::new(Some(42));
            /// assert_eq!(atomic_cell.take(), Some(42));
            /// assert_eq!(atomic_cell.take(), None);
            /// ```
            #[inline]
            pub fn take(&self) -> Option<T> {
                self.take_in().map(|x| *x)
            }

            /// Returns a mutable reference to the value inside the `AtomicCell`, if any.
            /// If the `AtomicCell` is empty, returns `None`.
            ///
            /// # Examples
            ///
            /// ```
            /// use utils_atomics::AtomicCell;
            ///
            /// let mut atomic_cell = AtomicCell::new(Some(42));
            /// let value_ref = atomic_cell.get_mut().unwrap();
            /// *value_ref = 24;
            /// assert_eq!(*value_ref, 24);
            /// ```
            #[inline]
            pub fn get_mut (&mut self) -> Option<&mut T> {
                let ptr = *self.inner.get_mut();
                if ptr.is_null() { return None }
                return unsafe { Some(&mut *ptr) }
            }

            /// Returns `true` if the `AtomicCell` contains a value.
            ///
            /// # Examples
            ///
            /// ```
            /// use utils_atomics::AtomicCell;
            ///
            /// let atomic_cell = AtomicCell::<i32>::new(Some(42));
            /// assert!(atomic_cell.is_some());
            /// ```
            #[inline]
            pub fn is_some (&self) -> bool {
                return !self.is_none()
            }

            /// Returns `true` if the `AtomicCell` is empty.
            ///
            /// # Examples
            ///
            /// ```
            /// use utils_atomics::AtomicCell;
            ///
            /// let atomic_cell = AtomicCell::<i32>::new(None);
            /// assert!(atomic_cell.is_none());
            /// ```
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

        unsafe impl<T: Send, A: Allocator + Send> Send for AtomicCell<T, A> {}
        unsafe impl<T: Sync, A: Allocator + Sync> Sync for AtomicCell<T, A> {}
    } else {
        impl<T> AtomicCell<T> {
            /// Takes the value out of the `AtomicCell`, leaving it empty.
            /// Returns an optional value. If the `AtomicCell` is empty, returns `None`.
            ///
            /// # Examples
            ///
            /// ```
            /// use utils_atomics::AtomicCell;
            ///
            /// let atomic_cell = AtomicCell::new(Some(42));
            /// assert_eq!(atomic_cell.take(), Some(42));
            /// assert_eq!(atomic_cell.take(), None);
            /// ```
            #[inline]
            pub fn take(&self) -> Option<T> {
                self.take_boxed().map(|x| *x)
            }

            /// Returns a mutable reference to the value inside the `AtomicCell`, if any.
            /// If the `AtomicCell` is empty, returns `None`.
            ///
            /// # Examples
            ///
            /// ```
            /// use utils_atomics::AtomicCell;
            ///
            /// let mut atomic_cell = AtomicCell::new(Some(42));
            /// let value_ref = atomic_cell.get_mut().unwrap();
            /// *value_ref = 24;
            /// assert_eq!(*value_ref, 24);
            /// ```
            #[inline]
            pub fn get_mut (&mut self) -> Option<&mut T> {
                let ptr = *self.inner.get_mut();
                if ptr.is_null() { return None }
                return unsafe { Some(&mut *ptr) }
            }

            /// Returns `true` if the `AtomicCell` contains a value.
            ///
            /// # Examples
            ///
            /// ```
            /// use utils_atomics::AtomicCell;
            ///
            /// let atomic_cell = AtomicCell::<i32>::new(Some(42));
            /// assert!(atomic_cell.is_some());
            /// ```
            #[inline]
            pub fn is_some (&self) -> bool {
                return !self.is_none()
            }

            /// Returns `true` if the `AtomicCell` is empty.
            ///
            /// # Examples
            ///
            /// ```
            /// use utils_atomics::AtomicCell;
            ///
            /// let atomic_cell = AtomicCell::<i32>::new(None);
            /// assert!(atomic_cell.is_none());
            /// ```
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
                        let _: Box<T> = Box::from_raw(ptr);
                    }
                }
            }
        }

        unsafe impl<T: Send> Send for AtomicCell<T> {}
        unsafe impl<T: Sync> Sync for AtomicCell<T> {}
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

    #[cfg(all(feature = "std", miri))]
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
