use std::{thread::{available_parallelism, sleep}, time::{Duration}, num::NonZeroUsize};
use std::sync::atomic::{AtomicUsize, Ordering, AtomicBool};
use utils_atomics::*;
use rand::random;

const RUNS: usize = 200_000;

#[cfg(feature = "alloc")]
#[test]
fn stress_fill_queue () {

    let queue: FillQueue<i32> = FillQueue::new();
    let mut pushed = AtomicUsize::new(0);
    let mut chopped = AtomicUsize::new(0);

    std::thread::scope(|s| {
        for _ in 1..(available_parallelism().unwrap().get() / 2) {
            s.spawn(|| {
                for _ in 0..RUNS {
                    let v = random::<i32>();
                    queue.push(v);
                    pushed.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                    //println!("looped 1")
                }
            });

            s.spawn(|| {
                for _ in 0..RUNS {
                    let count = queue.chop().count();
                    chopped.fetch_add(count, std::sync::atomic::Ordering::AcqRel);
                    //println!("looped 2")
                }
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