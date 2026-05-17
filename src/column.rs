use core::ffi::c_int;
use core::fmt;

/// Defines the index for a column.
///
/// This hides the implementation detail for traits which reference a column.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Column {
    raw: c_int,
}

impl Column {
    /// The first column index used when reading values from a [`Statement`].
    ///
    /// [`Statement`]: crate::Statement
    pub const FIRST: Self = Self::from_raw(0);

    /// Construct a column from its raw representation.
    pub const fn from_raw(raw: c_int) -> Self {
        Self { raw }
    }

    #[inline]
    pub(crate) const fn raw(&self) -> c_int {
        self.raw
    }
}

impl From<usize> for Column {
    #[inline]
    fn from(value: usize) -> Self {
        Column {
            raw: value as c_int,
        }
    }
}

impl fmt::Debug for Column {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.raw.fmt(f)
    }
}
