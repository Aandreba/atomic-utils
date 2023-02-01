use core::{sync::atomic::{AtomicPtr, Ordering}, ptr::NonNull, iter::FusedIterator, alloc::Layout};

use crate::{InnerAtomicFlag, FALSE, TRUE, AllocError};
use core::fmt::Debug;
#[cfg(feature = "alloc_api")]
use {alloc::{alloc::Global}, core::alloc::*};

macro_rules! impl_all {
    (impl $(@$tr:path =>)? $target:ident {
        $($t:tt)*
    }) => {
        cfg_if::cfg_if! {
            if #[cfg(feature = "alloc_api")] {
                impl<T, A: Allocator> $($tr for)? $target <T, A> {
                    $($t)*
                }
            } else {
                impl<T> $($tr for)? $target <T> {
                    $($t)*
                }
            }
        }
    };
}

struct FillQueueNode<T> {
    init: InnerAtomicFlag,
    prev: *mut Self,
    v: T
}

/// An atomic queue intended for use cases where taking the full contents of the queue is needed.
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub struct FillQueue<T, #[cfg(feature = "alloc_api")] A: Allocator = Global> {
    head: AtomicPtr<FillQueueNode<T>>,
    #[cfg(feature = "alloc_api")]
    alloc: A
}

impl<T> FillQueue<T> {
    /// Creates a new [`FillQueue`] with the global allocator.
    /// # Example
    /// ```rust
    /// use utils_atomics::prelude::*;
    /// 
    /// let queue = FillQueue::<i32>::new();
    /// ```
    #[inline(always)]
    pub const fn new () -> Self {
        Self {
            head: AtomicPtr::new(core::ptr::null_mut()),
            #[cfg(feature = "alloc_api")]
            alloc: Global
        }
    }
}

#[docfg::docfg(feature = "alloc_api")]
impl<T, A: Allocator> FillQueue<T, A> {
    /// Creates a new [`FillQueue`] with the given allocator.
    /// # Example
    /// ```rust
    /// #![feature(allocator_api)]
    /// 
    /// use utils_atomics::prelude::*;
    /// use std::alloc::Global;
    /// 
    /// let queue = FillQueue::<i32>::new_in(Global);
    /// ```
    #[inline(always)]
    pub const fn new_in (alloc: A) -> Self {
        Self {
            head: AtomicPtr::new(core::ptr::null_mut()),
            alloc
        }
    }

    /// Returns a reference to this queue's allocator.
    /// # Example
    /// ```rust
    /// #![feature(allocator_api)]
    /// 
    /// use utils_atomics::prelude::*;
    /// use std::alloc::Global;
    /// 
    /// let queue = FillQueue::<i32>::new();
    /// let alloc : &Global = queue.allocator();
    /// ```
    #[inline(always)]
    pub fn allocator (&self) -> &A {
        &self.alloc
    }
}

impl_all! {
    impl FillQueue {
        /// Returns `true` if the que is currently empty, `false` otherwise.
        /// # Safety
        /// Whilst this method is not unsafe, it's result should be considered immediately stale.
        /// # Example
        /// ```rust
        /// use utils_atomics::prelude::*;
        /// 
        /// let queue = FillQueue::<i32>::new();
        /// assert!(queue.is_empty());
        /// ```
        #[inline(always)]
        pub fn is_empty (&self) -> bool {
            self.head.load(Ordering::Relaxed).is_null()
        }

        /// Uses atomic operations to push an element to the queue.
        /// # Panics
        /// This method panics if `alloc` fails to allocate the memory needed for the node.
        /// # Example
        /// ```rust
        /// use utils_atomics::prelude::*;
        /// 
        /// let queue = FillQueue::<i32>::new();
        /// queue.push(1);
        /// assert_eq!(queue.chop().next(), Some(1));
        /// ```
        #[inline(always)]
        pub fn push (&self, v: T) {
            self.try_push(v).unwrap()
        }

        /// Uses non-atomic operations to push an element to the queue.
        /// # Panics
        /// This method panics if `alloc` fails to allocate the memory needed for the node.
        /// # Example
        /// ```rust
        /// use utils_atomics::prelude::*;
        /// 
        /// let mut queue = FillQueue::<i32>::new();
        /// queue.push_mut(1);
        /// assert_eq!(queue.chop_mut().next(), Some(1));
        /// ```
        #[inline(always)]
        pub fn push_mut (&mut self, v: T) {
            self.try_push_mut(v).unwrap()
        }

        /// Uses atomic operations to push an element to the queue.
        /// 
        /// # Errors
        /// 
        /// This method returns an error if `alloc` fails to allocate the memory needed for the node.
        /// 
        /// # Example
        /// ```rust
        /// use utils_atomics::prelude::*;
        /// 
        /// let queue = FillQueue::<i32>::new();
        /// assert!(queue.try_push(1).is_ok());
        /// assert_eq!(queue.chop().next(), Some(1));
        /// ```
        pub fn try_push (&self, v: T) -> Result<(), AllocError> {
            let node = FillQueueNode {
                init: InnerAtomicFlag::new(FALSE),
                prev: core::ptr::null_mut(),
                v
            };

            let layout = Layout::new::<FillQueueNode<T>>();
            #[cfg(feature = "alloc_api")]
            let ptr = self.alloc.allocate(layout)?.cast::<FillQueueNode<T>>();
            #[cfg(not(feature = "alloc_api"))]
            let ptr = match unsafe { NonNull::new(alloc::alloc::alloc(layout)) } {
                Some(x) => x.cast::<FillQueueNode<T>>(),
                None => return Err(AllocError)
            };

            unsafe {
                ptr.as_ptr().write(node)
            }

            let prev = self.head.swap(ptr.as_ptr(), Ordering::AcqRel);
            unsafe {
                let rf = &mut *ptr.as_ptr();
                rf.prev = prev;
                rf.init.store(TRUE, Ordering::Release);
            }

            Ok(())
        }

        /// Uses non-atomic operations to push an element to the queue.
        /// 
        /// # Safety
        /// 
        /// This method is safe because the mutable reference guarantees we are the only thread that can access this queue.
        /// 
        /// # Errors
        /// 
        /// This method returns an error if `alloc` fails to allocate the memory needed for the node.
        /// 
        /// # Example
        /// 
        /// ```rust
        /// use utils_atomics::prelude::*;
        /// 
        /// let mut queue = FillQueue::<i32>::new();
        /// assert!(queue.try_push_mut(1).is_ok());
        /// assert_eq!(queue.chop_mut().next(), Some(1));
        /// ```
        pub fn try_push_mut (&mut self, v: T) -> Result<(), AllocError> {
            let node = FillQueueNode {
                init: InnerAtomicFlag::new(TRUE),
                prev: core::ptr::null_mut(),
                v
            };

            let layout = Layout::new::<FillQueueNode<T>>();
            #[cfg(feature = "alloc_api")]
            let mut ptr = self.alloc.allocate(layout)?.cast::<FillQueueNode<T>>();
            #[cfg(not(feature = "alloc_api"))]
            let mut ptr = match unsafe { NonNull::new(alloc::alloc::alloc(layout)) } {
                Some(x) => x.cast::<FillQueueNode<T>>(),
                None => return Err(AllocError)
            };
            
            unsafe {
                ptr.as_ptr().write(node);
                let prev = core::ptr::replace(self.head.get_mut(), ptr.as_ptr());
                ptr.as_mut().prev = prev;
                Ok(())
            }
        }
    }
}

#[cfg(feature = "alloc_api")]
impl<T, A: Allocator> FillQueue<T, A> {
    /// Returns a LIFO (Last In First Out) iterator over a chopped chunk of a [`FillQueue`].
    /// The elements that find themselves inside the chopped region of the queue will be accessed through non-atomic operations.
    /// # Example
    /// ```rust
    /// use utils_atomics::prelude::*;
    /// 
    /// let queue = FillQueue::<i32>::new();
    /// 
    /// queue.push(1);
    /// queue.push(2);
    /// queue.push(3);
    /// 
    /// let mut iter = queue.chop();
    /// assert_eq!(iter.next(), Some(3));
    /// assert_eq!(iter.next(), Some(2));
    /// assert_eq!(iter.next(), Some(1));
    /// assert_eq!(iter.next(), None)
    /// ```
    #[inline(always)]
    pub fn chop (&self) -> ChopIter<T, A> where A: Clone {
        let ptr = self.head.swap(core::ptr::null_mut(), Ordering::AcqRel);
        ChopIter { 
            ptr: NonNull::new(ptr),
            alloc: self.alloc.clone()
        }
    }

    /// Returns a LIFO (Last In First Out) iterator over a chopped chunk of a [`FillQueue`]. The chopping is done with non-atomic operations.
    /// # Safety
    /// This method is safe because the mutable reference guarantees we are the only thread that can access this queue.
    /// # Example
    /// ```rust
    /// use utils_atomics::prelude::*;
    /// 
    /// let mut queue = FillQueue::<i32>::new();
    /// 
    /// queue.push_mut(1);
    /// queue.push_mut(2);
    /// queue.push_mut(3);
    /// 
    /// let mut iter = queue.chop_mut();
    /// assert_eq!(iter.next(), Some(3));
    /// assert_eq!(iter.next(), Some(2));
    /// assert_eq!(iter.next(), Some(1));
    /// assert_eq!(iter.next(), None)
    /// ```
    #[inline(always)]
    pub fn chop_mut (&mut self) -> ChopIter<T, A> where A: Clone {
        let ptr = unsafe {
            core::ptr::replace(self.head.get_mut(), core::ptr::null_mut())
        };

        ChopIter { 
            ptr: NonNull::new(ptr),
            alloc: self.alloc.clone()
        }
    }
}

#[cfg(not(feature = "alloc_api"))]
impl<T> FillQueue<T> {
    /// Returns a LIFO (Last In First Out) iterator over a chopped chunk of a [`FillQueue`].
    /// The elements that find themselves inside the chopped region of the queue will be accessed through non-atomic operations.
    /// # Example
    /// ```rust
    /// use utils_atomics::prelude::*;
    /// 
    /// let queue = FillQueue::<i32>::new();
    /// 
    /// queue.push(1);
    /// queue.push(2);
    /// queue.push(3);
    /// 
    /// let mut iter = queue.chop();
    /// assert_eq!(iter.next(), Some(3));
    /// assert_eq!(iter.next(), Some(2));
    /// assert_eq!(iter.next(), Some(1));
    /// assert_eq!(iter.next(), None)
    /// ```
    #[inline(always)]
    pub fn chop (&self) -> ChopIter<T> {
        let ptr = self.head.swap(core::ptr::null_mut(), Ordering::AcqRel);
        ChopIter { 
            ptr: NonNull::new(ptr),
        }
    }

    /// Returns a LIFO (Last In First Out) iterator over a chopped chunk of a [`FillQueue`]. The chopping is done with non-atomic operations.
    /// # Safety
    /// This method is safe because the mutable reference guarantees we are the only thread that can access this queue.
    /// # Example
    /// ```rust
    /// use utils_atomics::prelude::*;
    /// 
    /// let mut queue = FillQueue::<i32>::new();
    /// 
    /// queue.push_mut(1);
    /// queue.push_mut(2);
    /// queue.push_mut(3);
    /// 
    /// let mut iter = queue.chop_mut();
    /// assert_eq!(iter.next(), Some(3));
    /// assert_eq!(iter.next(), Some(2));
    /// assert_eq!(iter.next(), Some(1));
    /// assert_eq!(iter.next(), None)
    /// ```
    #[inline(always)]
    pub fn chop_mut (&mut self) -> ChopIter<T> {
        let ptr = unsafe {
            core::ptr::replace(self.head.get_mut(), core::ptr::null_mut())
        };

        ChopIter { 
            ptr: NonNull::new(ptr)
        }
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "alloc_api")] {
        unsafe impl<T: Send, A: Send + Allocator> Send for FillQueue<T, A> {}
        unsafe impl<T: Sync, A: Sync + Allocator> Sync for FillQueue<T, A> {}
        unsafe impl<T: Send, A: Send + Allocator> Send for ChopIter<T, A> {}
        unsafe impl<T: Sync, A: Sync + Allocator> Sync for ChopIter<T, A> {}
    } else {
        unsafe impl<T: Send> Send for FillQueue<T> {}
        unsafe impl<T: Sync> Sync for FillQueue<T> {}
        unsafe impl<T: Send> Send for ChopIter<T> {}
        unsafe impl<T: Sync> Sync for ChopIter<T> {}
    }
}

/// Iterator of [`FillQueue::chop`] and [`FillQueue::chop_mut`]
pub struct ChopIter<T, #[cfg(feature = "alloc_api")] A: Allocator = Global> {
    ptr: Option<NonNull<FillQueueNode<T>>>,
    #[cfg(feature = "alloc_api")]
    alloc: A
}

impl_all! {
    impl @Iterator => ChopIter {
        type Item = T;

        #[inline]
        fn next(&mut self) -> Option<Self::Item> {
            if let Some(ptr) = self.ptr {
                unsafe {
                    let node = core::ptr::read(ptr.as_ptr());

                    #[cfg(feature = "alloc_api")]
                    self.alloc.deallocate(ptr.cast(), Layout::new::<FillQueueNode<T>>());
                    #[cfg(not(feature = "alloc_api"))]
                    alloc::alloc::dealloc(ptr.as_ptr().cast(), Layout::new::<FillQueueNode<T>>());

                    while node.init.load(Ordering::Acquire) == FALSE { core::hint::spin_loop() }
                    self.ptr = NonNull::new(node.prev);
                    return Some(node.v)
                }
            }

            None
        }
    }
}

impl_all! {
    impl @Drop => ChopIter {
        #[inline]
        fn drop(&mut self) {
            while let Some(ptr) = self.ptr {
                unsafe {
                    let node = core::ptr::read(ptr.as_ptr());
                    
                    #[cfg(feature = "alloc_api")]
                    self.alloc.deallocate(ptr.cast(), Layout::new::<FillQueueNode<T>>());
                    #[cfg(not(feature = "alloc_api"))]
                    alloc::alloc::dealloc(ptr.as_ptr().cast(), Layout::new::<FillQueueNode<T>>());
                    
                    while node.init.load(Ordering::Acquire) == FALSE { core::hint::spin_loop() }
                    self.ptr = NonNull::new(node.prev);
                }
            }
        }
    }
}

impl_all! {
    impl @FusedIterator => ChopIter {}
}

#[cfg(feature = "alloc_api")]
impl<T, A: Debug + Allocator> Debug for FillQueue<T, A> {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.debug_struct("FillQueue").field("alloc", &self.alloc).finish_non_exhaustive()
    }
}
#[cfg(not(feature = "alloc_api"))]
impl<T> Debug for FillQueue<T> {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        f.debug_struct("FillQueue").finish_non_exhaustive()
    }
}