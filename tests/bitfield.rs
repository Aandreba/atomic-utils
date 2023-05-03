#![cfg(feature = "alloc")]

use std::sync::atomic::{AtomicU16, Ordering};
use utils_atomics::AtomicBitBox;

#[test]
fn basic() {
    let field = AtomicBitBox::<AtomicU16>::new(30);
    assert_eq!(field.get(0, Ordering::Acquire), Some(false));
    assert_eq!(field.get(30, Ordering::Acquire), None);

    assert_eq!(field.set_true(1, Ordering::Relaxed), Some(false));
    assert_eq!(field.set_true(31, Ordering::Relaxed), None);
    assert_eq!(field.get(1, Ordering::Acquire), Some(true));

    assert_eq!(field.set(true, 2, Ordering::Acquire), Some(false));
    assert_eq!(field.set(false, 2, Ordering::Acquire), Some(true));
    assert_eq!(field.set_false(2, Ordering::Acquire), Some(false));
}
