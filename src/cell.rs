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
