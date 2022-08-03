use core::{alloc::{Allocator, Layout, AllocError}, sync::atomic::{AtomicPtr, Ordering}, ptr::NonNull, iter::FusedIterator};
use alloc::{alloc::Global};

struct FillQueueNode<T> {
    prev: AtomicPtr<Self>,
    v: T
}

pub struct FillQueue<T, A: Allocator = Global> {
    head: AtomicPtr<FillQueueNode<T>>,
    alloc: A
}

impl<T> FillQueue<T> {
    /// Creates a new [`FillQueue`] with the global allocator.
    /// # Example
    /// ```rust
    /// use atomic_col::prelude::*;
    /// 
    /// let queue = FillQueue::<i32>::new();
    /// ```
    #[inline(always)]
    pub const fn new () -> Self {
        Self::new_in(Global)
    }
}

impl<T, A: Allocator> FillQueue<T, A> {
    /// Creates a new [`FillQueue`] with the given allocator.
    /// /// Creates a new [`FillQueue`] with the global allocator.
    /// # Example
    /// ```rust
    /// #![feature(allocator_api)]
    /// 
    /// use atomic_col::prelude::*;
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
    /// use atomic_col::prelude::*;
    /// use std::alloc::Global;
    /// 
    /// let queue = FillQueue::<i32>::new();
    /// let alloc : &Global = queue.allocator();
    /// ```
    #[inline(always)]
    pub fn allocator (&self) -> &A {
        &self.alloc
    }

    /// Returns `true` if the que is currently empty, `false` otherwise.
    /// # Safety
    /// Whilst this method is not unsafe, it's result should be considered immediately stale.
    /// # Example
    /// ```rust
    /// use atomic_col::prelude::*;
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
    /// use atomic_col::prelude::*;
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
    /// use atomic_col::prelude::*;
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
    /// # Errors
    /// This method returns an error if `alloc` fails to allocate the memory needed for the node.
    /// # Example
    /// ```rust
    /// use atomic_col::prelude::*;
    /// 
    /// let queue = FillQueue::<i32>::new();
    /// assert!(queue.try_push(1).is_ok());
    /// assert_eq!(queue.chop().next(), Some(1));
    /// ```
    pub fn try_push (&self, v: T) -> Result<(), AllocError> {
        let node = FillQueueNode {
            prev: AtomicPtr::default(),
            v
        };

        let ptr = self.alloc.allocate(Layout::new::<FillQueueNode<T>>())?.cast::<FillQueueNode<T>>();
        unsafe {
            ptr.as_ptr().write(node)
        }

        let prev = self.head.swap(ptr.as_ptr(), Ordering::AcqRel);
        unsafe {
            ptr.as_ref().prev.store(prev, Ordering::Release);
        }

        Ok(())
    }

    /// Uses non-atomic operations to push an element to the queue.
    /// # Safety
    /// This method is safe because the mutable reference guarantees we are the only thread that can access this queue.
    /// # Errors
    /// This method returns an error if `alloc` fails to allocate the memory needed for the node.
    /// # Example
    /// ```rust
    /// use atomic_col::prelude::*;
    /// 
    /// let mut queue = FillQueue::<i32>::new();
    /// assert!(queue.try_push_mut(1).is_ok());
    /// assert_eq!(queue.chop_mut().next(), Some(1));
    /// ```
    pub fn try_push_mut (&mut self, v: T) -> Result<(), AllocError> {
        let node = FillQueueNode {
            prev: AtomicPtr::default(),
            v
        };

        let mut ptr = self.alloc.allocate(Layout::new::<FillQueueNode<T>>())?.cast::<FillQueueNode<T>>();
        unsafe {
            ptr.as_ptr().write(node);
            let prev = core::ptr::replace(self.head.get_mut(), ptr.as_ptr());
            *ptr.as_mut().prev.get_mut() = prev;
            Ok(())
        }
    }
}

impl<T, A: Allocator> FillQueue<T, A> {
    /// Returns a LIFO (Last In First Out) iterator over a chopped chunk of a [`FillQueue`].
    /// The elements that find themselves inside the chopped region of the queue will be accessed through non-atomic operations.
    /// # Example
    /// ```rust
    /// use atomic_col::prelude::*;
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
    /// use atomic_col::prelude::*;
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

/// Iterator of [`FillQueue::chop`] and [`FillQueue::chop_mut`]
pub struct ChopIter<T, A: Allocator = Global> {
    ptr: Option<NonNull<FillQueueNode<T>>>,
    alloc: A
}

impl<T, A: Allocator> Iterator for ChopIter<T, A> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ptr) = self.ptr {
            unsafe {
                let mut node = core::ptr::read(ptr.as_ptr());
                self.alloc.deallocate(ptr.cast(), Layout::new::<FillQueueNode<T>>());
                self.ptr = NonNull::new(*node.prev.get_mut());
                return Some(node.v)
            }
        }

        None
    }
}

impl<T, A: Allocator> Drop for ChopIter<T, A> {
    #[inline]
    fn drop(&mut self) {
        while let Some(ptr) = self.ptr {
            unsafe {
                let mut node = core::ptr::read(ptr.as_ptr());
                self.alloc.deallocate(ptr.cast(), Layout::new::<FillQueueNode<T>>());
                self.ptr = NonNull::new(*node.prev.get_mut());
            }
        }
    }
}

impl<T> FusedIterator for ChopIter<T> {}