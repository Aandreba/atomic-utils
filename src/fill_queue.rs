use core::{sync::atomic::{AtomicPtr, Ordering, AtomicUsize, AtomicIsize}, ptr::NonNull, iter::FusedIterator, alloc::Layout, mem::MaybeUninit, num::NonZeroUsize};
use crate::{InnerFlag, FALSE, TRUE, AllocError};
use core::fmt::Debug;
#[cfg(feature = "alloc_api")]
use {alloc::{alloc::Global}, core::alloc::*};

// SAFETY: eight is not zero
const DEFAULT_BLOCK_SIZE: NonZeroUsize = unsafe {
    NonZeroUsize::new_unchecked(8)
};

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
                impl<T> $($tr for)? $target<T> {
                    $($t)*
                }
            }
        }
    };
}

struct NodeValue<T> {
    init: InnerFlag,
    v: MaybeUninit<T>,
    // only first elements of a block have a parent set, and that parent may be null
    prev: *mut NodeValue<T>
}

#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub struct FillQueue<T, #[cfg(feature = "alloc_api")] A: Allocator = Global> {
    // Current block's firts value slot
    head_block: AtomicPtr<NodeValue<T>>,
    // Current block's head position
    idx: AtomicIsize,
    block_size: usize,
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
    #[inline]
    pub const fn new () -> Self {
        Self::new_with_block_size(DEFAULT_BLOCK_SIZE)
    }

    #[inline]
    pub const fn new_with_block_size (block_size: NonZeroUsize) -> Self {
        const SIZE_LIMIT: usize = isize::MAX as usize;
        if block_size.get() >= SIZE_LIMIT { panic!("attempted to create a queue too big") }

        Self {
            head_block: AtomicPtr::new(core::ptr::null_mut()),
            idx: AtomicIsize::new((block_size.get() - 1) as isize),
            block_size: block_size.get(),
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
    #[inline]
    pub const fn new_in (alloc: A) -> Self {
        Self::new_with_block_size_in(DEFAULT_BLOCK_SIZE)
    }

    #[inline(always)]
    pub const fn new_with_block_size_in (block_size: NonZeroUsize, alloc: A) -> Self {
        let block_limit = block_size.get() - 1;
        Self {
            head_block: AtomicPtr::new(core::ptr::null_mut()),
            idx: AtomicUsize::new(block_limit),
            block_limit,
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
        #[inline]
        pub fn block_size (&self) -> usize {
            return self.block_size
        }

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
            self.head_block.load(Ordering::Relaxed).is_null()
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
            const ADD_MASK: isize = isize::MIN.add_wraping(1);

            // todo add with the mask, so when we add it is with locking possible chops

            let (idx, ptr) = loop {
                // Get index for our value 
                let mut idx = self.idx.fetch_add(1, Ordering::AcqRel);

                // New block is being created, wait for it to be done
                if idx.is_negative() {
                    // Avoid quick succesive additions to idx by checking in an inner loop the status of idx,
                    // thus avoiding danger to overflow back to positives
                    loop {
                        if !self.idx.load(Ordering::Relaxed).is_negative() { break }
                        #[cfg(feature = "std")]
                        std::thread::yield_now();
                    }
                }

                // There is no more space in the block
                if idx >= self.block_size as isize {
                    match self.idx.compare_exchange(idx, isize::MIN, Ordering::AcqRel, Ordering::Acquire) {
                        // We get to create the new block
                        Ok(_) => {
                            // Allocate the equivalent of `Vec::with_capacity(block_size)`
                            let layout = Layout::array::<NodeValue<T>>(self.block_size).map_err(|_| AllocError)?;
                            #[cfg(feature = "alloc_api")]
                            let alloc: Result<NonNull<NodeValue<T>>, AllocError> = self.alloc.allocate(layout).map(NonNull::cast::<NodeValue<T>>);
                            #[cfg(not(feature = "alloc_api"))]
                            let alloc: Result<NonNull<NodeValue<T>>, AllocError> = NonNull::new(unsafe { alloc::alloc::alloc(layout).cast::<NodeValue<T>>() }).ok_or(AllocError);

                            let block = match alloc {
                                Ok(x) => unsafe {
                                    // Set the new head of the list
                                    let prev = self.head.swap(x.as_mut_ptr(), Ordering::AcqRel);
                                    // Initialize node
                                    x.as_mut_ptr().write(NodeValue {
                                        init: InnerFlag::new(FALSE),
                                        v: MaybeUninit::uninit(),
                                        prev
                                    });

                                    // Next value must be at index 1 of the block, since 0 is the one we'll use
                                    self.idx.store(1, Ordering::Release);
                                    break (0, x.as_mut_ptr())
                                },
                                
                                Err(e) => {
                                    // Give someone else the chance to allocate the new block
                                    self.idx.store(self.block_size, Ordering::Release);
                                    return Err(e)
                                }
                            };
                        },

                        // Someone else beat us to creating the new block, we'll have to wait for them
                        Err(_) => continue
                    }
                }

                let ptr = self.ptr.load(Ordering::Acquire);
                break (idx, todo!())
            };

            // There is no more space in the block
            let my_node = 0;

            /*
            let node = FillQueueNode {
                init: InnerFlag::new(FALSE),
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

            let prev = self.head_block.swap(ptr.as_ptr(), Ordering::AcqRel);
            unsafe {
                let rf = &mut *ptr.as_ptr();
                rf.prev = prev;
                rf.init.store(TRUE, Ordering::Release);
            }*/

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
                init: InnerFlag::new(TRUE),
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
                let prev = core::ptr::replace(self.head_block.get_mut(), ptr.as_ptr());
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
        let ptr = self.head_block.swap(core::ptr::null_mut(), Ordering::AcqRel);
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
            core::ptr::replace(self.head_block.get_mut(), core::ptr::null_mut())
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