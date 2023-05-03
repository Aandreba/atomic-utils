#![cfg(feature = "alloc")]
#![cfg_attr(feature = "alloc_api", feature(allocator_api))]

use utils_atomics::cell::AtomicCell;

#[test]
fn basic_set() {
    let mut cell = AtomicCell::new(1);
    assert!(cell.is_some());
    assert_eq!(cell.take(), Some(1));
    assert_eq!(cell.replace(2), None);
    assert_eq!(cell.replace_boxed(Box::new(3)), Some(Box::new(2)));
    assert_eq!(cell.get_mut(), Some(&mut 3));
    assert_eq!(cell.take_boxed(), Some(Box::new(3)));
}

#[cfg(feature = "alloc_api")]
#[test]
fn test_alloc_api() {
    use std::alloc::System;

    let mut cell = AtomicCell::new_in(None, System);
    assert!(cell.is_none());
    assert_eq!(cell.take_in(), None);
    assert_eq!(cell.replace_in(2), None);
    assert_eq!(cell.get_mut(), Some(&mut 2));
}
