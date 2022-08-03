use core::sync::atomic::{Ordering};

pub trait HasAtomic {
    type Atomic: Atomic<Primitive = Self>;
}

/// # Safety
/// - `Self` must have the same size and alignment as [`Primitive`](`Atomic::Primitive`)
pub unsafe trait Atomic: Send + Sync {
    type Primitive: HasAtomic<Atomic = Self>;
    
    fn new (v: Self::Primitive) -> Self;
    fn get_mut (&mut self) -> &mut Self::Primitive;
    fn into_inner (self) -> Self::Primitive;
    fn load (&self, order: Ordering) -> Self::Primitive;
    fn store (&self, val: Self::Primitive, order: Ordering);
    fn swap (&self, val: Self::Primitive, order: Ordering) -> Self::Primitive;
    fn compare_exchange (&self, current: Self::Primitive, new: Self::Primitive, success: Ordering, failure: Ordering) -> Result<Self::Primitive, Self::Primitive>;
    fn compare_exchange_weak (&self, current: Self::Primitive, new: Self::Primitive, success: Ordering, failure: Ordering) -> Result<Self::Primitive, Self::Primitive>;
    fn fetch_update<F: FnMut(Self::Primitive) -> Option<Self::Primitive>> (&self, set_order: Ordering, fetch_ordering: Ordering, f: F) -> Result<Self::Primitive, Self::Primitive>;
}

#[cfg_attr(docsrs, doc(cfg(feature = "const")))]
#[cfg(feature = "const")]
pub trait AtomicConstNew: Atomic {
    fn new (v: Self::Primitive) -> Self;
}

pub trait AtomicAdd<T = <Self as Atomic>::Primitive>: Atomic {
    fn fetch_add(&self, val: T, order: Ordering) -> Self::Primitive;
}

pub trait AtomicSub<T = <Self as Atomic>::Primitive>: Atomic {
    fn fetch_sub(&self, val: T, order: Ordering) -> Self::Primitive;
}

pub trait AtomicBitAnd<T = <Self as Atomic>::Primitive>: Atomic {
    fn fetch_and(&self, val: T, order: Ordering) -> Self::Primitive;
    fn fetch_nand(&self, val: T, order: Ordering) -> Self::Primitive;
}

pub trait AtomicBitOr<T = <Self as Atomic>::Primitive>: Atomic {
    fn fetch_or(&self, val: T, order: Ordering) -> Self::Primitive;
}


pub trait AtomicBitXor<T = <Self as Atomic>::Primitive>: Atomic {
    fn fetch_xor(&self, val: T, order: Ordering) -> Self::Primitive;
}

pub trait AtomicMin<T = <Self as Atomic>::Primitive>: Atomic {
    fn fetch_min(&self, val: T, order: Ordering) -> Self::Primitive;
}

pub trait AtomicMax<T = <Self as Atomic>::Primitive>: Atomic {
    fn fetch_max(&self, val: T, order: Ordering) -> Self::Primitive;
}

// MARKER TRAITS
pub trait AtomicNumOps<T = <Self as Atomic>::Primitive>: Atomic + AtomicAdd<T> + AtomicSub<T> {}
pub trait AtomicBitOps<T = <Self as Atomic>::Primitive>: Atomic + AtomicBitAnd<T> + AtomicBitOr<T> + AtomicBitXor<T> {}
pub trait AtomicOrd<T = <Self as Atomic>::Primitive>: Atomic + AtomicMin<T> + AtomicMax<T> {}
pub trait AtomicNum: AtomicNumOps + AtomicOrd {}
pub trait AtomicInt: AtomicNum + AtomicBitOps {}

impl<T, U> AtomicNumOps<T> for U where U: Atomic + AtomicAdd<T> + AtomicSub<T> {}
impl<T, U> AtomicBitOps<T> for U where U: Atomic + AtomicBitAnd<T> + AtomicBitOr<T> + AtomicBitXor<T> {}
impl<T, U> AtomicOrd<T> for U where U: Atomic + AtomicMin<T> + AtomicMax<T> {}
impl<T> AtomicNum for T where T: AtomicNumOps + AtomicOrd {}
impl<T> AtomicInt for T where T: AtomicNum + AtomicBitOps {}

// IMPLEMENTATION

macro_rules! impl_atomic {
    ($($len:literal: $prim:ty => $atomic:ty),+) => {
        $(
            #[cfg(target_has_atomic = $len)]
            impl HasAtomic for $prim {
                type Atomic = $atomic;
            }

            #[cfg(target_has_atomic = $len)]
            unsafe impl Atomic for $atomic {
                type Primitive = $prim;

                #[inline(always)]
                fn new (v: Self::Primitive) -> Self {
                    <$atomic>::new(v)
                }

                #[inline(always)]
                fn get_mut (&mut self) -> &mut Self::Primitive {
                    <$atomic>::get_mut(self)
                }

                #[inline(always)]
                fn into_inner (self) -> Self::Primitive {
                    <$atomic>::into_inner(self)    
                }

                #[inline(always)]
                fn load (&self, order: Ordering) -> Self::Primitive {
                    <$atomic>::load(self, order)
                }

                #[inline(always)]
                fn store (&self, val: Self::Primitive, order: Ordering) {
                    <$atomic>::store(self, val, order)
                }

                #[inline(always)]
                fn swap (&self, val: Self::Primitive, order: Ordering) -> Self::Primitive {
                    <$atomic>::swap(self, val, order)
                }

                #[inline(always)]
                fn compare_exchange (&self, current: Self::Primitive, new: Self::Primitive, success: Ordering, failure: Ordering) -> Result<Self::Primitive, Self::Primitive> {
                    <$atomic>::compare_exchange(self, current, new, success, failure)
                }

                #[inline(always)]
                fn compare_exchange_weak (&self, current: Self::Primitive, new: Self::Primitive, success: Ordering, failure: Ordering) -> Result<Self::Primitive, Self::Primitive> {
                    <$atomic>::compare_exchange_weak(self, current, new, success, failure)
                }

                #[inline(always)]
                fn fetch_update<F: FnMut(Self::Primitive) -> Option<Self::Primitive>> (&self, set_order: Ordering, fetch_ordering: Ordering, f: F) -> Result<Self::Primitive, Self::Primitive> {
                    <$atomic>::fetch_update(self, set_order, fetch_ordering, f)
                }
            }

            cfg_if::cfg_if! {
                if #[cfg(feature = "const")] {
                    #[cfg_attr(docsrs, doc(cfg(feature = "const")))]
                    impl const AtomicConstNew for $atomic {
                        #[inline(always)]
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

            #[cfg(target_has_atomic = $len)]
            impl AtomicAdd for $atomic {
                #[inline(always)]
                fn fetch_add(&self, val: $prim, order: Ordering) -> $prim {
                    <$atomic>::fetch_add(self, val, order)
                }
            }

            #[cfg(target_has_atomic = $len)]
            impl AtomicSub for $atomic {
                #[inline(always)]
                fn fetch_sub(&self, val: $prim, order: Ordering) -> $prim {
                    <$atomic>::fetch_sub(self, val, order)
                }
            }

            #[cfg(target_has_atomic = $len)]
            impl AtomicBitAnd for $atomic {
                #[inline(always)]
                fn fetch_and(&self, val: $prim, order: Ordering) -> $prim {
                    <$atomic>::fetch_and(self, val, order)
                }

                #[inline(always)]
                fn fetch_nand(&self, val: $prim, order: Ordering) -> $prim {
                    <$atomic>::fetch_nand(self, val, order)
                }
            }

            #[cfg(target_has_atomic = $len)]
            impl AtomicBitOr for $atomic {
                #[inline(always)]
                fn fetch_or(&self, val: $prim, order: Ordering) -> $prim {
                    <$atomic>::fetch_or(self, val, order)
                }
            }

            #[cfg(target_has_atomic = $len)]
            impl AtomicBitXor for $atomic {
                #[inline(always)]
                fn fetch_xor(&self, val: $prim, order: Ordering) -> $prim {
                    <$atomic>::fetch_xor(self, val, order)
                }
            }

            #[cfg(target_has_atomic = $len)]
            impl AtomicMin for $atomic {
                #[inline(always)]
                fn fetch_min(&self, val: $prim, order: Ordering) -> $prim {
                    <$atomic>::fetch_min(self, val, order)
                }
            }

            #[cfg(target_has_atomic = $len)]
            impl AtomicMax for $atomic {
                #[inline(always)]
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
}

#[cfg(has_u128)]
impl_int! {
    "128": (u128, i128) => (core::sync::atomic::AtomicU128, core::sync::atomic::AtomicI128)
}

impl_atomic! {
    "8": bool => core::sync::atomic::AtomicBool
}

#[cfg(target_has_atomic = "ptr")]
impl<T> HasAtomic for *mut T {
    type Atomic = core::sync::atomic::AtomicPtr<T>;
}

#[cfg(target_has_atomic = "ptr")]
unsafe impl<T> Atomic for core::sync::atomic::AtomicPtr<T> {
    type Primitive = *mut T;

    #[inline(always)]
    fn new (v: Self::Primitive) -> Self {
        core::sync::atomic::AtomicPtr::new(v)
    }

    #[inline(always)]
    fn get_mut (&mut self) -> &mut Self::Primitive {
        core::sync::atomic::AtomicPtr::get_mut(self)
    }

    #[inline(always)]
    fn into_inner (self) -> Self::Primitive {
        core::sync::atomic::AtomicPtr::into_inner(self)
    }

    #[inline(always)]
    fn load (&self, order: Ordering) -> Self::Primitive {
        core::sync::atomic::AtomicPtr::load(self, order)
    }

    #[inline(always)]
    fn store (&self, val: Self::Primitive, order: Ordering) {
        core::sync::atomic::AtomicPtr::store(self, val, order)
    }

    #[inline(always)]
    fn swap (&self, val: Self::Primitive, order: Ordering) -> Self::Primitive {
        core::sync::atomic::AtomicPtr::swap(self, val, order)
    }

    #[inline(always)]
    fn compare_exchange (&self, current: Self::Primitive, new: Self::Primitive, success: Ordering, failure: Ordering) -> Result<Self::Primitive, Self::Primitive> {
        core::sync::atomic::AtomicPtr::compare_exchange(self, current, new, success, failure)
    }

    #[inline(always)]
    fn compare_exchange_weak (&self, current: Self::Primitive, new: Self::Primitive, success: Ordering, failure: Ordering) -> Result<Self::Primitive, Self::Primitive> {
        core::sync::atomic::AtomicPtr::compare_exchange_weak(self, current, new, success, failure)
    }

    #[inline(always)]
    fn fetch_update<F: FnMut(Self::Primitive) -> Option<Self::Primitive>> (&self, set_order: Ordering, fetch_ordering: Ordering, f: F) -> Result<Self::Primitive, Self::Primitive> {
        core::sync::atomic::AtomicPtr::fetch_update(self, set_order, fetch_ordering, f)
    }
}