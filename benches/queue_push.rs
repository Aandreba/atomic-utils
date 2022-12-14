#![feature(mem_copy_fn)]
use std::sync::Mutex;

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use crossbeam::queue::SegQueue;
use utils_atomics::FillQueue;

const RUNS_PER_THREAD: usize = 50;
const THREADS: usize = 8;

fn benchmark_queue_push(c: &mut Criterion) {
    for i in 8..=THREADS {
        c.bench_with_input(BenchmarkId::new("crossbeam", i), &(SegQueue::new(), i), |b, (queue, i)| {
            b.iter(|| {
                bench_through_threads(queue, SegQueue::push, *i);
            })
        });
    
        c.bench_with_input(BenchmarkId::new("mutex vec", i), &(Mutex::new(Vec::new()), i), |b, (queue, i)| {
            b.iter(|| {
                bench_through_threads(queue, |x, v| x.lock().unwrap().push(v), *i);
            })
        });
        
        c.bench_with_input(BenchmarkId::new("utils_atomics", i), &(FillQueue::new(), i), |b, (queue, i)| {
            b.iter(|| {
                bench_through_threads(queue, FillQueue::push, *i);
            })
        });
    }
}

#[inline]
fn bench_through_threads<Q: Send + Sync, F: Send + Sync + Fn(&Q, usize)> (queue: &Q, push: F, threads: usize) {    
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                for i in 0..RUNS_PER_THREAD {
                    push(queue, i);
                }
            });
        }
    })
}

criterion_group!(benches, benchmark_queue_push);
criterion_main!(benches);