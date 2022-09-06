use std::{thread::{spawn, available_parallelism, sleep}, time::{Duration}};
use utils_atomics::*;
use rand::random;

const RUNS : usize = 10;
const STRESS : i32 = 50;

#[test]
fn stress_fill_queue () {
    static QUEUE : FillQueue<i32> = FillQueue::new();

    for _ in 1..available_parallelism().unwrap().get() {
        spawn(move || {
            loop {
                let v = random::<i32>();
                QUEUE.push(v);

                let nanos = i32::abs(v / (2 * STRESS));
                sleep(Duration::from_nanos(nanos as u64));
            }
        });
    }

    for _ in 0..RUNS {
        sleep(Duration::from_secs(1));
        let count = QUEUE.chop().count();
        println!("Chopped elements: {count}!")
    }
}

#[test]
fn stress_flag () {
    use std::sync::atomic::AtomicUsize;

    static STARTED : AtomicUsize = AtomicUsize::new(0);
    static ENDED : AtomicUsize = AtomicUsize::new(0);

    let (flag, sub) = flag();
    let mut handles = Vec::new();

    for _ in 1..available_parallelism().unwrap().get() {
        let sub = sub.clone();
        handles.push(spawn(move || {
            STARTED.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            sub.subscribe();
            ENDED.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        }));
    }

    //flag.mark();
    drop(flag);
    handles.into_iter()
        .map(std::thread::JoinHandle::join)
        .for_each(Result::unwrap);

    assert_eq!(STARTED.load(std::sync::atomic::Ordering::Acquire), ENDED.load(std::sync::atomic::Ordering::Acquire));
}

#[cfg(feature = "futures")]
#[tokio::test]
async fn stress_async_flag () {
    use std::sync::atomic::AtomicUsize;

    const SIZE : usize = 100_000;
    static STARTED : AtomicUsize = AtomicUsize::new(0);
    static ENDED : AtomicUsize = AtomicUsize::new(0);

    let flag = utils_atomics::AsyncFlag::new();
    let mut handles = Vec::with_capacity(SIZE);

    for _ in 0..SIZE {
        let sub = flag.subscribe();
        handles.push(tokio::spawn(async move {
            STARTED.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            sub.await;
            ENDED.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        }));
    }

    flag.mark();
    let _ = futures::future::join_all(handles).await;
    assert_eq!(STARTED.load(std::sync::atomic::Ordering::Acquire), ENDED.load(std::sync::atomic::Ordering::Acquire));
}