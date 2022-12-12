#![feature(mem_copy_fn)]
use std::{sync::Mutex, hint::black_box};

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use crossbeam::queue::SegQueue;
use utils_atomics::FillQueue;

fn benchmark_queue_chop(c: &mut Criterion) {
    for i in [1, 10, 100, 1_000, 10_000] {
        let queue = SegQueue::new();
        (0usize..i).into_iter().for_each(|i| queue.push(i));
        c.bench_with_input(BenchmarkId::new("crossbeam", i), &(queue, i), |b, (queue, _)| {
            b.iter(|| {
                while let Some(x) = queue.pop() {
                    black_box(x);
                }
            })
        });
    
        let queue = Mutex::new((0..i).into_iter().collect::<Vec<_>>());
        c.bench_with_input(BenchmarkId::new("mutex vec", i), &(queue, i), |b, (queue, _)| {
            b.iter(|| {
                let mut queue = queue.lock().unwrap();
                for x in core::mem::take(&mut queue as &mut Vec<_>) {
                    black_box(x);
                }
            })
        });
        
        let mut queue = FillQueue::new();
        (0..i).into_iter().for_each(|i| queue.push_mut(i));
        c.bench_with_input(BenchmarkId::new("utils_atomics", i), &(queue, i), |b, (queue, _)| {
            b.iter(|| {
                for x in queue.chop() {
                    black_box(x);
                }
            })
        });
    }

    
}

criterion_group!(benches, benchmark_queue_chop);
criterion_main!(benches);