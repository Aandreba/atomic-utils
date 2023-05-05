#![deny(clippy::all)]
#![deny(clippy::perf)]
#![warn(clippy::pedantic)]
#![allow(clippy::needless_return)]
#![allow(clippy::missing_safety_doc)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::semicolon_if_nothing_returned)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::wildcard_imports)]
#![allow(clippy::explicit_deref_methods)]
#![allow(clippy::match_bool)]
#![cfg_attr(test, allow(clippy::bool_assert_comparison))]
/* */
#![cfg_attr(feature = "nightly", feature(int_roundings, negative_impls, c_size_t))]
#![cfg_attr(all(feature = "nightly", feature = "alloc"), feature(new_uninit))]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "alloc_api", feature(allocator_api))]
#![cfg_attr(feature = "const", feature(const_trait_impl))]
#![cfg_attr(docsrs, feature(doc_cfg))]

use core::fmt::Display;

use docfg::docfg;

#[cfg(feature = "alloc")]
pub(crate) extern crate alloc;

macro_rules! flat_mod {
    ($($i:ident),+) => {
        $(
            mod $i;
            pub use $i::*;
        )+
    };
}

cfg_if::cfg_if! {
    if #[cfg(feature = "alloc_api")] {
        pub use core::alloc::AllocError;
    } else {
        /// The `AllocError` error indicates an allocation failure
        /// that may be due to resource exhaustion or to
        /// something wrong when combining the given input arguments with this
        /// allocator.
        #[derive(Copy, Clone, PartialEq, Eq, Debug)]
        pub struct AllocError;

        #[cfg(feature = "std")]
        impl std::error::Error for AllocError {}

        // (we need this for downstream impl of trait Error)
        impl core::fmt::Display for AllocError {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str("memory allocation failed")
            }
        }
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "alloc")] {
        // #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
        // pub mod semaphore;
        #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
        pub mod fill_queue;
        #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
        pub mod bitfield;
        #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
        pub mod flag;
        #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
        pub mod channel;
        #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
        pub mod notify;
        mod cell;
        // #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
        // pub mod arc_cell;
        /// Blocking locks
        #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
        pub mod locks;

        #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
        pub use cell::AtomicCell;
        #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
        pub use fill_queue::FillQueue;
        #[docfg::docfg(feature = "alloc")]
        pub use bitfield::AtomicBitBox;
    }
}

flat_mod!(take);

#[path = "trait.rs"]
pub mod traits;

pub mod prelude {
    #[docfg::docfg(feature = "alloc")]
    pub use crate::fill_queue::*;
    pub use crate::take::*;
    pub use crate::traits::Atomic;
}

cfg_if::cfg_if! {
    if #[cfg(target_has_atomic = "8")] {
        pub(crate) type InnerFlag = u8;
        pub(crate) type InnerAtomicFlag = core::sync::atomic::AtomicU8;
    } else if #[cfg(target_has_atomic = "16")] {
        pub(crate) type InnerFlag = u16;
        pub(crate) type InnerAtomicFlag = core::sync::atomic::AtomicU16;
    } else if #[cfg(target_has_atomic = "32")] {
        pub(crate) type InnerFlag = u32;
        pub(crate) type InnerAtomicFlag = core::sync::atomic::AtomicU32;
    } else if #[cfg(target_has_atomic = "64")] {
        pub(crate) type InnerFlag = u64;
        pub(crate) type InnerAtomicFlag = core::sync::atomic::AtomicU64;
    } else {
        pub(crate) type InnerFlag = usize;
        pub(crate) type InnerAtomicFlag = core::sync::atomic::AtomicUsize;
    }
}

pub(crate) const TRUE: InnerFlag = 1;
pub(crate) const FALSE: InnerFlag = 0;

/// Error returned when a timeout ocurrs before the main operation completes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Timeout;

impl Display for Timeout {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "The main operation timed out before it could be completed"
        )
    }
}

#[docfg(feature = "std")]
impl std::error::Error for Timeout {}

#[allow(unused)]
#[inline]
pub(crate) fn is_some_and<T, F: FnOnce(T) -> bool>(v: Option<T>, f: F) -> bool {
    match v {
        None => false,
        Some(x) => f(x),
    }
}

#[allow(unused)]
#[inline]
pub(crate) fn div_ceil(lhs: usize, rhs: usize) -> usize {
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            return lhs.div_ceil(rhs)
        } else {
            let d = lhs / rhs;
            let r = lhs % rhs;
            if r > 0 && rhs > 0 {
                d + 1
            } else {
                d
            }
        }
    }
}
