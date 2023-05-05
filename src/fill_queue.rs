use crate::{AllocError, InnerAtomicFlag, FALSE, TRUE};
use core::fmt::Debug;
use core::{
    alloc::Layout,
    iter::FusedIterator,
    ptr::NonNull,
    sync::atomic::{AtomicPtr, Ordering},
};
#[cfg(feature = "alloc_api")]
use {alloc::alloc::Global, core::alloc::*};

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

struct PrevCell<T> {
    init: InnerAtomicFlag,
    prev: AtomicPtr<FillQueueNode<T>>,
}

impl<T> PrevCell<T> {
    #[inline]
    pub const fn new() -> Self {
        return Self {
            init: InnerAtomicFlag::new(FALSE),
            prev: AtomicPtr::new(core::ptr::null_mut()),
        };
    }

    #[inline]
    pub fn set(&self, prev: *mut FillQueueNode<T>) {
        cfg_if::cfg_if! {
            if #[cfg(debug_assertions)] {
                assert!(self.prev.swap(prev, Ordering::AcqRel).is_null());
                self.init.store(TRUE, Ordering::Release);
            } else {
                self.prev.store(prev, Ordering::Release);
                self.init.store(TRUE, Ordering::Release);
            }
        }
    }

    #[inline]
    pub fn set_mut(&mut self, prev: *mut FillQueueNode<T>) {
        let this_prev = self.prev.get_mut();
        debug_assert!(this_prev.is_null());

        *this_prev = prev;
        *self.init.get_mut() = TRUE;
    }

    pub fn get(&self) -> *mut FillQueueNode<T> {
        while self.init.load(Ordering::Acquire) == FALSE {
            core::hint::spin_loop()
        }
        return self.prev.swap(core::ptr::null_mut(), Ordering::Acquire);
    }
}

struct FillQueueNode<T> {
    prev: PrevCell<T>,
    v: T,
}

/// An atomic queue intended for use cases where taking the full contents of the queue is needed.
///
/// The queue is, basically, an atomic singly-linked list, where nodes are first allocated and then the list's tail
/// is atomically updated.
///
/// When the queue is "chopped", the list's tail is swaped to null, and it's previous tail is used as the base of the [`ChopIter`]
///
/// # Performance
/// The performance of pushing elements is expected to be similar to pushing elements to a [`SegQueue`](crossbeam::queue::SegQueue) or `Mutex<Vec<_>>`,
/// but "chopping" elements is expected to be arround 2 times faster than with a `Mutex<Vec<_>>`, and 3 times faster than a [`SegQueue`](crossbeam::queue::SegQueue)
///
/// > You can see the benchmark results [here](https://docs.google.com/spreadsheets/d/1wcyD3TlCQMCPFHOfeko5ytn-R7aM8T7lyKVir6vf_Wo/edit?usp=sharing)
///
/// # Use `FillQueue` when:
/// - You want a queue that's updateable by shared reference
/// - You want to retreive all elements of the queue at once
/// - There is no specifically desired order for the elements to be retreived on, or that order is LIFO (Last In First Out)
///
/// # Don't use `FillQueue` when:
/// - You don't need a queue updateable by shared reference
/// - You want to retreive the elements of the queue one by one (see [`SegQueue`](crossbeam::queue::SegQueue))
/// - You require the elements in a specific order that isn't LIFO
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub struct FillQueue<T, #[cfg(feature = "alloc_api")] A: Allocator = Global> {
    head: AtomicPtr<FillQueueNode<T>>,
    #[cfg(feature = "alloc_api")]
    alloc: A,
}

impl<T> FillQueue<T> {
    /// Creates a new [`FillQueue`] with the global allocator.
    /// # Example
    /// ```rust
    /// use utils_atomics::prelude::*;
    ///
    /// let queue = FillQueue::<i32>::new();
    /// ```
    #[inline]
    pub const fn new() -> Self {
        Self {
            head: AtomicPtr::new(core::ptr::null_mut()),
            #[cfg(feature = "alloc_api")]
            alloc: Global,
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
    #[inline]
    pub const fn new_in(alloc: A) -> Self {
        Self {
            head: AtomicPtr::new(core::ptr::null_mut()),
            alloc,
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
    #[inline]
    pub fn allocator(&self) -> &A {
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
        #[inline]
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
        #[inline]
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
        #[inline]
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
                prev: PrevCell::new(),
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
                let rf = &*ptr.as_ptr();
                rf.prev.set(prev);
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
                prev: PrevCell::new(),
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
                ptr.as_mut().prev.set_mut(prev);
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
    #[inline]
    pub fn chop(&self) -> ChopIter<T, A>
    where
        A: Clone,
    {
        let ptr = self.head.swap(core::ptr::null_mut(), Ordering::AcqRel);
        ChopIter {
            ptr: NonNull::new(ptr),
            alloc: self.alloc.clone(),
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
    #[inline]
    pub fn chop_mut(&mut self) -> ChopIter<T, A>
    where
        A: Clone,
    {
        let ptr = unsafe { core::ptr::replace(self.head.get_mut(), core::ptr::null_mut()) };

        ChopIter {
            ptr: NonNull::new(ptr),
            alloc: self.alloc.clone(),
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
    #[inline]
    pub fn chop(&self) -> ChopIter<T> {
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
    #[inline]
    pub fn chop_mut(&mut self) -> ChopIter<T> {
        let ptr = unsafe { core::ptr::replace(self.head.get_mut(), core::ptr::null_mut()) };

        ChopIter {
            ptr: NonNull::new(ptr),
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
    alloc: A,
}

impl_all! {
    impl @Iterator => ChopIter {
        type Item = T;

        #[inline]
        fn next(&mut self) -> Option<Self::Item> {
            if let Some(ptr) = self.ptr {
                unsafe {
                    let node = &*ptr.as_ptr();
                    let value = core::ptr::read(&node.v);
                    self.ptr = NonNull::new(node.prev.get());

                    #[cfg(feature = "alloc_api")]
                    self.alloc.deallocate(ptr.cast(), Layout::new::<FillQueueNode<T>>());
                    #[cfg(not(feature = "alloc_api"))]
                    alloc::alloc::dealloc(ptr.as_ptr().cast(), Layout::new::<FillQueueNode<T>>());

                    return Some(value)
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
            self.for_each(core::mem::drop)
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
        f.debug_struct("FillQueue")
            .field("alloc", &self.alloc)
            .finish_non_exhaustive()
    }
}
#[cfg(not(feature = "alloc_api"))]
impl<T> Debug for FillQueue<T> {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        f.debug_struct("FillQueue").finish_non_exhaustive()
    }
}

// Thanks ChatGPT!
#[cfg(test)]
mod tests {
    use super::FillQueue;

    #[test]
    fn test_basic_functionality() {
        let mut fill_queue = FillQueue::new();
        assert!(fill_queue.is_empty());

        fill_queue.push(1);
        fill_queue.push(2);
        fill_queue.push(3);

        assert!(!fill_queue.is_empty());

        let mut chop_iter = fill_queue.chop_mut();
        assert_eq!(chop_iter.next(), Some(3));
        assert_eq!(chop_iter.next(), Some(2));
        assert_eq!(chop_iter.next(), Some(1));
        assert_eq!(chop_iter.next(), None);

        fill_queue.push_mut(1);
        fill_queue.push_mut(2);
        fill_queue.push_mut(3);

        let mut chop_iter = fill_queue.chop();
        assert_eq!(chop_iter.next(), Some(3));
        assert_eq!(chop_iter.next(), Some(2));
        assert_eq!(chop_iter.next(), Some(1));
        assert_eq!(chop_iter.next(), None);

        assert!(fill_queue.is_empty());
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_concurrent_fill_queue() {
        use core::sync::atomic::{AtomicUsize, Ordering};

        let fill_queue = FillQueue::new();
        let mut count = AtomicUsize::new(0);

        std::thread::scope(|s| {
            for _ in 0..10 {
                s.spawn(|| {
                    for i in 1..=10 {
                        fill_queue.push(i);
                    }

                    count.fetch_add(fill_queue.chop().count(), Ordering::Relaxed);
                });
            }
        });

        assert_eq!(*count.get_mut(), 100);
    }
}
