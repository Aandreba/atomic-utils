#![feature(int_roundings, new_uninit)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "alloc", feature(allocator_api))]
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
    if #[cfg(feature = "alloc")] {
        pub mod fill_queue;
        pub mod bitfield;

        pub use fill_queue::FillQueue;
        pub use bitfield::AtomicBitBox;
    }
}

flat_mod!(take);

#[path = "trait.rs"]
pub mod traits;

pub mod prelude {
    pub use crate::fill_queue::*;
    pub use crate::take::*;
    pub use crate::traits::Atomic;
}

cfg_if::cfg_if! {
    if #[cfg(target_has_atomic = "8")] {
        pub(crate) type Flag = core::sync::atomic::AtomicU8;
        const TRUE : u8 = 1;
        const FALSE : u8 = 0;
    } else if #[cfg(target_has_atomic = "16")] {
        pub(crate) type Flag = core::sync::atomic::AtomicU16;
        const TRUE : u16 = 1;
        const FALSE : u16 = 0;
    } else if #[cfg(target_has_atomic = "32")] {
        pub(crate) type Flag = core::sync::atomic::AtomicU32;
        const TRUE : u32 = 1;
        const FALSE : u32 = 0;
    } else if #[cfg(target_has_atomic = "64")] {
        pub(crate) type Flag = core::sync::atomic::AtomicU64;
        const TRUE : u64 = 1;
        const FALSE : u64 = 0;
    } else {
        pub(crate) type Flag = core::sync::atomic::AtomicUsize;
        const TRUE : usize = 1;
        const FALSE : usize = 0;
    }
}