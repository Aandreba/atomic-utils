#[cfg(not(feature = "nightly"))]
use core::marker::PhantomData;
use core::{
    fmt::Debug, mem::ManuallyDrop,
};

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        #[derive(Debug)]
        #[repr(transparent)]
        pub struct Lock (std::thread::Thread);

        #[derive(Debug)]
        pub struct LockSub (#[cfg(not(feature = "nightly"))] PhantomData<std::sync::MutexGuard<'static, ()>>);

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
            #[inline]
            pub fn wait (self) {
                std::thread::park();
            }
        }

        impl Drop for Lock {
            #[inline]
            fn drop (&mut self) {
                self.0.unpark()
            }
        }

        #[inline]
        pub fn lock () -> (Lock, LockSub) {
            return (Lock(std::thread::current()), LockSub(#[cfg(not(feature = "nightly"))] PhantomData))
        }
    } else {
        #[derive(Debug)]
        #[repr(transparent)]
        pub struct Lock (alloc::sync::Arc<()>);

        #[derive(Debug)]
        pub struct LockSub (alloc::sync::Arc<()>, #[cfg(not(feature = "nightly"))] PhantomData<std::sync::MutexGuard<'static, ()>>);

        impl Lock {
            #[inline]
            pub unsafe fn into_raw (self) -> *mut () {
                return Arc::into_raw(self.0).cast_mut()
            }

            #[inline]
            pub unsafe fn from_raw (raw: *mut ()) -> Self {
                return Self(Arc::from_raw(raw.cast_const()))
            }

            #[inline]
            pub fn silent_drop (self) {
                core::mem::forget(ManuallyDrop::new(self));
            }
        }

        impl LockSub {
            #[inline]
            pub fn wait (self) {
                let mut this = self;
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
    #[inline]
    pub fn wake (self) {}
}

#[cfg(feature = "nightly")]
impl !Send for LockSub {}
