use core::cell::UnsafeCell;
use core::ffi::CStr;
use core::fmt;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use core::sync::atomic::{AtomicU64, Ordering};

use alloc::boxed::Box;
#[cfg(all(feature = "alloc", feature = "std"))]
use alloc::ffi::CString;
use alloc::sync::Arc;
use alloc::vec::Vec;

#[cfg(feature = "std")]
use std::path::Path;

use tokio::sync::{AcquireError, OwnedSemaphorePermit, Semaphore};

use crate::{Connection, Error, NotThreadSafe, OpenOptions};

/// A collection of prepared statements that can be built from a [`Connection`].
///
/// This is what the read (`R`) and write (`W`) sides of a [`Pool`] are made of.
/// Each field of the implementing type is a [`SendStatement`] prepared from a
/// `#[sql = "..."]` attribute. See the [`Statements` derive] for the available
/// attributes and a complete example.
///
/// Implementing this trait by hand is an error, it should only be implemented
/// by the [`Statements` derive] macro.
///
/// [`Statements` derive]: derive@crate::Statements
/// [`SendStatement`]: crate::SendStatement
pub trait Statements
where
    Self: Sized,
{
    #[doc(hidden)]
    fn build(c: Connection) -> Result<Self, PoolError>;
}

/// A marker trait for [`Statements`] that only ever read from the database.
///
/// This is required for the read side `R` of a [`Pool`], since it is what allows
/// multiple [`shared`] guards to access distinct connections concurrently. It is
/// implemented automatically by the [`Statements` derive] for types annotated
/// with `#[sql(read_only)]`.
///
/// [`shared`]: Pool::shared
/// [`Statements` derive]: derive@crate::Statements
///
/// # Safety
///
/// The implementer must guarantee that none of the statements in the collection
/// mutate the database. If a mutating statement is run through a [`shared`]
/// guard it may execute concurrently with other readers, which SQLite does not
/// permit on a single connection and which can lead to data races or
/// corruption. The [`Statements` derive] upholds this by rejecting any
/// statement for which [`Statement::is_read_only`] returns `false`.
///
/// [`Statement::is_read_only`]: crate::Statement::is_read_only
pub unsafe trait IsReadOnly: Statements {}

/// Errors raised by the high-level [`Pool`] API.
///
/// This covers failures while opening the underlying connections, preparing the
/// statements of a [`Statements`] collection, converting them into a
/// thread-safe form, or asserting that a `#[sql(read_only)]` collection really
/// is read-only.
pub struct PoolError {
    kind: ErrorKind,
}

impl PoolError {
    #[inline]
    #[doc(hidden)]
    pub fn index_not_read_only(index: usize) -> Self {
        Self::from(ErrorKind::IndexNotReadOnly(index))
    }

    #[inline]
    #[doc(hidden)]
    pub fn field_not_read_only(name: &'static str) -> Self {
        Self::from(ErrorKind::FieldNotReadOnly(name))
    }
    #[inline]
    #[doc(hidden)]
    pub fn index_prepare_failed(index: usize, source: Error) -> Self {
        Self::from(ErrorKind::IndexPrepareFailed(index, source))
    }

    #[inline]
    #[doc(hidden)]
    pub fn field_prepare_failed(name: &'static str, source: Error) -> Self {
        Self::from(ErrorKind::FieldPrepareFailed(name, source))
    }

    #[inline]
    #[doc(hidden)]
    pub fn index_not_thread_safe(index: usize, source: NotThreadSafe) -> Self {
        Self::from(ErrorKind::IndexNotThreadSafe(index, source))
    }

    #[inline]
    #[doc(hidden)]
    pub fn field_not_thread_safe(name: &'static str, source: NotThreadSafe) -> Self {
        Self::from(ErrorKind::FieldNotThreadSafe(name, source))
    }
}

impl fmt::Display for PoolError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind.fmt(f)
    }
}

impl fmt::Debug for PoolError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind.fmt(f)
    }
}

impl core::error::Error for PoolError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self.kind {
            ErrorKind::BuildWriteConnection(ref source) => Some(source),
            ErrorKind::BuildReadConnection(_, ref source) => Some(source),
            ErrorKind::IndexPrepareFailed(_, ref source) => Some(source),
            ErrorKind::FieldPrepareFailed(_, ref source) => Some(source),
            ErrorKind::IndexNotThreadSafe(_, ref source) => Some(source),
            ErrorKind::FieldNotThreadSafe(_, ref source) => Some(source),
            _ => None,
        }
    }
}

#[derive(Debug)]
enum ErrorKind {
    #[cfg(feature = "std")]
    NotUtf8Path,
    #[cfg(feature = "std")]
    NulByteInPath,
    BuildWriteConnection(Error),
    BuildReadConnection(usize, Error),
    NoConnections,
    ZeroConcurrency,
    TooManyConnections(usize),
    IndexNotReadOnly(usize),
    FieldNotReadOnly(&'static str),
    IndexPrepareFailed(usize, Error),
    FieldPrepareFailed(&'static str, Error),
    IndexNotThreadSafe(usize, NotThreadSafe),
    FieldNotThreadSafe(&'static str, NotThreadSafe),
}

impl From<ErrorKind> for PoolError {
    #[inline]
    fn from(kind: ErrorKind) -> Self {
        Self { kind }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(feature = "std")]
            Self::NotUtf8Path => write!(f, "path is not valid UTF-8"),
            #[cfg(feature = "std")]
            Self::NulByteInPath => write!(f, "path contains a nul byte"),
            Self::BuildWriteConnection(_) => write!(f, "building write connection"),
            Self::BuildReadConnection(index, _) => write!(f, "building read connection #{index}"),
            Self::NoConnections => write!(f, "no connections available"),
            Self::ZeroConcurrency => write!(f, "read concurrency must be at least 1"),
            Self::TooManyConnections(concurrency) => {
                write!(
                    f,
                    "read concurrency {concurrency} exceeds the maximum of 64"
                )
            }
            Self::IndexNotReadOnly(index) => write!(f, "index {index} is not read-only"),
            Self::FieldNotReadOnly(name) => write!(f, "field {name} is not read-only"),
            Self::IndexPrepareFailed(index, _) => write!(f, "index {index} prepare failed"),
            Self::FieldPrepareFailed(name, _) => write!(f, "field {name} prepare failed"),
            Self::IndexNotThreadSafe(index, _) => write!(f, "index {index} is not thread-safe"),
            Self::FieldNotThreadSafe(name, _) => write!(f, "field {name} is not thread-safe"),
        }
    }
}

/// A pool of connections to the database.
///
/// Connections are represented by the custom types `R` and `W`, where `R` is a
/// read-only connection and `W` is a write connection.
///
/// The pool allows for asynchronously locking either a [`shared`] or an
/// [`exclusive`] lock on the underlying database.
///
/// Note that the pool is fair, so if there are multiple tasks waiting for a
/// lock, they will be granted in the order they were requested meaning a call
/// to [`exclusive`] will complete in the order it was called.
///
/// [`shared`]: Self::shared
/// [`exclusive`]: Self::exclusive
///
/// # Examples
///
/// The read side `R` must be a `#[sql(read_only)]` [`Statements`] collection so
/// that its connections can be used concurrently, while the write side `W` holds
/// the statements that mutate the database:
///
/// ```no_run
/// use sqll::{SendStatement, Statements, Pool, OpenOptions};
///
/// #[derive(Statements)]
/// #[sql(read_only)]
/// struct Read {
///     #[sql = "SELECT name, age FROM users ORDER BY age"]
///     all_users: SendStatement,
/// }
///
/// #[derive(Statements)]
/// struct Write {
///     #[sql = "INSERT INTO users (name, age) VALUES (?, ?)"]
///     insert_user: SendStatement,
/// }
///
/// let mut options = OpenOptions::new();
/// options.no_mutex().create();
/// let pool = Pool::<Read, Write>::new(options, "example.db", 4)?;
/// # Ok::<_, sqll::PoolError>(())
/// ```
///
/// Once built, wrap the pool in an [`Arc`] and use [`shared`] to read or
/// [`exclusive`] to write. Since SQLite is a blocking library, the actual
/// statement calls should be performed on a blocking thread (such as
/// [`tokio::task::spawn_blocking`]). See [`examples/pool.rs`] for a complete
/// asynchronous example.
///
/// [`Arc`]: alloc::sync::Arc
/// [`examples/pool.rs`]: https://github.com/udoprog/sqll/blob/main/examples/pool.rs
/// [`tokio::task::spawn_blocking`]: https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html
pub struct Pool<R, W> {
    /// The pool of read connections of size `concurrency`.
    read_pool: Box<[UnsafeCell<R>]>,
    /// Bit set of which inner connections are in use.
    used_read: AtomicU64,
    /// The single write connection.
    write: UnsafeCell<W>,
    /// The maximum number of concurrent connections to the database.
    concurrency: usize,
    /// Semaphore to limit the number of concurrent connections to the database.
    semaphore: Arc<Semaphore>,
}

unsafe impl<R, W> Send for Pool<R, W>
where
    R: Send,
    W: Send,
{
}
unsafe impl<R, W> Sync for Pool<R, W>
where
    R: Send,
    W: Send,
{
}

impl<R, W> Pool<R, W>
where
    R: IsReadOnly,
    W: Statements,
{
    /// Create a new pool of connections to the database.
    ///
    /// The `open_options` parameter is used to configure the connections to the
    /// database, and must be configured with the [`no_mutex`] option to allow
    /// connection to be converted into [`SendConnection`].
    ///
    /// [`no_mutex`]: OpenOptions::no_mutex
    /// [`SendConnection`]: crate::SendConnection
    ///
    /// This type implements `Send` and `Sync`, so it can be shared across
    /// threads.
    ///
    /// It constructs `read_concurrency` read connections to the database at the
    /// specified path. This database must be a filesystem database, since
    /// SQLite otherwise has no way of coordinating access, so specifying
    /// `:memory:` or a URI with `mode=memory` while it won't cause any error
    /// will result in every instance having their own private database.
    ///
    /// Note that the statements of `R` and `W` are prepared eagerly here, so any
    /// table they reference must already exist in the database by the time the
    /// pool is constructed.
    ///
    /// # Errors
    ///
    /// `read_concurrency` must be in the range `1..=64` (the upper bound is a
    /// consequence of tracking in-use read connections in a single 64-bit word),
    /// otherwise a [`PoolError`] is returned.
    ///
    /// # Examples
    ///
    /// See the [type-level documentation][Pool] for how to declare the `R` and
    /// `W` statement collections, and [`examples/pool.rs`] for a complete
    /// asynchronous example.
    ///
    /// [`examples/pool.rs`]: https://github.com/udoprog/sqll/blob/main/examples/pool.rs
    #[inline]
    #[cfg(feature = "std")]
    pub fn new(
        open_options: OpenOptions,
        path: impl AsRef<Path>,
        read_concurrency: usize,
    ) -> Result<Self, PoolError> {
        let path = path.as_ref();

        let Some(bytes) = path.to_str() else {
            return Err(PoolError::from(ErrorKind::NotUtf8Path));
        };

        let Ok(string) = CString::new(bytes) else {
            return Err(PoolError::from(ErrorKind::NulByteInPath));
        };

        Self::_new_c_str(open_options, &string, read_concurrency)
    }

    /// Same as [`new`] but takes a C string for the path.
    ///
    /// [`new`]: Self::new
    pub fn new_c_str(
        open_options: OpenOptions,
        path: &CStr,
        read_concurrency: usize,
    ) -> Result<Self, PoolError> {
        Self::_new_c_str(open_options, path, read_concurrency)
    }

    fn _new_c_str(
        open_options: OpenOptions,
        path: &CStr,
        concurrency: usize,
    ) -> Result<Self, PoolError> {
        if concurrency == 0 {
            return Err(PoolError::from(ErrorKind::ZeroConcurrency));
        }

        // The set of in-use read connections is tracked as a bit set in a single
        // `AtomicU64`, so we cannot represent more than 64 concurrent readers.
        if concurrency > 64 {
            return Err(PoolError::from(ErrorKind::TooManyConnections(concurrency)));
        }

        let write = open_options
            .clone()
            .read_write()
            .open_c_str(path)
            .map_err(ErrorKind::BuildWriteConnection)?;

        let write = W::build(write)?;

        let mut read_pool = Vec::with_capacity(concurrency);

        for index in 0..concurrency {
            let read = open_options
                .clone()
                .no_create()
                .read_only()
                .open_c_str(path)
                .map_err(move |error| ErrorKind::BuildReadConnection(index, error))?;

            let read = R::build(read)?;
            read_pool.push(UnsafeCell::new(read));
        }

        Ok(Self {
            read_pool: Box::from(read_pool),
            used_read: AtomicU64::new(0),
            write: UnsafeCell::new(write),
            concurrency,
            semaphore: Arc::new(Semaphore::new(concurrency)),
        })
    }

    /// Access the first read connection without acquiring a lock.
    ///
    /// This is fine since it requires mutable access to the pool, so no other
    /// thread can have access to it at the same time. This is primarily useful
    /// for setup performed before the pool is shared across tasks.
    pub fn as_read_mut(&mut self) -> &mut R {
        unsafe { &mut *self.read_pool[0].get() }
    }

    /// Access the write connection without acquiring a lock.
    ///
    /// This is fine since it requires mutable access to the pool, so no other
    /// thread can have access to it at the same time.
    pub fn as_write_mut(&mut self) -> &mut W {
        unsafe { &mut *self.write.get() }
    }

    /// Acquire a shared lock on the pool.
    ///
    /// This provides access to one of the read side `R` connections in the
    /// pool. The number of concurrent shared locks is limited by the
    /// `read_concurrency` the pool was constructed with, and if all connections
    /// are in use, this method will wait until one is available.
    pub async fn shared(self: Arc<Self>) -> Result<SharedGuard<R, W>, PoolError> {
        let result = self.semaphore.clone().acquire_owned().await;

        let permit = match result {
            Ok(permit) => permit,
            Err(AcquireError { .. }) => return Err(PoolError::from(ErrorKind::NoConnections)),
        };

        // Find the first unused connection in the used set, mark it as used,
        // and return it. We can do this without locking because each permit
        // corresponds to one bit in the used set.
        let index = loop {
            let value = self.used_read.load(Ordering::Acquire);
            let index = value.trailing_ones();

            if self
                .used_read
                .compare_exchange(
                    value,
                    value | (1 << index),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                break index as usize;
            }
        };

        // SAFETY: We have exclusive access to the read connection at `index`
        // because we have acquired a permit from the semaphore, so no other
        // thread can have access to it. We are also keeping the pool alive by
        // storing it in the guard.
        let read = unsafe {
            let slice = self.read_pool[index].get();
            NonNull::new_unchecked(&mut *slice)
        };

        Ok(SharedGuard {
            read,
            pool: self,
            index,
            _permit: permit,
        })
    }

    /// Acquire an exclusive lock on the pool.
    ///
    /// This provides access to the write side `W` of the connection and for the
    /// lifetime of [`ExclusiveGuard`] no other locks can be acquired.
    pub async fn exclusive(self: Arc<Self>) -> Result<ExclusiveGuard<R, W>, PoolError> {
        let result = self
            .semaphore
            .clone()
            .acquire_many_owned(self.concurrency as u32)
            .await;

        let permit = match result {
            Ok(permit) => permit,
            Err(AcquireError { .. }) => return Err(PoolError::from(ErrorKind::NoConnections)),
        };

        // SAFETY: We have exclusive access to the write connection because we
        // have acquired all permits in the semaphore, so no other thread can
        // have access to it. We are also keeping the pool alive by storing it
        // in the guard.
        let write = unsafe { NonNull::new_unchecked(&mut *self.write.get()) };

        Ok(ExclusiveGuard {
            write,
            _pool: self,
            _permit: permit,
        })
    }
}

/// An shared guard to the pool.
///
/// See [`Pool::shared`] for more details.
pub struct SharedGuard<R, W> {
    read: NonNull<R>,
    pool: Arc<Pool<R, W>>,
    index: usize,
    _permit: OwnedSemaphorePermit,
}

impl<R, W> Deref for SharedGuard<R, W> {
    type Target = R;

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { self.read.as_ref() }
    }
}

impl<R, W> DerefMut for SharedGuard<R, W> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.read.as_mut() }
    }
}

unsafe impl<R, W> Send for SharedGuard<R, W>
where
    R: Send,
    W: Send,
{
}

impl<R, W> Drop for SharedGuard<R, W> {
    fn drop(&mut self) {
        // Mark the connection as unused again. We can do this without locking
        // because each permit corresponds to one bit in the used set.
        self.pool
            .used_read
            .fetch_and(!(1 << self.index), Ordering::AcqRel);
    }
}

/// An exclusive guard to the pool.
///
/// See [`Pool::exclusive`] for more details.
pub struct ExclusiveGuard<R, W> {
    write: NonNull<W>,
    _pool: Arc<Pool<R, W>>,
    _permit: OwnedSemaphorePermit,
}

impl<R, W> Deref for ExclusiveGuard<R, W> {
    type Target = W;

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { self.write.as_ref() }
    }
}

impl<R, W> DerefMut for ExclusiveGuard<R, W> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.write.as_mut() }
    }
}

unsafe impl<R, W> Send for ExclusiveGuard<R, W> {}
