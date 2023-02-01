#[cfg(feature = "alloc")]
use utils_atomics::channel::once::channel;

#[cfg(feature = "alloc")]
#[test]
fn send_and_receive () {
    let (send, recv) = channel::<i32>();
    std::thread::spawn(move || assert_eq!(recv.wait(), Some(32)));
    send.send(32);
}

#[cfg(feature = "alloc")]
#[test]
fn only_send () {
    let (send, _recv) = channel::<i32>();
    send.send(32);
}

#[cfg(feature = "alloc")]
#[test]
fn only_recv () {
    let (_send, recv) = channel::<i32>();
    std::thread::spawn(move || assert_eq!(recv.wait(), None));
}