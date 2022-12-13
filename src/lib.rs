#![cfg_attr(feature = "nightly", feature(int_roundings, ptr_metadata))]
#![cfg_attr(all(feature = "nightly", feature = "alloc"), feature(new_uninit))]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "alloc_api", feature(allocator_api))]
#![cfg_attr(feature = "const", feature(const_trait_impl))]
#![cfg_attr(docsrs, feature(doc_cfg))]

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

        #[cfg(not(bootstrap))]
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
        #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
        pub mod fill_queue;
        #[cfg(feature = "nightly")]
        #[cfg_attr(docsrs, doc(cfg(all(feature = "alloc", feature = "nightly"))))]
        pub mod bitfield;
        #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
        pub mod flag;

        #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
        pub use fill_queue::FillQueue;
        #[docfg::docfg(all(feature = "alloc", feature = "nightly"))]
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
        pub(crate) type InnerFlag = core::sync::atomic::AtomicU8;
        const TRUE : u8 = 1;
        const FALSE : u8 = 0;
    } else if #[cfg(target_has_atomic = "16")] {
        pub(crate) type InnerFlag = core::sync::atomic::AtomicU16;
        const TRUE : u16 = 1;
        const FALSE : u16 = 0;
    } else if #[cfg(target_has_atomic = "32")] {
        pub(crate) type InnerFlag = core::sync::atomic::AtomicU32;
        const TRUE : u32 = 1;
        const FALSE : u32 = 0;
    } else if #[cfg(target_has_atomic = "64")] {
        pub(crate) type InnerFlag = core::sync::atomic::AtomicU64;
        const TRUE : u64 = 1;
        const FALSE : u64 = 0;
    } else {
        pub(crate) type InnerFlag = core::sync::atomic::AtomicUsize;
        const TRUE : usize = 1;
        const FALSE : usize = 0;
    }
}