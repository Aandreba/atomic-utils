use crate::{div_ceil, InnerAtomicFlag};
use crate::{traits::AtomicInt, AllocError};
use alloc::boxed::Box;
use bytemuck::Zeroable;
use core::{
    ops::{BitAnd, Not, Shl, Shr},
    sync::atomic::Ordering,
};
use num_traits::{Num, One, WrappingSub, Zero};
#[cfg(feature = "alloc_api")]
use {alloc::alloc::Global, core::alloc::*};

/// Bitfield used with atomic operations
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub struct AtomicBitBox<
    T: AtomicInt = InnerAtomicFlag,
    #[cfg(feature = "alloc_api")] A: Allocator = Global,
> {
    #[cfg(feature = "alloc_api")]
    bits: Box<[T], A>,
    #[cfg(not(feature = "alloc_api"))]
    bits: Box<[T]>,
    len: usize,
}

impl<T: AtomicInt> AtomicBitBox<T>
where
    T::Primitive: BitFieldAble,
{
    /// Allocates a new bitfield. All values are initialized to `false`.
    ///
    /// # Panics
    /// This method panics if the memory allocation fails
    #[inline]
    pub fn new(len: usize) -> Self {
        Self::try_new(len).unwrap()
    }

    /// Allocates a new bitfield. All values are initialized to `false`.
    ///
    /// # Errors
    /// This method returns an error if the memory allocation fails
    #[inline]
    pub fn try_new(len: usize) -> Result<Self, AllocError> {
        let count = div_ceil(len, Self::BIT_SIZE);

        let bits;
        unsafe {
            cfg_if::cfg_if! {
                if #[cfg(feature = "nightly")] {
                    let uninit = Box::<[T]>::new_zeroed_slice(count);
                    bits = uninit.assume_init()
                } else {
                    let mut tmp = alloc::vec::Vec::with_capacity(count);
                    core::ptr::write_bytes(tmp.as_mut_ptr(), 0, count);
                    tmp.set_len(count);
                    bits = tmp.into_boxed_slice();
                }
            };
        }

        Ok(Self { bits, len })
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "alloc_api")] {
        impl<T: AtomicInt, A: Allocator> AtomicBitBox<T, A> where T::Primitive: BitFieldAble {
            const BIT_SIZE: usize = 8 * core::mem::size_of::<T>();

            /// Allocates a new bitfield. All values are initialized to `false`.
            ///
            /// # Panics
            /// This method panics if the memory allocation fails
            #[inline]
            pub fn new_in (len: usize, alloc: A) -> Self {
                Self::try_new_in(len, alloc).unwrap()
            }

            /// Allocates a new bitfield. All values are initialized to `false`.
            ///
            /// # Errors
            /// This method returns an error if the memory allocation fails
            #[inline]
            pub fn try_new_in (len: usize, alloc: A) -> Result<Self, AllocError> {
                let bytes = len.div_ceil(Self::BIT_SIZE);
                let bits = unsafe {
                    let uninit = Box::<[T], _>::new_zeroed_slice_in(bytes, alloc);
                    uninit.assume_init()
                };

                Ok(Self { bits, len })
            }

            pub fn get(&self, idx: usize, order: Ordering) -> Option<bool> {
                let byte = idx / Self::BIT_SIZE;
                let idx = idx % Self::BIT_SIZE;

                if !self.check_bounds(byte, idx) {
                    return None
                }

                let byte = unsafe { <[T]>::get_unchecked(&self.bits, byte) };
                let v = byte.load(order);
                let mask = T::Primitive::one() << idx;
                return Some((v & mask) != T::Primitive::zero())
            }

            #[inline]
            pub fn set (&self, v: bool, idx: usize, order: Ordering) -> Option<bool> {
                if v { return self.set_true(idx, order) }
                self.set_false(idx, order)
            }

            #[inline]
            pub fn set_true (&self, idx: usize, order: Ordering) -> Option<bool> {
                let byte = idx / Self::BIT_SIZE;
                let idx = idx % Self::BIT_SIZE;

                if !self.check_bounds(byte, idx) {
                    return None
                }

                let byte = unsafe { <[T]>::get_unchecked(&self.bits, byte) };
                let mask = T::Primitive::one() << idx;
                let prev = byte.fetch_or(mask, order);
                return Some((prev & mask) != T::Primitive::zero())
            }

            #[inline]
            pub fn set_false (&self, idx: usize, order: Ordering) -> Option<bool> {
                let byte = idx / Self::BIT_SIZE;
                let idx = idx % Self::BIT_SIZE;

                if !self.check_bounds(byte, idx) {
                    return None
                }

                let byte = unsafe { <[T]>::get_unchecked(&self.bits, byte) };
                let zero = T::Primitive::zero();
                let mask = T::Primitive::one() << idx;

                let prev = byte.fetch_and((!zero).wrapping_sub(&mask), order);
                return Some((prev & mask) != zero)
            }

            #[inline]
            fn check_bounds (&self, major: usize, minor: usize) -> bool {
                if major < self.bits.len() - 1 {
                    return minor < Self::BIT_SIZE
                }
                return minor < self.len % Self::BIT_SIZE
            }
        }
    } else {
        impl<T: AtomicInt> AtomicBitBox<T> where T::Primitive: BitFieldAble {
            const BIT_SIZE: usize = 8 * core::mem::size_of::<T>();

            pub fn get(&self, idx: usize, order: Ordering) -> Option<bool> {
                let byte = idx / Self::BIT_SIZE;
                let idx = idx % Self::BIT_SIZE;

                if !self.check_bounds(byte, idx) {
                    return None
                }

                let byte = unsafe { <[T]>::get_unchecked(&self.bits, byte) };
                let v = byte.load(order);
                let mask = T::Primitive::one() << idx;
                return Some((v & mask) != T::Primitive::zero())
            }

            #[inline]
            pub fn set (&self, v: bool, idx: usize, order: Ordering) -> Option<bool> {
                if v { return self.set_true(idx, order) }
                self.set_false(idx, order)
            }

            #[inline]
            pub fn set_true (&self, idx: usize, order: Ordering) -> Option<bool> {
                let byte = idx / Self::BIT_SIZE;
                let idx = idx % Self::BIT_SIZE;

                if !self.check_bounds(byte, idx) {
                    return None
                }

                let byte = unsafe { <[T]>::get_unchecked(&self.bits, byte) };
                let mask = T::Primitive::one() << idx;
                let prev = byte.fetch_or(mask, order);
                return Some((prev & mask) != T::Primitive::zero())
            }

            #[inline]
            pub fn set_false (&self, idx: usize, order: Ordering) -> Option<bool> {
                let byte = idx / Self::BIT_SIZE;
                let idx = idx % Self::BIT_SIZE;

                if !self.check_bounds(byte, idx) {
                    return None
                }

                let byte = unsafe { <[T]>::get_unchecked(&self.bits, byte) };
                let zero = T::Primitive::zero();
                let mask = T::Primitive::one() << idx;

                let prev = byte.fetch_and((!zero).wrapping_sub(&mask), order);
                return Some((prev & mask) != zero)
            }

            #[inline]
            fn check_bounds (&self, major: usize, minor: usize) -> bool {
                if major < self.bits.len() - 1 {
                    return minor < Self::BIT_SIZE
                }
                return minor < self.len % Self::BIT_SIZE
            }
        }
    }
}

pub trait BitFieldAble:
    Num
    + Copy
    + Zeroable
    + Eq
    + WrappingSub
    + BitAnd<Output = Self>
    + Shl<usize, Output = Self>
    + Shr<usize, Output = Self>
    + Not<Output = Self>
{
}
impl<T> BitFieldAble for T where
    T: Num
        + Copy
        + Zeroable
        + Eq
        + WrappingSub
        + BitAnd<Output = Self>
        + Shl<usize, Output = Self>
        + Shr<usize, Output = Self>
        + Not<Output = Self>
{
}
