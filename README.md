[![Crates.io](https://img.shields.io/crates/v/utils_atomics)](https://crates.io/crates/utils_atomics)
[![docs.rs](https://img.shields.io/docsrs/utils_atomics)](https://docs.rs/utils_atomics/latest)
[![GitHub](https://img.shields.io/github/license/Aandreba/atomic-utils)](https://github.com/Aandreba/atomic-utils)

# Atomic Utils
Simple crate with various miscelaneous atomic-related utils

## FillQueue
An atomic queue intended for use cases where taking the full contents of the queue is needed.

The queue is, basically, an atomic singly-linked list, where nodes are first allocated and then the list's tail
is atomically updated.

When the queue is "chopped", the list's tail is swaped to null, and it's previous tail is used as the base of the [`ChopIter`]

### Performance 
The performance of pushing elements is expected to be similar to pushing elements to a `SegQueue` or `Mutex<Vec<_>>`,
but "chopping" elements is expected to be arround 2 times faster than with a `Mutex<Vec<_>>`, and 3 times faster than a `SegQueue`

> You can see the benchmark results [here](https://docs.google.com/spreadsheets/d/1wcyD3TlCQMCPFHOfeko5ytn-R7aM8T7lyKVir6vf_Wo/edit?usp=sharing)

### Use `FillQueue` when:
- You want a queue that's updateable by shared reference
- You want to retreive all elements of the queue at once
- There is no specifically desired order for the elements to be retreived on, or that order is LIFO (Last In First Out)

### Don't use `FillQueue` when:
- You don't need a queue updateable by shared reference
- You want to retreive the elements of the queue one by one (see `SegQueue`)
- You require the elements in a specific order that isn't LIFO

## Features
| Name        | Description                            | Enables             | Default         |
| ----------- | -------------------------------------- | ------------------- | --------------- |
| `std`       | Enables libstd functionality           | `alloc`             | Yes             |
| `alloc`     | Enables liballoc functionality         |                     | Yes (via `std`) |
| `alloc_api` | Enables `allocator_api` functionality  | `alloc` & `nightly` | No              |  
| `futures`   | Enables async/await functionality      |                     | No              |
| `const`     | Enables constant trait implementations |                     | No              |
| `nightly`   | Enables the use of nightly features    |                     | Yes             |