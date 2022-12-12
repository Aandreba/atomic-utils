use std::{thread::{spawn, available_parallelism, sleep}, time::{Duration}};
use utils_atomics::*;
use rand::random;

const RUNS : usize = 10;
const STRESS : i32 = 50;

#[cfg(feature = "alloc")]
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