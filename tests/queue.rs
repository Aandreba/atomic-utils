#![feature(allocator_api)]

use std::{thread::{available_parallelism, sleep}, num::NonZeroUsize};
use std::sync::atomic::{AtomicUsize, Ordering, AtomicBool};
use utils_atomics::*;
use rand::random;
mod debug_alloc;

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

#[cfg(feature = "alloc_api")]
#[test]
fn stress_fill_queue () {
    use core::time::Duration;
    
    let alloc = debug_alloc::DebugAlloc::new(std::alloc::Global, std::fs::File::create("alloc.txt").unwrap());
    let queue: FillQueue<i32, _> = FillQueue::new_in(&alloc);

    let mut pushed = AtomicUsize::new(0);
    let mut chopped = AtomicUsize::new(0);
    println!("initialized");

    std::thread::scope(|s| {
        for _ in 0..6 {
            s.spawn(|| {
                for _ in 0..RUNS {
                    let v = random::<i32>();
                    queue.push(v);
                    pushed.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                    sleep(Duration::from_millis(2));
                }
                println!("thread 1 done!");
            });
        }

        for _ in 0..4 {
            s.spawn(|| {
                for _ in 0..RUNS {
                    let count = queue.chop().count();
                    chopped.fetch_add(count, std::sync::atomic::Ordering::AcqRel);
                    sleep(Duration::from_millis(1));
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

/* REPRODUCABLE BUG (AT LEAST IN M1) */
#[cfg(feature = "alloc_api")]
#[test]
fn singlethread_queue () {
    let alloc = debug_alloc::DebugAlloc::new(std::alloc::Global, std::fs::File::create("alloc.txt").unwrap());
    let queue = FillQueue::<_, _>::new_with_block_size_in(NonZeroUsize::new(2).unwrap(), &alloc);

    for i in 0..RUNS {
        queue.push(i);
    }

    let chop = queue.chop();
    for i in chop {
        println!("{i}");
    }

    for i in 0..RUNS {
        queue.push(i);
    }

    let mut chop = queue.chop();
    for i in chop {
        println!("{i}");
    }
}