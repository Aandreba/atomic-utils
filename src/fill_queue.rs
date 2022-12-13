use core::{sync::atomic::{AtomicPtr, Ordering, AtomicIsize}, ptr::NonNull, iter::FusedIterator, alloc::{Layout, LayoutError}, mem::MaybeUninit, num::NonZeroUsize, cell::UnsafeCell};
use crate::{InnerFlag, FALSE, TRUE, AllocError};
use core::fmt::Debug;
#[cfg(feature = "alloc_api")]
use {alloc::{alloc::Global}, core::alloc::*};

// SAFETY: eight is not zero
const DEFAULT_BLOCK_SIZE: usize = 8;
const SIZE_LIMIT: isize = 1isize << (isize::BITS - 2);

macro_rules! impl_all {
    (impl $(@$tr:path =>)? $target:ident {
        $($t:tt)*
    }) => {
        cfg_if::cfg_if! {
            if #[cfg(feature = "alloc_api")] {
                impl<T, A: Allocator + Clone> $($tr for)? $target <T, A> {
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

#[repr(C)]
struct Block<T> {
    prev: Option<Box<Block<T>>>,
    nodes: [MaybeUninit<NodeValue<T>>]
}

#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub struct FillQueue<T, #[cfg(feature = "alloc_api")] A: Allocator = Global> {
    #[cfg(feature = "alloc_api")]
    block: UnsafeCell<Option<Box<Block<T>>>>,
    #[cfg(not(feature = "alloc_api"))]
    block: UnsafeCell<Option<Box<Block<T>>>>,
    block_size: usize,
    // Current block's head position
    idx: AtomicIsize,
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
        if block_size.get() >= SIZE_LIMIT { panic!("attempted to create a queue too big") }

        Self {
            block: UnsafeCell::new(None),
            idx: AtomicIsize::new((block_size.get() - 1) as isize),
            block_size: block_size.get(),
            #[cfg(feature = "alloc_api")]
            alloc: Global
        }
    }
}

#[docfg::docfg(feature = "alloc_api")]
impl<T, A: Allocator + Clone> FillQueue<T, A> {
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
        todo!()
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
            self.block.load(Ordering::Relaxed).is_null()
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
            // Bit structure
            //  [1]     [1]       ...
            //  ^^^     ^^^
            //  Locked  Allocating
            
            const ALLOCATION_SETTER: isize = isize::MIN | SIZE_LIMIT;

            let idx = loop {
                // Get index for our value 
                let mut idx = self.idx.fetch_add(1, Ordering::AcqRel);

                // New block is being created, wait for it to be done
                if idx.is_negative() {
                    // Avoid quick succesive additions to idx by checking in an inner loop the status of idx,
                    // thus avoiding danger to overflow back to positives
                    loop {
                        let idx = self.idx.load(Ordering::Relaxed);
                        if !idx.is_negative() { break }

                        match idx & SIZE_LIMIT {
                            // We aren't allocating, so wait for the initialization, which should be fast
                            0 => core::hint::spin_loop(),
                            // If we are allocating, yield to the OS scheduler
                            #[cfg(feature = "std")]
                            _ => std::thread::yield_now(),
                            // We don't have an OS to yield to
                            #[cfg(not(feature = "std"))]
                            _ => {}
                        }
                    }
                }

                // There is no more space in the block
                if idx >= self.block_size as isize {
                    match self.idx.compare_exchange(idx, ALLOCATION_SETTER, Ordering::AcqRel, Ordering::Acquire) {
                        // We get to create the new block
                        Ok(_) => {
                            // Allocate the equivalent of `Vec::with_capacity(block_size)`
                            let layout = Self::calculate_layout(self.block_size).map_err(|_| AllocError)?;
                            #[cfg(feature = "alloc_api")]
                            let alloc: Result<NonNull<u8>, AllocError> = self.alloc.allocate(layout).map(NonNull::cast::<u8>);
                            #[cfg(not(feature = "alloc_api"))]
                            let alloc: Result<NonNull<u8>, AllocError> = NonNull::new(unsafe { alloc::alloc::alloc(layout) }).ok_or(AllocError);

                            let block = match alloc {
                                Ok(x) => unsafe {
                                    // Inform other threads that we are done allocating and they should stop yielding.
                                    self.idx.store(isize::MIN, Ordering::Release);

                                    // Create a fat pointer to the new block
                                    #[cfg(feature = "nightly")]
                                    let ptr: *mut Block<T> = core::ptr::from_raw_parts_mut(x.as_mut_ptr(), self.block_size);
                                    #[cfg(not(feature = "nightly"))]
                                    let ptr: *mut Block<T> = {
                                        let dangling_slice = core::slice::from_raw_parts_mut(x.as_mut_ptr(), self.block_size);
                                        core::mem::transmute(dangling_slice)
                                    };

                                    // Put the new block inside a `Box`
                                    #[cfg(feature = "alloc_api")]
                                    let block = Box::from_raw_in(ptr, self.alloc.clone());
                                    #[cfg(not(feature = "alloc_api"))]
                                    let block = Box::from_raw(ptr);

                                    /* SAFETY: Since we've locked the current queue, we are the only thread with acces to the head */
                                    // Set the new head block
                                    let prev = core::mem::replace(&mut *self.block.get(), Some(block));
                                    // Initialize block (specifically, set it's parent)
                                    ptr.cast::<Option<Box<Block<T>>>>().write(prev);
                                    
                                    // Next value must be at index 1 of the block, since 0 is the one we'll use
                                    self.idx.store(1, Ordering::Release);
                                    break 0
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

        fn calculate_layout (len: usize) -> Result<Layout, LayoutError> {
            let prev = Layout::new::<Option<Box<Block<T>>>>();
            let nodes = Layout::array::<MaybeUninit<NodeValue<T>>>(len)?;
            let padding = {
                let len = prev.size();
                let align = nodes.align();

                let len_rounded_up = len.wrapping_add(align).wrapping_sub(1) & !align.wrapping_sub(1);
                len_rounded_up.wrapping_sub(len)
            };

            return Layout::from_size_align(prev.size() + padding + nodes.len(), prev.align())
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
            todo!()
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
        let ptr = self.block.swap(core::ptr::null_mut(), Ordering::AcqRel);
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
            core::ptr::replace(self.block.get_mut(), core::ptr::null_mut())
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