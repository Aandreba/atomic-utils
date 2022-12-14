use core::{ops::{Shl, Shr, BitAnd, Not}, sync::atomic::Ordering};
use alloc::{boxed::Box};
use bytemuck::Zeroable;
use num_traits::{One, Zero, Num, WrappingSub};
use crate::{traits::{AtomicInt}, AllocError};
use crate::InnerFlag;
#[cfg(feature = "alloc_api")]
use {alloc::alloc::Global, core::alloc::*};

/// Bitfield used with atomic operations
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
#[repr(transparent)]
pub struct AtomicBitBox<T: AtomicInt = InnerFlag, #[cfg(feature = "alloc_api")] A: Allocator = Global> {
    #[cfg(feature = "alloc_api")]
    bits: Box<[T], A>,
    #[cfg(not(feature = "alloc_api"))]
    bits: Box<[T]>,
}

impl<T: AtomicInt> AtomicBitBox<T> where T::Primitive: BitFieldAble {
    #[inline(always)]
    pub fn new (bits: usize) -> Self {
        Self::try_new(bits).unwrap()
    }

    #[inline(always)]
    pub fn try_new (bits: usize) -> Result<Self, AllocError> {
        let bytes = bits.div_ceil(core::mem::size_of::<T>());
        let bits = unsafe {
            let uninit = Box::<[T]>::new_zeroed_slice(bytes);
            uninit.assume_init()
        };
        
        Ok(Self { bits })
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "alloc_api")] {
        impl<T: AtomicInt, A: Allocator> AtomicBitBox<T, A> where T::Primitive: BitFieldAble {
            #[inline(always)]
            pub fn new_in (bits: usize, alloc: A) -> Self {
                Self::try_new_in(bits, alloc).unwrap()
            }
            
            #[inline]
            pub fn try_new_in (bits: usize, alloc: A) -> Result<Self, AllocError> {
                let bytes = bits.div_ceil(core::mem::size_of::<T>());
                let bits = unsafe {
                    let uninit = Box::<[T], _>::new_zeroed_slice_in(bytes, alloc);
                    uninit.assume_init()
                };
                
                Ok(Self { bits })
            }
        
            pub fn get (&self, idx: usize, order: Ordering) -> Option<bool> {
                let byte = idx / core::mem::size_of::<T>();
                let idx = idx % core::mem::size_of::<T>();
        
                if let Some(byte) = <[T]>::get(&self.bits, byte) {
                    let v = byte.load(order);
                    let mask = T::Primitive::one() << idx;
                    return Some((v & mask) != T::Primitive::zero())
                }
        
                None
            }
        
            #[inline(always)]
            pub fn set (&self, v: bool, idx: usize, order: Ordering) -> Option<bool> {
                if v { return self.set_true(idx, order) }
                self.set_false(idx, order)
            }
        
            #[inline]
            pub fn set_true (&self, idx: usize, order: Ordering) -> Option<bool> {
                let byte = idx / core::mem::size_of::<T>();
                let idx = idx % core::mem::size_of::<T>();
        
                if let Some(byte) = <[T]>::get(&self.bits, byte) {
                    let mask = T::Primitive::one() << idx;
                    let prev = byte.fetch_or(mask, order);
                    return Some((prev & mask) != T::Primitive::zero())
                }
        
                None
            }
        
            #[inline]
            pub fn set_false (&self, idx: usize, order: Ordering) -> Option<bool> {
                let byte = idx / core::mem::size_of::<T>();
                let idx = idx % core::mem::size_of::<T>();
        
                if let Some(byte) = <[T]>::get(&self.bits, byte) {
                    let zero = T::Primitive::zero();
                    let mask = T::Primitive::one() << idx;
        
                    let prev = byte.fetch_and((!zero).wrapping_sub(&mask), order);
                    return Some((prev & mask) != zero)
                }
        
                None
            }
        }
    } else {
        impl<T: AtomicInt> AtomicBitBox<T> where T::Primitive: BitFieldAble {        
            pub fn get (&self, idx: usize, order: Ordering) -> Option<bool> {
                let byte = idx / core::mem::size_of::<T>();
                let idx = idx % core::mem::size_of::<T>();
        
                if let Some(byte) = <[T]>::get(&self.bits, byte) {
                    let v = byte.load(order);
                    let mask = T::Primitive::one() << idx;
                    return Some((v & mask) != T::Primitive::zero())
                }
        
                None
            }
        
            #[inline(always)]
            pub fn set (&self, v: bool, idx: usize, order: Ordering) -> Option<bool> {
                if v { return self.set_true(idx, order) }
                self.set_false(idx, order)
            }
        
            #[inline]
            pub fn set_true (&self, idx: usize, order: Ordering) -> Option<bool> {
                let byte = idx / core::mem::size_of::<T>();
                let idx = idx % core::mem::size_of::<T>();
        
                if let Some(byte) = <[T]>::get(&self.bits, byte) {
                    let mask = T::Primitive::one() << idx;
                    let prev = byte.fetch_or(mask, order);
                    return Some((prev & mask) != T::Primitive::zero())
                }
        
                None
            }
        
            #[inline]
            pub fn set_false (&self, idx: usize, order: Ordering) -> Option<bool> {
                let byte = idx / core::mem::size_of::<T>();
                let idx = idx % core::mem::size_of::<T>();
        
                if let Some(byte) = <[T]>::get(&self.bits, byte) {
                    let zero = T::Primitive::zero();
                    let mask = T::Primitive::one() << idx;
        
                    let prev = byte.fetch_and((!zero).wrapping_sub(&mask), order);
                    return Some((prev & mask) != zero)
                }
        
                None
            }
        }
    }
}

pub trait BitFieldAble: Num + Copy + Zeroable + Eq + WrappingSub + BitAnd<Output = Self> + Shl<usize, Output = Self> + Shr<usize, Output = Self> + Not<Output = Self> {}
impl<T> BitFieldAble for T where T: Num + Copy + Zeroable + Eq + WrappingSub + BitAnd<Output = Self> + Shl<usize, Output = Self> + Shr<usize, Output = Self> + Not<Output = Self> {}