use core::ffi::c_int;
use core::fmt;
use core::ops::Add;

/// Defines a parameter index for binding values to a statement.
///
/// This hides the implementation detail for traits which reference an index.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Index {
    raw: c_int,
}

impl Index {
    /// The first index used when binding parameters into a [`Statement`].
    ///
    /// This is useful when implementing [`Bind`] for a primitive type which
    /// implements [`BindValue`].
    ///
    /// [`Statement`]: crate::Statement
    /// [`Bind`]: crate::Bind
    /// [`BindValue`]: crate::BindValue
    pub const BIND: Self = Self::from_raw(1);

    /// Construct an index from its raw representation.
    #[inline]
    pub const fn from_raw(raw: c_int) -> Self {
        Self { raw }
    }

    #[inline]
    pub(crate) const fn raw(&self) -> c_int {
        self.raw
    }
}

/// Add implementation for [`Index`] to allow incrementing the index.
impl Add<usize> for Index {
    type Output = Self;

    #[inline]
    fn add(self, rhs: usize) -> Self::Output {
        Index {
            raw: self.raw + rhs as c_int,
        }
    }
}

impl fmt::Debug for Index {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.raw.fmt(f)
    }
}
