use core::{sync::atomic::{AtomicPtr, Ordering, fence, AtomicUsize}, ptr::NonNull, alloc::{Layout, LayoutError}, mem::{MaybeUninit}, num::NonZeroUsize, marker::PhantomData, cell::UnsafeCell, ops::Range};
use crate::{InnerFlag, FALSE, AllocError, TRUE};

#[cfg(feature = "alloc_api")]
use {core::mem::ManuallyDrop, alloc::{alloc::Global}, core::alloc::*};

// SAFETY: eight is not zero
const DEFAULT_BLOCK_SIZE: NonZeroUsize = unsafe { NonZeroUsize::new_unchecked(8) };
const ALLOCATION_MASK: isize = isize::MIN;

const EMPTY: *mut u8 = 0usize as *mut u8;
const LOCKED: *mut u8 = 1usize as *mut u8;
const ALLOCATING: *mut u8 = 2usize as *mut u8;

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

#[repr(C)]
struct Node<T> {
    init: InnerFlag,
    v: UnsafeCell<MaybeUninit<T>>
}

#[repr(C)]
struct Block<T> {
    init: InnerFlag,
    prev: *mut Self,
    // Current block's head position
    idx: AtomicUsize,
    nodes: [Node<T>]
}

impl<T> Block<T> {
    #[inline]
    fn len (&self) -> usize {
        #[cfg(feature = "nightly")]
        return core::ptr::metadata(self);
        #[cfg(not(feature = "nightly"))]
        return self.nodes.len()
    }

    #[inline]
    fn next_node (&self) -> Option<&Node<T>> {
        let idx = self.idx.fetch_add(1, Ordering::AcqRel);
        if idx > self.len() { return None }
        return Some(&self.nodes[idx])
    }

    #[inline]
    fn next_node_mut (&mut self) -> Option<&mut Node<T>> {
        let self_idx = self.idx.get_mut();
        let idx = *self_idx;
        *self_idx += 1;
        if idx > self.len() { return None }
        return Some(&mut self.nodes[idx])
    }
}

#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub struct FillQueue<T, #[cfg(feature = "alloc_api")] A: Allocator = Global> {
    block: AtomicPtr<u8>,
    block_size: usize,
    _phtm: PhantomData<T>,
    #[cfg(feature = "alloc_api")]
    alloc: ManuallyDrop<A>
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
        if block_size.get() >= ALLOCATION_MASK as usize { panic!("attempted to create a queue too big") }

        Self {
            block: AtomicPtr::new(core::ptr::null_mut()),
            block_size: block_size.get(),
            _phtm: PhantomData,
            #[cfg(feature = "alloc_api")]
            alloc: ManuallyDrop::new(Global)
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
        Self::new_with_block_size_in(DEFAULT_BLOCK_SIZE, alloc)
    }

    #[inline]
    pub const fn new_with_block_size_in (block_size: NonZeroUsize, alloc: A) -> Self {
        if block_size.get() >= ALLOCATION_MASK as usize { panic!("attempted to create a queue too big") }

        Self {
            block: AtomicPtr::new(core::ptr::null_mut()),
            block_size: block_size.get(),
            _phtm: PhantomData,
            alloc: ManuallyDrop::new(alloc)
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
        const CALCULATED_RAW_LAYOUT: (Layout, usize, usize, usize) = match Self::calculate_layout(0) {
            Ok(x) => x,
            Err(_) => unreachable!()
        };
        const RAW_LAYOUT: Layout = Self::CALCULATED_RAW_LAYOUT.0;
        const PREV_OFFSET: usize = Self::CALCULATED_RAW_LAYOUT.1;
        const IDX_OFFSET: usize = Self::CALCULATED_RAW_LAYOUT.2;
        const NODES_OFFSET: usize = Self::CALCULATED_RAW_LAYOUT.3;

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
            todo!()
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
            let node: &Node<T> = 'outer: loop {
                // Get block for our value
                let mut block_ptr = self.block.load(Ordering::Acquire);
                
                loop {
                    match block_ptr {
                        // There is currently no block in the queue. Let's try to allocate a new one
                        EMPTY => match self.allocate_new_block(block_ptr) {
                            // Successfully allocated a new block, and we got it's first node
                            Ok(Some(node)) => break 'outer node,
                            // Someone else beat us to allocating a new block, we'll wait for them.
                            Ok(None) => {
                                block_ptr = self.block.load(Ordering::Acquire);
                                continue
                            },
                            // Error allocating new block
                            Err(e) => {
                                // Give someone else the chance of creating a new block
                                self.block.store(EMPTY, Ordering::Release);
                                return Err(e)
                            }
                        },

                        // Someone is initializing a new block, wait for them
                        LOCKED => loop {
                            core::hint::spin_loop();
                            block_ptr = self.block.load(Ordering::Relaxed);
                            if block_ptr != LOCKED { break }
                        },

                        // Someone is allocating a new block, wait for them
                        ALLOCATING => loop {
                            #[cfg(feature = "std")]
                            std::thread::yield_now();
                            block_ptr = self.block.load(Ordering::Relaxed);
                            if block_ptr != ALLOCATING { break }
                        },

                        _ => break
                    }
                }
                
                // Get reference to it
                let block = unsafe {
                    &*crate::ptr_from_raw_parts_mut::<Block<T>>(block_ptr.cast(), self.block_size)
                };

                // Try to get next node
                if let Some(node) = block.next_node() {
                    break node
                }

                // This block is now full, let's rerun and try to get a node again
                block_ptr = self.block.load(Ordering::Acquire);
            };

            // Initialize the appropiate node
            unsafe {
                (&mut *node.v.get()).write(v);
                node.init.store(TRUE, Ordering::Release);
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
            let self_block = self.block.get_mut();

            let node: &mut Node<T> = 'outer: loop {
                // Get block for our value
                let block_ptr: *mut u8 = match *self_block {
                    // There is currently no block in the queue. Let's allocate a new one
                    EMPTY => {
                        // Allocate the new block (somewhat equivalent to `Vec::with_capacity(block_size)` with extra info)
                        let (layout, prev_offset, idx_offset, nodes_offset) = Self::calculate_layout(self.block_size).map_err(|_| AllocError)?;
                        #[cfg(feature = "alloc_api")]
                        let alloc: Result<NonNull<u8>, AllocError> = self.alloc.allocate(layout).map(NonNull::cast::<u8>);
                        #[cfg(not(feature = "alloc_api"))]
                        let alloc: Result<NonNull<u8>, AllocError> = NonNull::new(unsafe { alloc::alloc::alloc(layout) }).ok_or(AllocError);

                        match alloc {
                            Ok(ptr) => unsafe {
                                // Initialize all nodes (by setting them as uninitialized)
                                let nodes = ptr.as_ptr().add(nodes_offset).cast::<Node<T>>();
                                for i in 0..self.block_size {
                                    nodes.add(i).cast::<InnerFlag>().write(InnerFlag::new(FALSE));
                                }

                                // Initialize block's index at one (we'll take index zero)
                                ptr.as_ptr().add(idx_offset).cast::<AtomicUsize>().write(AtomicUsize::new(1));

                                // Set the new head block
                                let prev = core::mem::replace(self_block, ptr.as_ptr());

                                // Initialize block (specifically, set it's parent)
                                let prev = crate::ptr_from_raw_parts_mut(prev.cast(), if prev.is_null() { 0 } else { self.block_size });
                                ptr.as_ptr().add(prev_offset).cast::<*mut Block<T>>().write(prev);

                                // Mark node as initialized before we end
                                ptr.as_ptr().cast::<InnerFlag>().write(InnerFlag::new(TRUE));
                                
                                break 'outer &mut *ptr.as_ptr().add(nodes_offset).cast()
                            },
                            
                            Err(e) => {
                                // Give someone else the chance to allocate the new block
                                *self_block = EMPTY;
                                return Err(e)
                            }
                        };
                    },

                    // Someone is initializing a new block, wait for them
                    LOCKED => unreachable!(),

                    // Someone is allocating a new block, wait for them
                    ALLOCATING => unreachable!(),
                    
                    other => other
                };
                
                // Get reference to it
                let block = unsafe {
                    &mut *crate::ptr_from_raw_parts_mut::<Block<T>>(block_ptr.cast(), self.block_size)
                };

                // Try to get next node
                if let Some(node) = block.next_node_mut() {
                    break node
                }

                // This block is now full, let's rerun and try to get a node again
                *self_block = EMPTY;
            };

            // Initialize the appropiate node
            unsafe {
                (&mut *node.v.get()).write(v);
                *node.init.get_mut() = TRUE;
            }

            Ok(())
        }

        fn allocate_new_block (&self, prev: *mut u8) -> Result<Option<&Node<T>>, AllocError> {
            match self.block.compare_exchange(prev, ALLOCATING, Ordering::AcqRel, Ordering::Acquire) {
                // We get to create the new block
                Ok(_) => {
                    // Allocate the new block (somewhat equivalent to `Vec::with_capacity(block_size)` with extra info)
                    let (layout, prev_offset, idx_offset, nodes_offset) = Self::calculate_layout(self.block_size).map_err(|_| AllocError)?;
                    #[cfg(feature = "alloc_api")]
                    let alloc: Result<NonNull<u8>, AllocError> = self.alloc.allocate(layout).map(NonNull::cast::<u8>);
                    #[cfg(not(feature = "alloc_api"))]
                    let alloc: Result<NonNull<u8>, AllocError> = NonNull::new(unsafe { alloc::alloc::alloc(layout) }).ok_or(AllocError);

                    match alloc {
                        Ok(ptr) => unsafe {
                            // Inform other threads that we are done allocating and they should stop yielding.
                            self.block.store(LOCKED, Ordering::Release);

                            // Initialize all nodes (by setting them as uninitialized)
                            let nodes = ptr.as_ptr().add(nodes_offset).cast::<Node<T>>();
                            for i in 0..self.block_size {
                                nodes.add(i).cast::<InnerFlag>().write(InnerFlag::new(FALSE));
                            }

                            // Initialize block's index at one (we'll take index zero)
                            ptr.as_ptr().add(idx_offset).cast::<AtomicUsize>().write(AtomicUsize::new(1));

                            // Mark node as uninitialized before we put it in
                            ptr.as_ptr().cast::<InnerFlag>().write(InnerFlag::new(FALSE));

                            // Initialize block (specifically, set it's parent)
                            let prev = crate::ptr_from_raw_parts_mut(prev.cast(), if prev.is_null() { 0 } else { self.block_size });
                            ptr.as_ptr().add(prev_offset).cast::<*mut Block<T>>().write(prev);

                            // Mark node as initialized
                            (&*ptr.as_ptr().cast::<InnerFlag>()).store(TRUE, Ordering::Release);
                            
                            return Ok(Some(&*ptr.as_ptr().add(nodes_offset).cast()))
                        },
                        
                        Err(e) => {
                            // Give someone else the chance to allocate the new block
                            self.block.store(EMPTY, Ordering::Release);
                            return Err(e)
                        }
                    };
                },

                // Someone else beat us to creating the new block, we'll have to wait for them
                Err(_) => return Ok(None)
            }
        }

        fn calculate_layout_fast () {
            todo!()
        }

        const fn calculate_layout (len: usize) -> Result<(Layout, usize, usize, usize), LayoutError> {
            macro_rules! tri {
                ($e:expr) => {
                    match $e {
                        Ok(x) => x,
                        Err(e) => return Err(e)
                    }
                };
            }

            #[inline]
            const fn add_field (parent: Layout, field: Layout) -> Result<(Layout, usize), LayoutError> {
                #[cfg(feature = "nightly")]
                let padding = parent.padding_needed_for(field.align());
                #[cfg(not(feature = "nightly"))]
                let padding = {
                    let len = parent.size();
                    let align = field.align();

                    // Rounded up value is:
                    //   len_rounded_up = (len + align - 1) & !(align - 1);
                    // and then we return the padding difference: `len_rounded_up - len`.
                    //
                    // We use modular arithmetic throughout:
                    //
                    // 1. align is guaranteed to be > 0, so align - 1 is always
                    //    valid.
                    //
                    // 2. `len + align - 1` can overflow by at most `align - 1`,
                    //    so the &-mask with `!(align - 1)` will ensure that in the
                    //    case of overflow, `len_rounded_up` will itself be 0.
                    //    Thus the returned padding, when added to `len`, yields 0,
                    //    which trivially satisfies the alignment `align`.
                    //
                    // (Of course, attempts to allocate blocks of memory whose
                    // size and padding overflow in the above manner should cause
                    // the allocator to yield an error anyway.)

                    let len_rounded_up = len.wrapping_add(align).wrapping_sub(1) & !align.wrapping_sub(1);
                    len_rounded_up.wrapping_sub(len)
                };

                let offset = parent.size() + padding;
                let layout = tri! {
                    Layout::from_size_align(offset + field.size(), match parent.align() <= field.align() {
                        true => field.align(),
                        false => parent.align(),
                    })
                };
                return Ok((layout, offset))
            }

            let result = Layout::new::<InnerFlag>();
            let (result, prev) = tri! { add_field(result, Layout::new::<*mut Block<T>>()) };
            let (result, idx) = tri! { add_field(result, Layout::new::<AtomicUsize>()) };

            #[cfg(not(feature = "nightly"))]
            let (result, nodes) = tri! { 
                add_field(result, tri! {{
                    #[inline]
                    const fn inner(
                        element_size: usize,
                        align: usize,
                        n: usize,
                    ) -> Result<Layout, LayoutError> {
                        // We need to check two things about the size:
                        //  - That the total size won't overflow a `usize`, and
                        //  - That the total size still fits in an `isize`.
                        // By using division we can check them both with a single threshold.
                        // That'd usually be a bad idea, but thankfully here the element size
                        // and alignment are constants, so the compiler will fold all of it.
                        if element_size != 0 && n > { isize::MAX as usize - (align - 1) } / element_size {
                            return Err(LayoutError);
                        }

                        let array_size = element_size * n;

                        // SAFETY: We just checked above that the `array_size` will not
                        // exceed `isize::MAX` even when rounded up to the alignment.
                        // And `Alignment` guarantees it's a power of two.
                        unsafe { Ok(Layout::from_size_align_unchecked(array_size, align.as_usize())) }
                    }

                    // Reduce the amount of code we need to monomorphize per `T`.
                    inner(core::mem::size_of::<T>(), core::mem::align_of::<T>(), len)
                }})
            };

            #[cfg(feature = "nightly")]
            let (result, nodes) = tri! { add_field(result, tri! { Layout::array::<Node<T>>(len) }) };

            return Ok((result, prev, idx, nodes))
        }
    }
}

#[cfg(feature = "alloc_api")]
impl<T, A: Allocator + Clone> FillQueue<T, A> {
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
    pub fn chop (&self) -> ChopIter<T, A> {
        loop {
            match self.block.swap(EMPTY, Ordering::AcqRel) {
                EMPTY => return ChopIter {
                    ptr: None,
                    #[cfg(not(feature = "nightly"))]
                    block_size: self.block_size,
                    range: Range::default(),
                    alloc: ManuallyDrop::into_inner(self.alloc.clone())
                },
    
                LOCKED => todo!(),
    
                ALLOCATING => todo!(),
                
                block => {
                    let ptr: NonNull<Block<T>> = unsafe { NonNull::new_unchecked(crate::ptr_from_raw_parts_mut(block.cast(), self.block_size)) };
                    let limit = *unsafe { ptr.as_mut() }.idx.get_mut();
                    
                    return ChopIter {
                        ptr: Some(ptr),
                        #[cfg(not(feature = "nightly"))]
                        block_size: self.block_size,
                        range: 0..limit,
                        alloc: A::clone(&self.alloc)
                    }
                }
            }
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
    pub fn chop_mut (&mut self) -> ChopIter<T, A> {
        let self_block = self.block.get_mut();
        let self_idx = self.idx.get_mut();

        let ptr = core::mem::replace(self_block, core::ptr::null_mut());
        let len = if ptr.is_null() { 0 } else { self.block_size };
        let limit = core::mem::replace(self_idx, 0);
        let range;

        // New block is being allocated, which means this one is full 
        if limit.is_negative() {
            range = 0..len
        } else {
            range = 0..(limit as usize)
        }

        ChopIter {
            ptr: NonNull::new(crate::ptr_from_raw_parts_mut(ptr.cast(), len)),
            #[cfg(not(feature = "nightly"))]
            block_size: self.block_size,
            range,
            alloc: ManuallyDrop::into_inner(self.alloc.clone())
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
        let ptr = self.block.swap(core::ptr::null_mut(), Ordering::Relaxed);
        let limit = self.idx.swap(0, Ordering::Relaxed);
        fence(Ordering::Acquire);

        let len = if ptr.is_null() { 0 } else { self.block_size };
        let range;

        // New block is being allocated, which means this one is full 
        if limit.is_negative() {
            range = 0..len
        } else {
            range = 0..(limit as usize)
        }

        ChopIter {
            ptr: NonNull::new(crate::ptr_from_raw_parts_mut(ptr.cast(), len)),
            #[cfg(not(feature = "nightly"))]
            block_size: self.block_size,
            range
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
        let self_block = self.block.get_mut();
        let self_idx = self.idx.get_mut();

        let ptr = core::mem::replace(self_block, core::ptr::null_mut());
        let limit = core::mem::replace(self_idx, 0);
        let len = if ptr.is_null() { 0 } else { self.block_size };
        let range;

        // New block is being allocated, which means this one is full 
        if limit.is_negative() {
            range = 0..len
        } else {
            range = 0..(limit as usize)
        }

        ChopIter {
            ptr: NonNull::new(crate::ptr_from_raw_parts_mut(ptr.cast(), len)),
            #[cfg(not(feature = "nightly"))]
            block_size: self.block_size,
            range
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

impl_all! {
    impl @Drop => FillQueue {
        #[inline]
        fn drop(&mut self) {
            let self_block = self.block.get_mut();
            let self_idx = self.idx.get_mut();

            let ptr = core::mem::replace(self_block, core::ptr::null_mut());
            let limit = core::mem::replace(self_idx, 0);
            let len = if ptr.is_null() { 0 } else { self.block_size };
            let range;

            // New block is being allocated, which means this one is full 
            if limit.is_negative() {
                range = 0..len
            } else {
                range = 0..(limit as usize)
            }

            #[cfg(feature = "alloc_api")]
            let _chop: ChopIter<T, A>;
            #[cfg(not(feature = "alloc_api"))]
            let _chop: ChopIter<T>;
            _chop = ChopIter {
                ptr: NonNull::new(crate::ptr_from_raw_parts_mut(ptr.cast(), len)),
                #[cfg(not(feature = "nightly"))]
                block_size: self.block_size,
                range,
                #[cfg(feature = "alloc_api")]
                alloc: unsafe { ManuallyDrop::take(&mut self.alloc) }
            };
        }
    }
}

/// Iterator of [`FillQueue::chop`] and [`FillQueue::chop_mut`]
pub struct ChopIter<T, #[cfg(feature = "alloc_api")] A: Allocator = Global> {
    ptr: Option<NonNull<Block<T>>>,
    #[cfg(not(feature = "nightly"))]
    block_size: usize,
    range: Range<usize>,
    #[cfg(feature = "alloc_api")]
    alloc: A
}

impl_all! {
    impl @Iterator => ChopIter {
        type Item = T;

        #[inline]
        fn next(&mut self) -> Option<Self::Item> {
            while let Some(ptr) = self.ptr {
                unsafe {
                    let block = &*ptr.as_ptr();

                    // Wait for block to initialize (shouldn't be long)
                    while block.init.load(Ordering::Acquire) == FALSE {
                        core::hint::spin_loop()
                    }

                    if let Some(i) = self.range.next_back() {
                        let node = block.nodes.get_unchecked(i);

                        // Wait for node to initialize (shouldn't be long)
                        while node.init.load(Ordering::Acquire) == FALSE {
                            core::hint::spin_loop()
                        }

                        return Some((&*node.v.get()).assume_init_read())
                    }

                    self.ptr = NonNull::new(block.prev);
                    #[cfg(feature = "nightly")]
                    { self.range = 0..core::ptr::metadata(block.prev) };
                    #[cfg(not(feature = "nightly"))]
                    { self.range = 0..(if block.prev.is_null() { 0 } else { self.block_size }) };

                    let (layout, _, _) = FillQueue::<T>::calculate_layout(block.nodes.len()).unwrap();
                    #[cfg(feature = "alloc_api")]
                    self.alloc.deallocate(ptr.cast(), layout);
                    #[cfg(not(feature = "alloc_api"))]
                    alloc::alloc::dealloc(ptr.as_ptr().cast(), layout);
                }
            }
            return None
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

#[cfg(feature = "alloc_api")]
impl<T, A: core::fmt::Debug + Allocator> core::fmt::Debug for FillQueue<T, A> {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.debug_struct("FillQueue").field("alloc", &self.alloc).finish_non_exhaustive()
    }
}
#[cfg(not(feature = "alloc_api"))]
impl<T> core::fmt::Debug for FillQueue<T> {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        f.debug_struct("FillQueue").finish_non_exhaustive()
    }
}