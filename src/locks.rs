#[cfg(not(feature = "nightly"))]
use core::marker::PhantomData;
use core::{fmt::Debug, mem::ManuallyDrop};

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        #[derive(Debug)]
        #[repr(transparent)]
        pub struct Lock (std::thread::Thread);

        #[derive(Debug)]
        pub struct LockSub ((), #[cfg(not(feature = "nightly"))] PhantomData<*mut ()>);

        impl Lock {
            #[inline]
            pub unsafe fn into_raw (self) -> *mut () {
                static_assertions::assert_eq_align!(Lock, *mut ());
                return core::mem::transmute(self)
            }

            #[inline]
            pub unsafe fn from_raw (raw: *mut ()) -> Self {
                static_assertions::assert_eq_align!(Lock, *mut ());
                return Self(core::mem::transmute(raw))
            }

            #[inline]
            pub fn silent_drop (self) {
                let mut this = ManuallyDrop::new(self);
                unsafe { core::ptr::drop_in_place(&mut this.0) }
            }
        }

        impl LockSub {
            #[allow(clippy::unused_self)]
            #[inline]
            pub fn wait (self) {
                std::thread::park();
            }

            #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
            #[allow(clippy::unused_self)]
            #[inline]
            pub fn wait_timeout (self, dur: core::time::Duration) {
                std::thread::park_timeout(dur);
            }
        }

        impl Drop for Lock {
            #[inline]
            fn drop (&mut self) {
                self.0.unpark();
            }
        }

        #[inline]
        pub fn lock () -> (Lock, LockSub) {
            return (Lock(std::thread::current()), LockSub((), #[cfg(not(feature = "nightly"))] PhantomData))
        }
    } else {
        use alloc::sync::Arc;

        #[derive(Debug)]
        #[repr(transparent)]
        pub struct Lock (alloc::sync::Arc<()>);

        #[derive(Debug)]
        pub struct LockSub (alloc::sync::Arc<()>, #[cfg(not(feature = "nightly"))] PhantomData<*mut ()>);

        impl Lock {
            #[inline]
            pub unsafe fn into_raw (self) -> *mut () {
                let this = ManuallyDrop::new(self);
                return Arc::into_raw(core::ptr::read(&this.0)).cast_mut()
            }

            #[inline]
            pub unsafe fn from_raw (raw: *mut ()) -> Self {
                return Self(Arc::from_raw(raw.cast_const()))
            }

            #[inline]
            pub fn silent_drop (self) {
                core::mem::forget(self);
            }
        }

        impl LockSub {
            #[inline]
            pub fn wait (self) {
                let mut this = self.0;
                loop {
                    match alloc::sync::Arc::try_unwrap(this) {
                        Ok(_) => return,
                        Err(e) => this = e
                    }
                    core::hint::spin_loop()
                }
            }
        }

        #[inline]
        pub fn lock () -> (Lock, LockSub) {
            let lock = alloc::sync::Arc::new(());
            return (Lock(lock.clone()), LockSub(lock, #[cfg(not(feature = "nightly"))] PhantomData))
        }

        impl Drop for Lock {
            #[inline]
            fn drop (&mut self) {}
        }
    }
}

impl Lock {
    #[allow(clippy::unused_self)]
    #[inline]
    pub fn wake(self) {}
}

cfg_if::cfg_if! {
    if #[cfg(feature = "nightly")] {
        impl !Send for LockSub {}
    } else {
        unsafe impl Sync for LockSub {}
    }
}
