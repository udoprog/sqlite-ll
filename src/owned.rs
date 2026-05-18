use core::alloc::Layout;
use core::ptr::NonNull;

use alloc::alloc;

use crate::{Code, Error, Result};

/// An owned pointer with drop glue.
///
/// This is used internally to store opaque types.
pub(crate) struct Owned {
    ptr: NonNull<()>,
    drop: unsafe fn(NonNull<()>),
}

impl Owned {
    pub(crate) fn new<T>(value: T) -> Result<Self> {
        let layout = Layout::new::<T>();

        if layout.size() == 0 {
            return Ok(Self {
                ptr: NonNull::dangling(),
                drop: zero_sized_drop_glue::<T>,
            });
        }

        let ptr = unsafe {
            let ptr = alloc::alloc(layout);

            if ptr.is_null() {
                return Err(Error::new(Code::NOMEM, "allocation failed"));
            }

            ptr.cast::<T>().write(value);
            NonNull::new_unchecked(ptr.cast())
        };

        Ok(Self {
            ptr,
            drop: drop_glue::<T>,
        })
    }

    #[inline]
    pub(crate) fn as_ptr(&self) -> *mut () {
        self.ptr.as_ptr()
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: The busy callback is constructed in one go.
        unsafe {
            (self.drop)(self.ptr);
        }
    }
}

unsafe fn drop_glue<F>(ptr: NonNull<()>) {
    unsafe {
        let layout = Layout::new::<F>();
        alloc::dealloc(ptr.as_ptr().cast(), layout);
    }
}

unsafe fn zero_sized_drop_glue<T>(_: NonNull<()>) {}
