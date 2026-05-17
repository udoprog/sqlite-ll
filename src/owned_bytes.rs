use core::cmp::Ordering;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::ops::Deref;
use core::ptr::NonNull;
use core::slice;

use crate::ffi;

/// A slice of bytes that has been allocated with the sqlite allocator.
///
/// This dereferences to a byte slice.
pub struct OwnedBytes {
    ptr: NonNull<u8>,
    len: usize,
}

impl OwnedBytes {
    #[inline]
    pub(super) unsafe fn from_raw(ptr: NonNull<u8>, len: usize) -> Self {
        Self { ptr, len }
    }
}

impl fmt::Debug for OwnedBytes {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self[..].fmt(f)
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
