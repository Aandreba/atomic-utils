#![cfg(feature = "alloc")]

use std::thread::{available_parallelism, spawn};
use utils_atomics::*;

#[test]
fn stress_flag() {
    use std::sync::atomic::AtomicUsize;

    static STARTED: AtomicUsize = AtomicUsize::new(0);
    static ENDED: AtomicUsize = AtomicUsize::new(0);

    let (flag, sub) = flag::mpmc::flag();
    let mut handles = Vec::new();

    for _ in 1..available_parallelism().unwrap().get() {
        let sub = sub.clone();
        handles.push(spawn(move || {
            STARTED.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            sub.wait();
            ENDED.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        }));
    }

    //flag.mark();
    drop(flag);
    handles
        .into_iter()
        .map(std::thread::JoinHandle::join)
        .for_each(Result::unwrap);

    assert_eq!(
        STARTED.load(std::sync::atomic::Ordering::Acquire),
        ENDED.load(std::sync::atomic::Ordering::Acquire)
    );
}

#[cfg(feature = "futures")]
#[tokio::test]
async fn stress_async_flag() {
    use std::sync::atomic::AtomicUsize;

    const SIZE: usize = 100_000;
    static STARTED: AtomicUsize = AtomicUsize::new(0);
    static ENDED: AtomicUsize = AtomicUsize::new(0);

    let (flag, _) = utils_atomics::flag::mpmc::async_flag();
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
    assert_eq!(
        STARTED.load(std::sync::atomic::Ordering::Acquire),
        ENDED.load(std::sync::atomic::Ordering::Acquire)
    );
}
