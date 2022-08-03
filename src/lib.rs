#![no_std]
#![feature(allocator_api)]

pub(crate) extern crate alloc;

macro_rules! flat_mod {
    ($($i:ident),+) => {
        $(
            mod $i;
            pub use $i::*;
        )+
    };
}

flat_mod!(fill_queue);

pub mod prelude {
    pub use crate::fill_queue::*;
}