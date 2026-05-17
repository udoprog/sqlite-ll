use core::alloc::Layout;
use core::cmp::Ordering;
use core::ffi::c_int;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::ops::Deref;
use core::ptr;
use core::ptr::NonNull;
use core::slice;

use alloc::alloc::handle_alloc_error;

use crate::ffi;
use crate::{Code, Error, Result};

/// A slice of bytes that has been allocated with the sqlite allocator.
///
/// This dereferences to a byte slice.
pub struct OwnedBytes {
    ptr: NonNull<u8>,
    len: usize,
    cap: usize,
}

impl OwnedBytes {
    pub fn from_slice(bytes: impl AsRef<[u8]>) -> Result<Self> {
        let bytes = bytes.as_ref();
        Self::_from_slice(bytes)
    }

    fn _from_slice(bytes: &[u8]) -> Result<Self> {
        // SAFETY: We trust that the provided sqlite_* methods work as
        // advertised, and are abiding by the guarantees provided by the passed
        // in slice.
        unsafe {
            let len = bytes.len();

            // We do want to overallocate a little bit.
            let cap = match len {
                0 => 0,
                len => len.next_power_of_two().max(16),
            };

            let Ok(size) = c_int::try_from(cap) else {
                return Err(Error::new(Code::NOMEM, "slice is too large"));
            };

            let ptr = ffi::sqlite3_malloc(size);

            if ptr.is_null() {
                return Err(Error::new(Code::NOMEM, "failed to allocate memory"));
            }

            ptr::copy_nonoverlapping(bytes.as_ptr(), ptr.cast(), len);
            let ptr = NonNull::new_unchecked(ptr.cast());
            Ok(Self { ptr, len, cap })
        }
    }

    #[inline]
    pub(super) fn capacity(&self) -> usize {
        self.cap
    }

    #[inline]
    pub(super) unsafe fn from_raw(ptr: NonNull<u8>, len: usize) -> Self {
        Self { ptr, len, cap: len }
    }
}

impl Clone for OwnedBytes {
    #[inline]
    fn clone(&self) -> Self {
        match Self::_from_slice(self) {
            Ok(bytes) => bytes,
            Err(..) => {
                let layout = unsafe { Layout::from_size_align_unchecked(self.len, 1) };
                handle_alloc_error(layout)
            }
        }
    }
}

impl fmt::Debug for OwnedBytes {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self[..].fmt(f)
    }
}

impl AsRef<[u8]> for OwnedBytes {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self
    }
}

impl Deref for OwnedBytes {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }
}

impl Drop for OwnedBytes {
    #[inline]
    fn drop(&mut self) {
        // SAFETY: Construction is unsafe.
        unsafe {
            ffi::sqlite3_free(self.ptr.as_ptr().cast());
        }
    }
}

impl PartialEq for OwnedBytes {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self[..] == other[..]
    }
}

impl Eq for OwnedBytes {}

impl PartialOrd for OwnedBytes {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OwnedBytes {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self[..].cmp(&other[..])
    }
}

impl Hash for OwnedBytes {
    #[inline]
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        self[..].hash(state);
    }
}

impl PartialEq<[u8]> for OwnedBytes {
    #[inline]
    fn eq(&self, other: &[u8]) -> bool {
        self[..] == other[..]
    }
}

impl PartialEq<OwnedBytes> for [u8] {
    #[inline]
    fn eq(&self, other: &OwnedBytes) -> bool {
        self[..] == other[..]
    }
}
