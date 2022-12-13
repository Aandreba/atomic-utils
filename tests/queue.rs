use std::{thread::{spawn, available_parallelism, sleep}, time::{Duration}, num::NonZeroUsize};
use utils_atomics::*;
use rand::random;

const RUNS: usize = 2;
const STRESS: i32 = 50;

#[cfg(feature = "alloc")]
#[test]
fn stress_fill_queue () {
    use std::sync::atomic::{AtomicUsize, Ordering, AtomicBool};

    let queue: FillQueue<i32> = FillQueue::new();
    let mut pushed = AtomicUsize::new(0);
    let mut chopped = AtomicUsize::new(0);
    let alive = AtomicBool::new(true);

    std::thread::scope(|s| {
        for _ in 1..(available_parallelism().unwrap().get() / 2) {
            s.spawn(|| {
                while alive.load(Ordering::SeqCst) {
                    let v = random::<i32>();
                    queue.push(v);
                    pushed.fetch_add(1, std::sync::atomic::Ordering::AcqRel);

                    let nanos = i32::abs(v / (2 * STRESS));
                    sleep(Duration::from_nanos(nanos as u64));
                    //println!("looped 1")
                }
            });

            s.spawn(|| {
                while alive.load(Ordering::SeqCst) {
                    let v = random::<i32>();
                    let count = queue.chop().count();
                    chopped.fetch_add(count, std::sync::atomic::Ordering::AcqRel);

                    let nanos = i32::abs(v / (2 * STRESS));
                    sleep(Duration::from_nanos(nanos as u64));
                    //println!("looped 2")
                }
            });
        }

        for _ in 0..RUNS {
            sleep(Duration::from_secs(1));
            let count = queue.chop().count();
            chopped.fetch_add(count, std::sync::atomic::Ordering::AcqRel);

            println!("Current tally");
            println!("Pushed: {}", pushed.load(Ordering::Acquire));
            println!("Chopped: {}", chopped.load(Ordering::Acquire));
            println!("")
        }

        alive.store(false, Ordering::SeqCst);
        println!("Done!")
    });

    let rem = queue.chop().count();
    println!("{} v. {}", *pushed.get_mut(), *chopped.get_mut() + rem);
    assert_eq!(*pushed.get_mut(), *chopped.get_mut() + rem);
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