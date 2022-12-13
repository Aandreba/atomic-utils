use std::{thread::{available_parallelism, sleep}, num::NonZeroUsize};
use std::sync::atomic::{AtomicUsize, Ordering, AtomicBool};
use utils_atomics::*;
use rand::random;

const RUNS: usize = 20;

/*
TODO FIX MEMORY LEAK

thread 'stress_fill_queue' panicked at 'assertion failed: `(left == right)`
  left: `100`,
 right: `99`', tests/queue.rs:39:5
stack backtrace:
   0: rust_begin_unwind
             at /rustc/bdb07a8ec8e77aa10fb84fae1d4ff71c21180bb4/library/std/src/panicking.rs:575:5
   1: core::panicking::panic_fmt
             at /rustc/bdb07a8ec8e77aa10fb84fae1d4ff71c21180bb4/library/core/src/panicking.rs:64:14
   2: core::panicking::assert_failed_inner
   3: core::panicking::assert_failed
             at /rustc/bdb07a8ec8e77aa10fb84fae1d4ff71c21180bb4/library/core/src/panicking.rs:199:5
   4: queue::stress_fill_queue
             at ./tests/queue.rs:39:5
   5: queue::stress_fill_queue::{{closure}}
             at ./tests/queue.rs:10:25
   6: core::ops::function::FnOnce::call_once
             at /rustc/bdb07a8ec8e77aa10fb84fae1d4ff71c21180bb4/library/core/src/ops/function.rs:507:5
   7: core::ops::function::FnOnce::call_once
             at /rustc/bdb07a8ec8e77aa10fb84fae1d4ff71c21180bb4/library/core/src/ops/function.rs:507:5
note: Some details are omitted, run with `RUST_BACKTRACE=full` for a verbose backtrace.
test stress_fill_queue ... FAILED
*/

#[cfg(feature = "alloc")]
#[test]
fn stress_fill_queue () {
    let queue: FillQueue<i32> = FillQueue::new();
    let mut pushed = AtomicUsize::new(0);
    let mut chopped = AtomicUsize::new(0);
    println!("initialized");

    std::thread::scope(|s| {
        for _ in 1..(available_parallelism().unwrap().get() / 2) {
            s.spawn(|| {
                for _ in 0..RUNS {
                    let v = random::<i32>();
                    queue.push(v);
                    pushed.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                }
                println!("thread 1 done!");
            });

            s.spawn(|| {
                for _ in 0..RUNS {
                    let count = queue.chop().count();
                    chopped.fetch_add(count, std::sync::atomic::Ordering::AcqRel);
                }
                println!("thread 2 done!");
            });
        }
    });

    let rem = queue.chop().count();
    println!("{} v. {}", *pushed.get_mut(), *chopped.get_mut() + rem);
    assert_eq!(*pushed.get_mut(), *chopped.get_mut() + rem);
    queue.push(1);
}

#[test]
fn singlethread_queue () {
    let queue = FillQueue::new_with_block_size(NonZeroUsize::new(2).unwrap());
    queue.push(1);
    queue.push(2);
    queue.push(3);

    let mut chop = queue.chop();
    assert_eq!(chop.next(), Some(3));
    assert_eq!(chop.next(), Some(2));
    assert_eq!(chop.next(), Some(1));
    assert_eq!(chop.next(), None);
}