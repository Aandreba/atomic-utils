#![feature(allocator_api)]
use std::{alloc::Allocator, io::Write, sync::Mutex};

pub struct DebugAlloc<A, W> {
    inner: A,
    file: Mutex<W>
}

impl<A: Allocator, W: std::io::Write> DebugAlloc<A, W> {
    #[inline]
    pub fn new (inner: A, writer: W) -> Self {
        return Self { inner, file: Mutex::new(writer) }
    }
}

unsafe impl<A: Allocator, W: std::io::Write> Allocator for DebugAlloc<A, W> {
    #[inline]
    fn allocate(&self, layout: std::alloc::Layout) -> Result<std::ptr::NonNull<[u8]>, std::alloc::AllocError> {
        macro_rules! tri {
            ($e:expr) => {
                match $e {
                    Ok(x) => x,
                    Err(_) => return Err(std::alloc::AllocError)
                }
            };
        }

        let ptr = self.inner.allocate(layout)?;
        let mut file = match self.file.lock() {
            Ok(x) => x,
            Err(e) => e.into_inner()
        };

        tri! { file.write_fmt(format_args!("Allocated {ptr:p}: {layout:?}\n")) };
        return Ok(ptr)
    }

    #[inline]
    unsafe fn deallocate(&self, ptr: std::ptr::NonNull<u8>, layout: std::alloc::Layout) {
        self.inner.deallocate(ptr, layout);
        let mut file = match self.file.lock() {
            Ok(x) => x,
            Err(e) => e.into_inner()
        };
        let _ = file.write_fmt(format_args!("Deallocated {ptr:p}: {layout:?}\n"));
    }
}