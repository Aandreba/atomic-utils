use crate::traits::{Atomic, AtomicBitAnd, AtomicBitOr, HasAtomicInt};
use crate::AllocError;
use crate::{div_ceil, InnerFlag};
use alloc::boxed::Box;
use bytemuck::Zeroable;
use core::{
    ops::{BitAnd, Not, Shl, Shr},
    sync::atomic::Ordering,
};
use num_traits::Num;
#[cfg(feature = "alloc_api")]
use {alloc::alloc::Global, core::alloc::*};

/// An atomic bitfield with a static size, stored in a boxed slice.
///
/// This struct provides methods for working with atomic bitfields, allowing
/// concurrent access and manipulation of individual bits. It is particularly
/// useful when you need to store a large number of boolean flags and want to
/// minimize memory usage.
///
/// # Example
///
/// ```
/// use utils_atomics::{AtomicBitBox};
/// use core::sync::atomic::Ordering;
///
/// let bit_box = AtomicBitBox::<u8>::new(10);
/// assert_eq!(bit_box.get(3, Ordering::Relaxed), Some(false));
/// bit_box.set(3, Ordering::Relaxed);
/// assert_eq!(bit_box.get(3, Ordering::Relaxed), Some(true));
/// ```
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub struct AtomicBitBox<
    T: HasAtomicInt = InnerFlag,
    #[cfg(feature = "alloc_api")] A: Allocator = Global,
> {
    #[cfg(feature = "alloc_api")]
    bits: Box<[T::AtomicInt], A>,
    #[cfg(not(feature = "alloc_api"))]
    bits: Box<[T::AtomicInt]>,
    len: usize,
}

impl<T: HasAtomicInt> AtomicBitBox<T>
where
    T: BitFieldAble,
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
                    let uninit = Box::<[T::AtomicInt]>::new_zeroed_slice(count);
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
        impl<T: HasAtomicInt, A: Allocator> AtomicBitBox<T, A> where T: BitFieldAble {
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
                    let uninit = Box::<[T::AtomicInt], _>::new_zeroed_slice_in(bytes, alloc);
                    uninit.assume_init()
                };

                Ok(Self { bits, len })
            }

            /// Returns the value of the bit at the specified index, or `None` if the index is out of bounds.
            ///
            /// `order` defines the memory ordering for this operation.
            pub fn get(&self, idx: usize, order: Ordering) -> Option<bool> {
                let byte = idx / Self::BIT_SIZE;
                let idx = idx % Self::BIT_SIZE;

                if !self.check_bounds(byte, idx) {
                    return None
                }

                let byte = unsafe { <[T::AtomicInt]>::get_unchecked(&self.bits, byte) };
                let v = byte.load(order);
                let mask = T::one() << idx;
                return Some((v & mask) != T::zero())
            }

            /// Sets the value of the bit at the specified index and returns the previous value, or `None` if the index is out of bounds.
            ///
            /// `order` defines the memory ordering for this operation.
            #[inline]
            pub fn set_value (&self, v: bool, idx: usize, order: Ordering) -> Option<bool> {
                if v { return self.set(idx, order) }
                self.clear(idx, order)
            }

            /// Sets the bit at the specified index to `true` and returns the previous value, or `None` if the index is out of bounds.
            ///
            /// `order` defines the memory ordering for this operation.
            #[inline]
            pub fn set (&self, idx: usize, order: Ordering) -> Option<bool> {
                let byte = idx / Self::BIT_SIZE;
                let idx = idx % Self::BIT_SIZE;

                if !self.check_bounds(byte, idx) {
                    return None
                }

                let byte = unsafe { <[T::AtomicInt]>::get_unchecked(&self.bits, byte) };
                let mask = T::one() << idx;
                let prev = byte.fetch_or(mask, order);
                return Some((prev & mask) != T::zero())
            }

            /// Sets the bit at the specified index to `false` and returns the previous value, or `None` if the index is out of bounds.
            ///
            /// `order` defines the memory ordering for this operation.
            #[inline]
            pub fn clear (&self, idx: usize, order: Ordering) -> Option<bool> {
                let byte = idx / Self::BIT_SIZE;
                let idx = idx % Self::BIT_SIZE;

                if !self.check_bounds(byte, idx) {
                    return None
                }

                let byte = unsafe { <[T::AtomicInt]>::get_unchecked(&self.bits, byte) };
                let mask = T::one() << idx;
                let prev = byte.fetch_and(!mask, order);
                return Some((prev & mask) != T::zero())
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
        impl<T: HasAtomicInt> AtomicBitBox<T> where T: BitFieldAble {
            const BIT_SIZE: usize = 8 * core::mem::size_of::<T>();

            /// Returns the value of the bit at the specified index, or `None` if the index is out of bounds.
            ///
            /// `order` defines the memory ordering for this operation.
            pub fn get(&self, idx: usize, order: Ordering) -> Option<bool> {
                let byte = idx / Self::BIT_SIZE;
                let idx = idx % Self::BIT_SIZE;

                if !self.check_bounds(byte, idx) {
                    return None
                }

                let byte = unsafe { <[T::AtomicInt]>::get_unchecked(&self.bits, byte) };
                let v = byte.load(order);
                let mask = T::one() << idx;
                return Some((v & mask) != T::zero())
            }

            /// Sets the value of the bit at the specified index and returns the previous value, or `None` if the index is out of bounds.
            ///
            /// `order` defines the memory ordering for this operation.
            #[inline]
            pub fn set_value (&self, v: bool, idx: usize, order: Ordering) -> Option<bool> {
                if v { return self.set(idx, order) }
                self.clear(idx, order)
            }

            /// Sets the bit at the specified index to `true` and returns the previous value, or `None` if the index is out of bounds.
            ///
            /// `order` defines the memory ordering for this operation.
            #[inline]
            pub fn set (&self, idx: usize, order: Ordering) -> Option<bool> {
                let byte = idx / Self::BIT_SIZE;
                let idx = idx % Self::BIT_SIZE;

                if !self.check_bounds(byte, idx) {
                    return None
                }

                let byte = unsafe { <[T::AtomicInt]>::get_unchecked(&self.bits, byte) };
                let mask = T::one() << idx;
                let prev = byte.fetch_or(mask, order);
                return Some((prev & mask) != T::zero())
            }

            /// Sets the bit at the specified index to `false` and returns the previous value, or `None` if the index is out of bounds.
            ///
            /// `order` defines the memory ordering for this operation.
            #[inline]
            pub fn clear (&self, idx: usize, order: Ordering) -> Option<bool> {
                let byte = idx / Self::BIT_SIZE;
                let idx = idx % Self::BIT_SIZE;

                if !self.check_bounds(byte, idx) {
                    return None
                }

                let byte = unsafe { <[T::AtomicInt]>::get_unchecked(&self.bits, byte) };
                let mask = T::one() << idx;
                let prev = byte.fetch_and(!mask, order);
                return Some((prev & mask) != T::zero())
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
        + BitAnd<Output = Self>
        + Shl<usize, Output = Self>
        + Shr<usize, Output = Self>
        + Not<Output = Self>
{
}

// Thanks ChatGPT!
#[cfg(test)]
mod tests {
    use core::sync::atomic::Ordering;

    pub type AtomicBitBox = super::AtomicBitBox<u16>;

    #[test]
    fn new_bitbox() {
        let bitbox = AtomicBitBox::new(10);
        for i in 0..10 {
            assert_eq!(bitbox.get(i, Ordering::SeqCst), Some(false));
        }
    }

    #[test]
    fn set_and_get() {
        let bitbox = AtomicBitBox::new(10);

        bitbox.set(2, Ordering::SeqCst);
        bitbox.set(7, Ordering::SeqCst);

        for i in 0..10 {
            let expected = (i == 2) || (i == 7);
            assert_eq!(bitbox.get(i, Ordering::SeqCst), Some(expected));
        }
    }

    #[test]
    fn set_false_and_get() {
        let bitbox = AtomicBitBox::new(10);

        bitbox.set(2, Ordering::SeqCst);
        bitbox.set(7, Ordering::SeqCst);

        bitbox.clear(2, Ordering::SeqCst);

        for i in 0..10 {
            let expected = i == 7;
            assert_eq!(bitbox.get(i, Ordering::SeqCst), Some(expected));
        }
    }

    #[test]
    fn out_of_bounds() {
        let bitbox = AtomicBitBox::new(10);
        assert_eq!(bitbox.get(11, Ordering::SeqCst), None);
        assert_eq!(bitbox.set(11, Ordering::SeqCst), None);
        assert_eq!(bitbox.clear(11, Ordering::SeqCst), None);
    }

    #[cfg(feature = "alloc_api")]
    mod custom_allocator {
        use core::sync::atomic::Ordering;
        use std::alloc::System;

        pub type AtomicBitBox = super::super::AtomicBitBox<u16, System>;

        #[test]
        fn new_bitbox() {
            let bitbox = AtomicBitBox::new_in(10, System);
            for i in 0..10 {
                assert_eq!(bitbox.get(i, Ordering::SeqCst), Some(false));
            }
        }

        #[test]
        fn set_and_get() {
            let bitbox = AtomicBitBox::new_in(10, System);

            bitbox.set(2, Ordering::SeqCst);
            bitbox.set(7, Ordering::SeqCst);

            for i in 0..10 {
                let expected = (i == 2) || (i == 7);
                assert_eq!(bitbox.get(i, Ordering::SeqCst), Some(expected));
            }
        }

        #[test]
        fn set_false_and_get() {
            let bitbox = AtomicBitBox::new_in(10, System);

            bitbox.set(2, Ordering::SeqCst);
            bitbox.set(7, Ordering::SeqCst);

            bitbox.clear(2, Ordering::SeqCst);

            for i in 0..10 {
                let expected = i == 7;
                assert_eq!(bitbox.get(i, Ordering::SeqCst), Some(expected));
            }
        }

        #[test]
        fn out_of_bounds() {
            let bitbox = AtomicBitBox::new_in(10, System);
            assert_eq!(bitbox.get(11, Ordering::SeqCst), None);
            assert_eq!(bitbox.set(11, Ordering::SeqCst), None);
            assert_eq!(bitbox.clear(11, Ordering::SeqCst), None);
        }
    }
}
