# Atomic Utils
Simple crate with various miscelaneous atomic-related utils

## Features
| Name        | Description                            | Enables             | Default         |
| ----------- | -------------------------------------- | ------------------- | --------------- |
| `std`       | Enables libstd functionality           | `alloc`             | Yes             |
| `alloc`     | Enables liballoc functionality         |                     | Yes (via `std`) |
| `alloc_api` | Enables `allocator_api` functionality  | `alloc` & `nightly` | No              |  
| `futures` | Enables async/await functionality        |                     | No              |
| `const`     | Enables constant trait implementations |                     | No              |
| `nightly`   | Enables the use of nightly features    |                     | Yes             |