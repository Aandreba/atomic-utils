#[allow(unused_imports)]
use core::sync::atomic::Ordering::{self, *};
use docfg::docfg;

#[allow(non_camel_case_types)]
pub type Atomic_c_char = <core::ffi::c_char as HasAtomic>::Atomic;
#[allow(non_camel_case_types)]
pub type Atomic_c_schar = <core::ffi::c_schar as HasAtomic>::Atomic;
#[allow(non_camel_case_types)]
pub type Atomic_c_uchar = <core::ffi::c_uchar as HasAtomic>::Atomic;
#[allow(non_camel_case_types)]
pub type Atomic_c_short = <core::ffi::c_short as HasAtomic>::Atomic;
#[allow(non_camel_case_types)]
pub type Atomic_c_ushort = <core::ffi::c_ushort as HasAtomic>::Atomic;
#[allow(non_camel_case_types)]
pub type Atomic_c_int = <core::ffi::c_int as HasAtomic>::Atomic;
#[allow(non_camel_case_types)]
pub type Atomic_c_uint = <core::ffi::c_uint as HasAtomic>::Atomic;
#[allow(non_camel_case_types)]
pub type Atomic_c_long = <core::ffi::c_long as HasAtomic>::Atomic;
#[allow(non_camel_case_types)]
pub type Atomic_c_ulong = <core::ffi::c_ulong as HasAtomic>::Atomic;
#[allow(non_camel_case_types)]
pub type Atomic_c_longlong = <core::ffi::c_longlong as HasAtomic>::Atomic;
#[allow(non_camel_case_types)]
pub type Atomic_c_ulonglong = <core::ffi::c_ulonglong as HasAtomic>::Atomic;
#[docfg(feature = "nightly")]
#[allow(non_camel_case_types)]
pub type Atomic_c_size_t = <core::ffi::c_size_t as HasAtomic>::Atomic;
#[docfg(feature = "nightly")]
#[allow(non_camel_case_types)]
pub type Atomic_c_ssize_t = <core::ffi::c_ssize_t as HasAtomic>::Atomic;
#[docfg(feature = "nightly")]
#[allow(non_camel_case_types)]
pub type Atomic_c_ptrdiff_t = <core::ffi::c_ptrdiff_t as HasAtomic>::Atomic;

/// A trait representing types that have an associated atomic type.
pub trait HasAtomic {
    type Atomic: Atomic<Primitive = Self>;
}

#[allow(clippy::missing_errors_doc)]
/// A trait representing atomic types.
/// # Safety
/// - `Self` must have the same size and alignment as [`Primitive`](`Atomic::Primitive`)
pub unsafe trait Atomic: Send + Sync {
    type Primitive: HasAtomic<Atomic = Self>;

    /// Creates a new atomic integer.
    fn new(v: Self::Primitive) -> Self;

    /// Returns a mutable reference to the underlying integer.
    ///
    /// This is safe because the mutable reference guarantees that no other threads are
    /// concurrently accessing the atomic data.
    fn get_mut(&mut self) -> &mut Self::Primitive;
    /// Consumes the atomic and returns the contained value.
    ///
    /// This is safe because passing `self` by value guarantees that no other threads are
    /// concurrently accessing the atomic data.
    fn into_inner(self) -> Self::Primitive;
    /// Loads a value from the atomic integer.
    ///
    /// `load` takes an [`Ordering`] argument which describes the memory ordering of this operation.
    /// Possible values are [`SeqCst`], [`Acquire`] and [`Relaxed`].
    ///
    /// # Panics
    ///
    /// Panics if `order` is [`Release`] or [`AcqRel`].
    fn load(&self, order: Ordering) -> Self::Primitive;
    /// Stores a value into the atomic integer.
    ///
    /// `store` takes an [`Ordering`] argument which describes the memory ordering of this operation.
    ///  Possible values are [`SeqCst`], [`Release`] and [`Relaxed`].
    ///
    /// # Panics
    ///
    /// Panics if `order` is [`Acquire`] or [`AcqRel`].
    fn store(&self, val: Self::Primitive, order: Ordering);
    /// Stores a value into the atomic integer, returning the previous value.
    ///
    /// `swap` takes an [`Ordering`] argument which describes the memory ordering
    /// of this operation. All ordering modes are possible. Note that using
    /// [`Acquire`] makes the store part of this operation [`Relaxed`], and
    /// using [`Release`] makes the load part [`Relaxed`].
    fn swap(&self, val: Self::Primitive, order: Ordering) -> Self::Primitive;

    /// Stores a value into the atomic integer if the current value is the same as
    /// the `current` value.
    ///
    /// The return value is a result indicating whether the new value was written and
    /// containing the previous value. On success this value is guaranteed to be equal to
    /// `current`.
    ///
    /// `compare_exchange` takes two [`Ordering`] arguments to describe the memory
    /// ordering of this operation. `success` describes the required ordering for the
    /// read-modify-write operation that takes place if the comparison with `current` succeeds.
    /// `failure` describes the required ordering for the load operation that takes place when
    /// the comparison fails. Using [`Acquire`] as success ordering makes the store part
    /// of this operation [`Relaxed`], and using [`Release`] makes the successful load
    /// [`Relaxed`]. The failure ordering can only be [`SeqCst`], [`Acquire`] or [`Relaxed`].
    fn compare_exchange(
        &self,
        current: Self::Primitive,
        new: Self::Primitive,
        success: Ordering,
        failure: Ordering,
    ) -> Result<Self::Primitive, Self::Primitive>;

    /// Stores a value into the atomic integer if the current value is the same as
    /// the `current` value.
    ///
    /// Unlike [`compare_exchange`](Atomic::compare_exchange), this function is allowed to spuriously fail even
    /// when the comparison succeeds, which can result in more efficient code on some
    /// platforms. The return value is a result indicating whether the new value was
    /// written and containing the previous value.
    ///
    /// `compare_exchange_weak` takes two [`Ordering`] arguments to describe the memory
    /// ordering of this operation. `success` describes the required ordering for the
    /// read-modify-write operation that takes place if the comparison with `current` succeeds.
    /// `failure` describes the required ordering for the load operation that takes place when
    /// the comparison fails. Using [`Acquire`] as success ordering makes the store part
    /// of this operation [`Relaxed`], and using [`Release`] makes the successful load
    /// [`Relaxed`]. The failure ordering can only be [`SeqCst`], [`Acquire`] or [`Relaxed`].
    fn compare_exchange_weak(
        &self,
        current: Self::Primitive,
        new: Self::Primitive,
        success: Ordering,
        failure: Ordering,
    ) -> Result<Self::Primitive, Self::Primitive>;

    /// Fetches the value, and applies a function to it that returns an optional
    /// new value. Returns a `Result` of `Ok(previous_value)` if the function returned `Some(_)`, else
    /// `Err(previous_value)`.
    ///
    /// Note: This may call the function multiple times if the value has been changed from other threads in
    /// the meantime, as long as the function returns `Some(_)`, but the function will have been applied
    /// only once to the stored value.
    ///
    /// `fetch_update` takes two [`Ordering`] arguments to describe the memory ordering of this operation.
    /// The first describes the required ordering for when the operation finally succeeds while the second
    /// describes the required ordering for loads. These correspond to the success and failure orderings of
    /// [`compare_exchange`](Atomic::compare_exchange) respectively.
    ///
    /// Using [`Acquire`] as success ordering makes the store part
    /// of this operation [`Relaxed`], and using [`Release`] makes the final successful load
    /// [`Relaxed`]. The (failed) load ordering can only be [`SeqCst`], [`Acquire`] or [`Relaxed`].
    ///
    /// # Considerations
    ///
    /// This method is not magic; it is not provided by the hardware.
    /// It is implemented in terms of [`compare_exchange_weak`](Atomic::compare_exchange_weak),
    /// and suffers from the same drawbacks.
    /// In particular, this method will not circumvent the [ABA Problem].
    ///
    /// [ABA Problem]: https://en.wikipedia.org/wiki/ABA_problem
    fn fetch_update<F: FnMut(Self::Primitive) -> Option<Self::Primitive>>(
        &self,
        set_order: Ordering,
        fetch_ordering: Ordering,
        f: F,
    ) -> Result<Self::Primitive, Self::Primitive>;
}

/// A trait representing atomic types that can be constructed in a "const" context.
#[cfg_attr(docsrs, doc(cfg(feature = "const")))]
#[cfg(feature = "const")]
#[const_trait]
pub trait AtomicConstNew: Atomic {
    fn new(v: Self::Primitive) -> Self;
}

/// A trait representing atomic types that support addition operations.
pub trait AtomicAdd<T = <Self as Atomic>::Primitive>: Atomic {
    /// Adds to the current value, returning the previous value.
    ///
    /// This operation wraps around on overflow.
    ///
    /// `fetch_add` takes an [`Ordering`] argument which describes the memory ordering
    /// of this operation. All ordering modes are possible. Note that using
    /// [`Acquire`] makes the store part of this operation [`Relaxed`], and
    /// using [`Release`] makes the load part [`Relaxed`].
    fn fetch_add(&self, val: T, order: Ordering) -> Self::Primitive;
}

/// A trait representing atomic types that support subtraction operations.
pub trait AtomicSub<T = <Self as Atomic>::Primitive>: Atomic {
    /// Subtracts from the current value, returning the previous value.
    ///
    /// This operation wraps around on overflow.
    ///
    /// `fetch_sub` takes an [`Ordering`] argument which describes the memory ordering
    /// of this operation. All ordering modes are possible. Note that using
    /// [`Acquire`] makes the store part of this operation [`Relaxed`], and
    /// using [`Release`] makes the load part [`Relaxed`].
    fn fetch_sub(&self, val: T, order: Ordering) -> Self::Primitive;
}

/// A trait representing atomic types that support subtraction operations.
pub trait AtomicBitAnd<T = <Self as Atomic>::Primitive>: Atomic {
    /// Bitwise "and" with the current value.
    ///
    /// Performs a bitwise "and" operation on the current value and the argument `val`, and
    /// sets the new value to the result.
    ///
    /// Returns the previous value.
    ///
    /// `fetch_and` takes an [`Ordering`] argument which describes the memory ordering
    /// of this operation. All ordering modes are possible. Note that using
    /// [`Acquire`] makes the store part of this operation [`Relaxed`], and
    /// using [`Release`] makes the load part [`Relaxed`].
    fn fetch_and(&self, val: T, order: Ordering) -> Self::Primitive;
    /// Bitwise "nand" with the current value.
    ///
    /// Performs a bitwise "nand" operation on the current value and the argument `val`, and
    /// sets the new value to the result.
    ///
    /// Returns the previous value.
    ///
    /// `fetch_nand` takes an [`Ordering`] argument which describes the memory ordering
    /// of this operation. All ordering modes are possible. Note that using
    /// [`Acquire`] makes the store part of this operation [`Relaxed`], and
    /// using [`Release`] makes the load part [`Relaxed`].
    fn fetch_nand(&self, val: T, order: Ordering) -> Self::Primitive;
}

/// A trait representing atomic types that support bitwise OR operations.
pub trait AtomicBitOr<T = <Self as Atomic>::Primitive>: Atomic {
    /// Bitwise "or" with the current value.
    ///
    /// Performs a bitwise "or" operation on the current value and the argument `val`, and
    /// sets the new value to the result.
    ///
    /// Returns the previous value.
    ///
    /// `fetch_or` takes an [`Ordering`] argument which describes the memory ordering
    /// of this operation. All ordering modes are possible. Note that using
    /// [`Acquire`] makes the store part of this operation [`Relaxed`], and
    /// using [`Release`] makes the load part [`Relaxed`].
    fn fetch_or(&self, val: T, order: Ordering) -> Self::Primitive;
}

/// A trait representing atomic types that support bitwise XOR operations.
pub trait AtomicBitXor<T = <Self as Atomic>::Primitive>: Atomic {
    /// Bitwise "xor" with the current value.
    ///
    /// Performs a bitwise "xor" operation on the current value and the argument `val`, and
    /// sets the new value to the result.
    ///
    /// Returns the previous value.
    ///
    /// `fetch_xor` takes an [`Ordering`] argument which describes the memory ordering
    /// of this operation. All ordering modes are possible. Note that using
    /// [`Acquire`] makes the store part of this operation [`Relaxed`], and
    /// using [`Release`] makes the load part [`Relaxed`].
    fn fetch_xor(&self, val: T, order: Ordering) -> Self::Primitive;
}

/// A trait representing atomic types that support minimum operations.
pub trait AtomicMin<T = <Self as Atomic>::Primitive>: Atomic {
    /// Minimum with the current value.
    ///
    /// Finds the minimum of the current value and the argument `val`, and
    /// sets the new value to the result.
    ///
    /// Returns the previous value.
    ///
    /// `fetch_min` takes an [`Ordering`] argument which describes the memory ordering
    /// of this operation. All ordering modes are possible. Note that using
    /// [`Acquire`] makes the store part of this operation [`Relaxed`], and
    /// using [`Release`] makes the load part [`Relaxed`].
    fn fetch_min(&self, val: T, order: Ordering) -> Self::Primitive;
}

/// A trait representing atomic types that support maximum operations.
pub trait AtomicMax<T = <Self as Atomic>::Primitive>: Atomic {
    /// Maximum with the current value.
    ///
    /// Finds the maximum of the current value and the argument `val`, and
    /// sets the new value to the result.
    ///
    /// Returns the previous value.
    ///
    /// `fetch_max` takes an [`Ordering`] argument which describes the memory ordering
    /// of this operation. All ordering modes are possible. Note that using
    /// [`Acquire`] makes the store part of this operation [`Relaxed`], and
    /// using [`Release`] makes the load part [`Relaxed`].
    fn fetch_max(&self, val: T, order: Ordering) -> Self::Primitive;
}

/* MARKER TRAITS */

/// A marker trait representing types that have an associated atomic integer type.
pub trait HasAtomicInt: HasAtomic {
    type AtomicInt: AtomicInt<Primitive = Self>;
}
/// A marker trait representing atomic types that support numerical operations.
pub trait AtomicNumOps<T = <Self as Atomic>::Primitive>:
    Atomic + AtomicAdd<T> + AtomicSub<T>
{
}
/// A marker trait representing atomic types that support bitwise operations.
pub trait AtomicBitOps<T = <Self as Atomic>::Primitive>:
    Atomic + AtomicBitAnd<T> + AtomicBitOr<T> + AtomicBitXor<T>
{
}
/// A marker trait representing atomic types that support ordering operations.
pub trait AtomicOrd<T = <Self as Atomic>::Primitive>: Atomic + AtomicMin<T> + AtomicMax<T> {}
/// A marker trait representing atomic types that support numerical and ordering operations.
pub trait AtomicNum: AtomicNumOps + AtomicOrd {}
/// A marker trait representing atomic types that support numerical and ordering operations.
pub trait AtomicInt: AtomicNum + AtomicBitOps {}

impl<T: HasAtomic> HasAtomicInt for T
where
    T::Atomic: AtomicInt<Primitive = T>,
{
    type AtomicInt = <T as HasAtomic>::Atomic;
}
impl<T, U> AtomicNumOps<T> for U where U: Atomic + AtomicAdd<T> + AtomicSub<T> {}
impl<T, U> AtomicBitOps<T> for U where U: Atomic + AtomicBitAnd<T> + AtomicBitOr<T> + AtomicBitXor<T>
{}
impl<T, U> AtomicOrd<T> for U where U: Atomic + AtomicMin<T> + AtomicMax<T> {}
impl<T> AtomicNum for T where T: AtomicNumOps + AtomicOrd {}
impl<T> AtomicInt for T where T: AtomicNum + AtomicBitOps {}

// IMPLEMENTATION

macro_rules! impl_atomic {
    ($($len:literal: $prim:ty => $atomic:ty),+) => {
        $(
            #[docfg(target_has_atomic = $len)]
            impl HasAtomic for $prim {
                type Atomic = $atomic;
            }

            #[docfg(target_has_atomic = $len)]
            unsafe impl Atomic for $atomic {
                type Primitive = $prim;

                #[inline]
                fn new (v: Self::Primitive) -> Self {
                    <$atomic>::new(v)
                }

                #[inline]
                fn get_mut (&mut self) -> &mut Self::Primitive {
                    <$atomic>::get_mut(self)
                }

                #[inline]
                fn into_inner (self) -> Self::Primitive {
                    <$atomic>::into_inner(self)
                }

                #[inline]
                fn load (&self, order: Ordering) -> Self::Primitive {
                    <$atomic>::load(self, order)
                }

                #[inline]
                fn store (&self, val: Self::Primitive, order: Ordering) {
                    <$atomic>::store(self, val, order)
                }

                #[inline]
                fn swap (&self, val: Self::Primitive, order: Ordering) -> Self::Primitive {
                    <$atomic>::swap(self, val, order)
                }

                #[inline]
                fn compare_exchange (&self, current: Self::Primitive, new: Self::Primitive, success: Ordering, failure: Ordering) -> Result<Self::Primitive, Self::Primitive> {
                    <$atomic>::compare_exchange(self, current, new, success, failure)
                }

                #[inline]
                fn compare_exchange_weak (&self, current: Self::Primitive, new: Self::Primitive, success: Ordering, failure: Ordering) -> Result<Self::Primitive, Self::Primitive> {
                    <$atomic>::compare_exchange_weak(self, current, new, success, failure)
                }

                #[inline]
                fn fetch_update<F: FnMut(Self::Primitive) -> Option<Self::Primitive>> (&self, set_order: Ordering, fetch_ordering: Ordering, f: F) -> Result<Self::Primitive, Self::Primitive> {
                    <$atomic>::fetch_update(self, set_order, fetch_ordering, f)
                }
            }

            cfg_if::cfg_if! {
                if #[cfg(feature = "const")] {
                    #[cfg_attr(docsrs, doc(cfg(feature = "const")))]
                    impl const AtomicConstNew for $atomic {
                        #[inline]
                        fn new (v: Self::Primitive) -> Self {
                            <$atomic>::new(v)
                        }
                    }
                }
            }
        )+
    };
}

macro_rules! impl_int {
    ($($len:literal: ($int:ty, $uint:ty) => ($iatomic:ty, $uatomic:ty)),+) => {
        $(
            impl_int!($len: $int => $iatomic);
            impl_int!($len: $uint => $uatomic);
        )+
    };

    ($($len:literal: $prim:ty => $atomic:ty),+) => {
        $(
            impl_atomic!($len: $prim => $atomic);

            #[docfg(target_has_atomic = $len)]
            impl AtomicAdd for $atomic {
                #[inline]
                fn fetch_add(&self, val: $prim, order: Ordering) -> $prim {
                    <$atomic>::fetch_add(self, val, order)
                }
            }

            #[docfg(target_has_atomic = $len)]
            impl AtomicSub for $atomic {
                #[inline]
                fn fetch_sub(&self, val: $prim, order: Ordering) -> $prim {
                    <$atomic>::fetch_sub(self, val, order)
                }
            }

            #[docfg(target_has_atomic = $len)]
            impl AtomicBitAnd for $atomic {
                #[inline]
                fn fetch_and(&self, val: $prim, order: Ordering) -> $prim {
                    <$atomic>::fetch_and(self, val, order)
                }

                #[inline]
                fn fetch_nand(&self, val: $prim, order: Ordering) -> $prim {
                    <$atomic>::fetch_nand(self, val, order)
                }
            }

            #[docfg(target_has_atomic = $len)]
            impl AtomicBitOr for $atomic {
                #[inline]
                fn fetch_or(&self, val: $prim, order: Ordering) -> $prim {
                    <$atomic>::fetch_or(self, val, order)
                }
            }

            #[docfg(target_has_atomic = $len)]
            impl AtomicBitXor for $atomic {
                #[inline]
                fn fetch_xor(&self, val: $prim, order: Ordering) -> $prim {
                    <$atomic>::fetch_xor(self, val, order)
                }
            }

            #[docfg(target_has_atomic = $len)]
            impl AtomicMin for $atomic {
                #[inline]
                fn fetch_min(&self, val: $prim, order: Ordering) -> $prim {
                    <$atomic>::fetch_min(self, val, order)
                }
            }

            #[docfg(target_has_atomic = $len)]
            impl AtomicMax for $atomic {
                #[inline]
                fn fetch_max(&self, val: $prim, order: Ordering) -> $prim {
                    <$atomic>::fetch_max(self, val, order)
                }
            }
        )+
    };
}

impl_int! {
    "8": (u8, i8) => (core::sync::atomic::AtomicU8, core::sync::atomic::AtomicI8),
    "16": (u16, i16) => (core::sync::atomic::AtomicU16, core::sync::atomic::AtomicI16),
    "32": (u32, i32) => (core::sync::atomic::AtomicU32, core::sync::atomic::AtomicI32),
    "64": (u64, i64) => (core::sync::atomic::AtomicU64, core::sync::atomic::AtomicI64),
    "ptr": (usize, isize) => (core::sync::atomic::AtomicUsize, core::sync::atomic::AtomicIsize)
    //"128": (u128, i128) => (core::sync::atomic::AtomicU128, core::sync::atomic::AtomicI128)
}

impl_atomic! {
    "8": bool => core::sync::atomic::AtomicBool
}

#[docfg(target_has_atomic = "ptr")]
impl<T> HasAtomic for *mut T {
    type Atomic = core::sync::atomic::AtomicPtr<T>;
}

#[docfg(target_has_atomic = "ptr")]
unsafe impl<T> Atomic for core::sync::atomic::AtomicPtr<T> {
    type Primitive = *mut T;

    #[inline]
    fn new(v: Self::Primitive) -> Self {
        core::sync::atomic::AtomicPtr::new(v)
    }

    #[inline]
    fn get_mut(&mut self) -> &mut Self::Primitive {
        core::sync::atomic::AtomicPtr::get_mut(self)
    }

    #[inline]
    fn into_inner(self) -> Self::Primitive {
        core::sync::atomic::AtomicPtr::into_inner(self)
    }

    #[inline]
    fn load(&self, order: Ordering) -> Self::Primitive {
        core::sync::atomic::AtomicPtr::load(self, order)
    }

    #[inline]
    fn store(&self, val: Self::Primitive, order: Ordering) {
        core::sync::atomic::AtomicPtr::store(self, val, order)
    }

    #[inline]
    fn swap(&self, val: Self::Primitive, order: Ordering) -> Self::Primitive {
        core::sync::atomic::AtomicPtr::swap(self, val, order)
    }

    #[inline]
    fn compare_exchange(
        &self,
        current: Self::Primitive,
        new: Self::Primitive,
        success: Ordering,
        failure: Ordering,
    ) -> Result<Self::Primitive, Self::Primitive> {
        core::sync::atomic::AtomicPtr::compare_exchange(self, current, new, success, failure)
    }

    #[inline]
    fn compare_exchange_weak(
        &self,
        current: Self::Primitive,
        new: Self::Primitive,
        success: Ordering,
        failure: Ordering,
    ) -> Result<Self::Primitive, Self::Primitive> {
        core::sync::atomic::AtomicPtr::compare_exchange_weak(self, current, new, success, failure)
    }

    #[inline]
    fn fetch_update<F: FnMut(Self::Primitive) -> Option<Self::Primitive>>(
        &self,
        set_order: Ordering,
        fetch_ordering: Ordering,
        f: F,
    ) -> Result<Self::Primitive, Self::Primitive> {
        core::sync::atomic::AtomicPtr::fetch_update(self, set_order, fetch_ordering, f)
    }
}
