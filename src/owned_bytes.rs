use core::alloc::Layout;
use core::cmp::Ordering;
use core::ffi::c_int;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::mem;
use core::ops::Deref;
use core::ptr::NonNull;
use core::slice;

use alloc::alloc::handle_alloc_error;

use crate::ffi;
use crate::{Code, Error, Result};

const MAX_CAP: usize = if mem::size_of::<isize>() > mem::size_of::<c_int>() {
    c_int::MAX as usize
} else {
    isize::MAX as usize
};

struct AllocError;

impl fmt::Display for AllocError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "failed to allocate memory")
    }
}

/// A slice of bytes that has been allocated with the sqlite allocator.
///
/// This dereferences to a byte slice.
pub struct OwnedBytes {
    ptr: NonNull<u8>,
    len: usize,
    cap: usize,
}

impl OwnedBytes {
    /// Creates a new, empty `OwnedBytes`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sqll::OwnedBytes;
    ///
    /// let bytes = OwnedBytes::new();
    /// assert_eq!(bytes.len(), 0);
    /// assert!(bytes.is_empty());
    /// # Ok::<_, sqll::Error>(())
    /// ```
    pub const fn new() -> Self {
        Self {
            ptr: NonNull::dangling(),
            len: 0,
            cap: 0,
        }
    }

    /// Creates a new `OwnedBytes` with the specified capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// use sqll::OwnedBytes;
    ///
    /// let bytes = OwnedBytes::with_capacity(100)?;
    /// assert!(bytes.capacity() >= 100);
    /// # Ok::<_, sqll::Error>(())
    /// ```
    pub fn with_capacity(cap: usize) -> Result<Self> {
        let mut this = Self::new();

        if let Err(error) = this.reserve(cap) {
            return Err(Error::new(Code::NOMEM, error));
        }

        Ok(this)
    }

    /// Returns the length of the byte slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use sqll::OwnedBytes;
    ///
    /// let mut bytes = OwnedBytes::new();
    /// assert_eq!(bytes.len(), 0);
    ///
    /// bytes.extend_from_slice(b"hello ")?;
    /// assert_eq!(bytes.len(), 6);
    ///
    /// assert_eq!(bytes[..], b"hello "[..]);
    /// bytes.extend_from_slice(b"world")?;
    /// assert_eq!(bytes.len(), 11);
    /// assert_eq!(bytes[..], b"hello world"[..]);
    /// # Ok::<_, sqll::Error>(())
    /// ```
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the byte slice is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use sqll::OwnedBytes;
    ///
    /// let mut bytes = OwnedBytes::new();
    /// assert!(bytes.is_empty());
    ///
    /// bytes.extend_from_slice(b"hello world")?;
    /// assert!(!bytes.is_empty());
    /// # Ok::<_, sqll::Error>(())
    /// ```
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Creates a new `OwnedBytes` from the given slice, copying the provided
    /// data into it.
    ///
    /// # Examples
    ///
    /// ```
    /// use sqll::OwnedBytes;
    ///
    /// let mut bytes = OwnedBytes::new();
    /// bytes.extend_from_slice(b"hello world")?;
    /// assert_eq!(bytes[..], b"hello world"[..]);
    /// # Ok::<_, sqll::Error>(())
    /// ```
    pub fn extend_from_slice(&mut self, bytes: &[u8]) -> Result<()> {
        if let Err(error) = self.try_extend_from_slice(bytes) {
            return Err(Error::new(Code::NOMEM, error));
        }

        Ok(())
    }

    fn try_extend_from_slice(&mut self, bytes: &[u8]) -> Result<(), AllocError> {
        self.reserve(bytes.len())?;

        // SAFETY: We trust that the provided sqlite_* methods work as
        // advertised, and are abiding by the guarantees provided by the passed
        // in slice.
        unsafe {
            bytes
                .as_ptr()
                .copy_to_nonoverlapping(self.ptr.as_ptr().add(self.len), bytes.len());

            self.len += bytes.len();
            Ok(())
        }
    }

    /// Returns the capacity of the byte slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use sqll::OwnedBytes;
    ///
    /// let bytes = OwnedBytes::with_capacity(100)?;
    /// assert!(bytes.capacity() >= 100);
    /// # Ok::<_, sqll::Error>(())
    /// ```
    #[inline]
    pub fn capacity(&self) -> usize {
        self.cap
    }

    /// Creates a new `OwnedBytes` from the given raw parts.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the provided pointer is initialized up to
    /// `len`, and that it has been constructed using the sqlite allocator.
    #[inline]
    pub(super) unsafe fn from_raw(ptr: NonNull<u8>, len: usize) -> Self {
        Self { ptr, len, cap: len }
    }

    fn reserve(&mut self, additional: usize) -> Result<(), AllocError> {
        fn checked_grow(base: usize, needed: usize) -> Option<usize> {
            let new_cap = base.checked_add(needed)?.max(16);

            let new_cap = match new_cap.checked_next_power_of_two() {
                Some(cap) => cap,
                None if new_cap <= MAX_CAP => new_cap,
                None => return None,
            };

            Some(new_cap)
        }

        unsafe {
            let Some(new_cap) = checked_grow(self.len, additional) else {
                return Err(AllocError);
            };

            if new_cap < self.cap {
                return Ok(());
            }

            let ptr = if self.cap == 0 {
                ffi::sqlite3_malloc(new_cap as c_int)
            } else {
                ffi::sqlite3_realloc(self.ptr.as_ptr().cast(), new_cap as c_int)
            };

            if ptr.is_null() {
                return Err(AllocError);
            }

            self.ptr = NonNull::new_unchecked(ptr.cast());
            self.cap = new_cap;
            Ok(())
        }
    }
}

impl Clone for OwnedBytes {
    #[inline]
    fn clone(&self) -> Self {
        let mut this = Self::new();

        if let Err(AllocError) = this.try_extend_from_slice(self) {
            let layout = unsafe { Layout::from_size_align_unchecked(self.len, 1) };
            handle_alloc_error(layout);
        }

        this
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
        if self.cap == 0 {
            return;
        }

        // SAFETY: All ways we have to construct OwnedBytes require that it's
        // done through sqlite's allocator.
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
