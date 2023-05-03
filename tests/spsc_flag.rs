#![cfg(feature = "alloc")]
use utils_atomics::*;

#[test]
fn stress_flag() {
    use std::sync::atomic::AtomicUsize;

    static STARTED: AtomicUsize = AtomicUsize::new(0);
    static ENDED: AtomicUsize = AtomicUsize::new(0);

    let (flag, sub) = flag::mpsc::flag();
    let handle = std::thread::spawn(move || {
        STARTED.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        sub.wait();
        ENDED.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    });

    flag.mark();
    handle.join().unwrap();

    assert_eq!(
        STARTED.load(std::sync::atomic::Ordering::Acquire),
        ENDED.load(std::sync::atomic::Ordering::Acquire)
    );
}

#[cfg(feature = "futures")]
#[tokio::test]
async fn stress_async_flag() {
    use std::sync::atomic::AtomicUsize;

    static STARTED: AtomicUsize = AtomicUsize::new(0);
    static ENDED: AtomicUsize = AtomicUsize::new(0);

    let (flag, sub) = utils_atomics::flag::mpsc::async_flag();
    let handle = tokio::spawn(async move {
        STARTED.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        sub.await;
        ENDED.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    });

    flag.mark();
    handle.await.unwrap();
    assert_eq!(
        STARTED.load(std::sync::atomic::Ordering::Acquire),
        ENDED.load(std::sync::atomic::Ordering::Acquire)
    );
}
