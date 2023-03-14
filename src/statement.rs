use std::mem::MaybeUninit;
use std::ptr;

use crate::error::{Error, Result};
use crate::utils;
use crate::value::{Type, Value};
use libc::{c_char, c_double, c_int};
use sqlite3_sys as ffi;

// https://sqlite.org/c3ref/c_static.html
macro_rules! transient(
    () => (::std::mem::transmute(!0 as *const ::libc::c_void));
);

/// A prepared statement.
#[repr(transparent)]
pub struct Statement {
    raw: ptr::NonNull<ffi::sqlite3_stmt>,
}

/// A prepared statement is `Send`.
unsafe impl Send for Statement {}

/// A state of a prepared statement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// There is a row available for reading.
    Row,
    /// The statement has been entirely evaluated.
    Done,
}

/// A type suitable for binding to a prepared statement.
pub trait Bindable {
    /// Bind to a parameter.
    ///
    /// The first parameter has index 1.
    fn bind(self, _: &mut Statement, _: usize) -> Result<()>;
}

/// A type suitable for reading from a prepared statement.
pub trait Readable: Sized {
    /// Read from a column.
    ///
    /// The first column has index 0.
    fn read(_: &Statement, _: usize) -> Result<Self>;
}

impl Statement {
    /// Construct a new statement.
    #[inline]
    pub(crate) fn new<T>(handle: *mut ffi::sqlite3, statement: T) -> Result<Statement>
    where
        T: AsRef<str>,
    {
        let mut raw = MaybeUninit::uninit();
        let statement = statement.as_ref();

        unsafe {
            sqlite3_try! {
                handle,
                ffi::sqlite3_prepare_v2(
                    handle,
                    statement.as_bytes().as_ptr() as *const _,
                    statement.len() as c_int,
                    raw.as_mut_ptr(),
                    ptr::null_mut(),
                )
            };
        }

        Ok(Statement {
            raw: unsafe { ptr::NonNull::new_unchecked(raw.assume_init()) },
        })
    }

    /// Bind a value to a parameter by index.
    ///
    /// The first parameter has index 1.
    #[inline]
    pub fn bind<T: Bindable>(&mut self, i: usize, value: T) -> Result<()> {
        value.bind(self, i)
    }

    /// Bind a value to a parameter by name.
    ///
    /// # Examples
    ///
    /// ```
    /// # let connection = sqlite_ll::Connection::open(":memory:")?;
    /// # connection.execute("CREATE TABLE users (name STRING)");
    /// let mut statement = unsafe { connection.prepare("SELECT * FROM users WHERE name = :name")? };
    /// statement.bind_by_name(":name", "Bob")?;
    /// # Ok::<(), sqlite_ll::Error>(())
    /// ```
    pub fn bind_by_name<T: Bindable>(&mut self, name: &str, value: T) -> Result<()> {
        if let Some(i) = self.parameter_index(name)? {
            self.bind(i, value)?;
            Ok(())
        } else {
            Err(Error::from_code(ffi::SQLITE_MISMATCH))
        }
    }

    /// Return the number of columns.
    #[inline]
    pub fn column_count(&self) -> usize {
        unsafe { ffi::sqlite3_column_count(self.raw.as_ptr()) as usize }
    }

    /// Return the name of a column.
    ///
    /// The first column has index 0.
    #[inline]
    pub fn column_name(&self, i: usize) -> Result<&str> {
        debug_assert!(i < self.column_count(), "the index is out of range");
        unsafe {
            let pointer = ffi::sqlite3_column_name(self.raw.as_ptr(), i as c_int);

            if pointer.is_null() {
                let handle = ffi::sqlite3_db_handle(self.raw.as_ptr());
                let code = ffi::sqlite3_errcode(handle);
                return Err(Error::from_code(code));
            }

            utils::cstr_to_str(pointer)
        }
    }

    /// Return column names.
    #[inline]
    pub fn column_names(&self) -> Result<Vec<&str>> {
        (0..self.column_count())
            .map(|i| self.column_name(i))
            .collect()
    }

    /// Return the type of a column.
    ///
    /// The first column has index 0. The type becomes available after taking a step.
    pub fn column_type(&self, i: usize) -> Type {
        debug_assert!(i < self.column_count(), "the index is out of range");

        match unsafe { ffi::sqlite3_column_type(self.raw.as_ptr(), i as c_int) } {
            ffi::SQLITE_BLOB => Type::Blob,
            ffi::SQLITE_FLOAT => Type::Float,
            ffi::SQLITE_INTEGER => Type::Integer,
            ffi::SQLITE_TEXT => Type::Text,
            ffi::SQLITE_NULL => Type::Null,
            _ => unreachable!(),
        }
    }

    /// Step to the next state.
    ///
    /// The function should be called multiple times until `State::Done` is
    /// reached in order to evaluate the statement entirely.
    pub fn step(&mut self) -> Result<State> {
        unsafe {
            match ffi::sqlite3_step(self.raw.as_ptr()) {
                ffi::SQLITE_ROW => Ok(State::Row),
                ffi::SQLITE_DONE => Ok(State::Done),
                _ => {
                    let handle = ffi::sqlite3_db_handle(self.raw.as_ptr());
                    let code = ffi::sqlite3_errcode(handle);
                    Err(Error::from_code(code))
                }
            }
        }
    }

    /// Return the index for a named parameter if exists.
    ///
    /// # Examples
    ///
    /// ```
    /// # let connection = sqlite_ll::Connection::open(":memory:")?;
    /// # connection.execute("CREATE TABLE users (name STRING)");
    /// let statement = unsafe { connection.prepare("SELECT * FROM users WHERE name = :name")? };
    /// assert_eq!(statement.parameter_index(":name")?, Some(1));
    /// assert_eq!(statement.parameter_index(":asdf")?, None);
    /// # Ok::<(), sqlite_ll::Error>(())
    /// ```
    #[inline]
    pub fn parameter_index(&self, parameter: &str) -> Result<Option<usize>> {
        let index = unsafe {
            ffi::sqlite3_bind_parameter_index(
                self.raw.as_ptr(),
                utils::string_to_cstring(parameter)?.as_ptr(),
            )
        };

        match index {
            0 => Ok(None),
            _ => Ok(Some(index as usize)),
        }
    }

    /// Read a value from a column.
    ///
    /// The first column has index 0.
    #[inline]
    pub fn read<T: Readable>(&self, i: usize) -> Result<T> {
        debug_assert!(i < self.column_count(), "the index is out of range");
        Readable::read(self, i)
    }

    /// Reset the statement.
    #[inline]
    pub fn reset(&mut self) -> Result<()> {
        unsafe { ffi::sqlite3_reset(self.raw.as_ptr()) };
        Ok(())
    }
}

impl Drop for Statement {
    #[inline]
    fn drop(&mut self) {
        unsafe { ffi::sqlite3_finalize(self.raw.as_ptr()) };
    }
}

impl<'a> Bindable for &'a Value {
    fn bind(self, statement: &mut Statement, i: usize) -> Result<()> {
        match self {
            Value::Blob(value) => value.as_slice().bind(statement, i),
            Value::Float(value) => value.bind(statement, i),
            Value::Integer(value) => value.bind(statement, i),
            Value::Text(value) => value.as_str().bind(statement, i),
            Value::Null => ().bind(statement, i),
        }
    }
}

impl<'a> Bindable for &'a [u8] {
    #[inline]
    fn bind(self, statement: &mut Statement, i: usize) -> Result<()> {
        debug_assert!(i > 0, "the indexing starts from 1");

        unsafe {
            sqlite3_try! {
                ffi::sqlite3_db_handle(statement.raw.as_ptr()),
                ffi::sqlite3_bind_blob(
                    statement.raw.as_ptr(),
                    i as c_int,
                    self.as_ptr() as *const _,
                    self.len() as c_int,
                    transient!(),
                )
            };
        }

        Ok(())
    }
}

impl Bindable for f64 {
    #[inline]
    fn bind(self, statement: &mut Statement, i: usize) -> Result<()> {
        debug_assert!(i > 0, "the indexing starts from 1");

        unsafe {
            sqlite3_try! {
                ffi::sqlite3_db_handle(statement.raw.as_ptr()),
                ffi::sqlite3_bind_double(
                    statement.raw.as_ptr(),
                    i as c_int,
                    self as c_double
                )
            };
        }

        Ok(())
    }
}

impl Bindable for i64 {
    #[inline]
    fn bind(self, statement: &mut Statement, i: usize) -> Result<()> {
        debug_assert!(i > 0, "the indexing starts from 1");

        unsafe {
            sqlite3_try! {
                ffi::sqlite3_db_handle(statement.raw.as_ptr()),
                ffi::sqlite3_bind_int64(
                    statement.raw.as_ptr(),
                    i as c_int,
                    self as ffi::sqlite3_int64
                )
            };
        }

        Ok(())
    }
}

impl<'a> Bindable for &'a str {
    #[inline]
    fn bind(self, statement: &mut Statement, i: usize) -> Result<()> {
        debug_assert!(i > 0, "the indexing starts from 1");

        unsafe {
            sqlite3_try! {
                ffi::sqlite3_db_handle(statement.raw.as_ptr()),
                ffi::sqlite3_bind_text(
                    statement.raw.as_ptr(),
                    i as c_int,
                    self.as_ptr() as *const _,
                    self.len() as c_int,
                    transient!(),
                )
            };
        }

        Ok(())
    }
}

impl Bindable for () {
    #[inline]
    fn bind(self, statement: &mut Statement, i: usize) -> Result<()> {
        debug_assert!(i > 0, "the indexing starts from 1");

        unsafe {
            sqlite3_try! {
                ffi::sqlite3_db_handle(statement.raw.as_ptr()),
                ffi::sqlite3_bind_null(statement.raw.as_ptr(), i as c_int)
            };
        }

        Ok(())
    }
}

impl<T> Bindable for Option<T>
where
    T: Bindable,
{
    #[inline]
    fn bind(self, statement: &mut Statement, i: usize) -> Result<()> {
        debug_assert!(i > 0, "the indexing starts from 1");
        match self {
            Some(inner) => Bindable::bind(inner, statement, i),
            None => Bindable::bind((), statement, i),
        }
    }
}

impl Readable for Value {
    fn read(statement: &Statement, i: usize) -> Result<Self> {
        Ok(match statement.column_type(i) {
            Type::Blob => Value::Blob(Readable::read(statement, i)?),
            Type::Float => Value::Float(Readable::read(statement, i)?),
            Type::Integer => Value::Integer(Readable::read(statement, i)?),
            Type::Text => Value::Text(Readable::read(statement, i)?),
            Type::Null => Value::Null,
        })
    }
}

impl Readable for f64 {
    #[inline]
    fn read(statement: &Statement, i: usize) -> Result<Self> {
        Ok(unsafe { ffi::sqlite3_column_double(statement.raw.as_ptr(), i as c_int) })
    }
}

impl Readable for i64 {
    #[inline]
    fn read(statement: &Statement, i: usize) -> Result<Self> {
        Ok(unsafe { ffi::sqlite3_column_int64(statement.raw.as_ptr(), i as c_int) })
    }
}

impl Readable for String {
    #[inline]
    fn read(statement: &Statement, i: usize) -> Result<Self> {
        unsafe {
            let pointer = ffi::sqlite3_column_text(statement.raw.as_ptr(), i as c_int);

            if pointer.is_null() {
                return Err(Error::from_code(ffi::SQLITE_MISMATCH));
            }

            Ok(utils::cstr_to_str(pointer as *const c_char)?.to_owned())
        }
    }
}

impl Readable for Vec<u8> {
    #[inline]
    fn read(statement: &Statement, i: usize) -> Result<Self> {
        unsafe {
            let pointer = ffi::sqlite3_column_blob(statement.raw.as_ptr(), i as c_int);
            if pointer.is_null() {
                return Ok(vec![]);
            }
            let count = ffi::sqlite3_column_bytes(statement.raw.as_ptr(), i as c_int) as usize;
            let mut buffer = Vec::with_capacity(count);
            ptr::copy_nonoverlapping(pointer as *const u8, buffer.as_mut_ptr(), count);
            buffer.set_len(count);
            Ok(buffer)
        }
    }
}

/// A helper to read at most a fixed number of `N` bytes from a column. This
/// allocates the storage for the bytes read on the stack.
pub struct FixedBytes<const N: usize> {
    /// Storage to read to.
    data: [MaybeUninit<u8>; N],
    /// Number of bytes initialized.
    init: usize,
}

impl<const N: usize> FixedBytes<N> {
    /// Coerce into the underlying bytes if all of them have been initialized.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use sqlite_ll::{Connection, State, FixedBytes};
    ///
    /// let c: Connection = todo!();
    /// let stmt = unsafe { c.prepare("SELECT id FROM users")? };
    ///
    /// while let State::Row = stmt.step()? {
    ///     let id = stmt.read::<FixedBytes<16>>(0)?;
    ///
    ///     // Note: we have to check the result of `into_bytes` to ensure that the field contained exactly 16 bytes.
    ///     let bytes: [u8; 16] = match id.into_bytes() {
    ///         Some(bytes) => bytes,
    ///         None => continue,
    ///     };
    ///
    ///     /* use bytes */
    /// }
    /// # Ok::<_, sqlite_ll::Error>(())
    /// ```
    pub fn into_bytes(self) -> Option<[u8; N]> {
        if self.init == N {
            // SAFETY: All of the bytes in the sequence have been initialized
            // and can be safety transmuted.
            //
            // Method of transmuting comes from the implementation of
            // `MaybeUninit::array_assume_init` which is not yet stable.
            unsafe { Some((&self.data as *const _ as *const [u8; N]).read()) }
        } else {
            None
        }
    }

    /// Coerce into the slice of initialized memory which is present.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use sqlite_ll::{Connection, State, FixedBytes};
    ///
    /// let c: Connection = todo!();
    /// let stmt = unsafe { c.prepare("SELECT id FROM users")? };
    ///
    /// while let State::Row = stmt.step()? {
    ///     let id = stmt.read::<FixedBytes<16>>(0)?;
    ///     let bytes: &[u8] = id.as_bytes();
    ///
    ///     /* use bytes */
    /// }
    /// # Ok::<_, sqlite_ll::Error>(())
    /// ```
    pub fn as_bytes(&self) -> &[u8] {
        if self.init == 0 {
            return &[];
        }

        // SAFETY: We've asserted that `initialized` accounts for the number of
        // bytes that have been initialized.
        unsafe { std::slice::from_raw_parts(self.data.as_ptr() as *const u8, self.init) }
    }
}

impl<const N: usize> Readable for FixedBytes<N> {
    #[inline]
    fn read(statement: &Statement, i: usize) -> Result<Self> {
        let mut bytes = FixedBytes {
            // SAFETY: this is safe as per `MaybeUninit::uninit_array`, which isn't stable (yet).
            data: unsafe { MaybeUninit::<[MaybeUninit<u8>; N]>::uninit().assume_init() },
            init: 0,
        };

        unsafe {
            let pointer = ffi::sqlite3_column_blob(statement.raw.as_ptr(), i as c_int);

            if pointer.is_null() {
                return Ok(bytes);
            }

            let count = ffi::sqlite3_column_bytes(statement.raw.as_ptr(), i as c_int) as usize;
            let copied = usize::min(N, count);

            ptr::copy_nonoverlapping(
                pointer as *const u8,
                bytes.data.as_mut_ptr() as *mut u8,
                copied,
            );

            bytes.init = copied;
            Ok(bytes)
        }
    }
}

impl<T> Readable for Option<T>
where
    T: Readable,
{
    #[inline]
    fn read(statement: &Statement, i: usize) -> Result<Self> {
        if statement.column_type(i) == Type::Null {
            Ok(None)
        } else {
            T::read(statement, i).map(Some)
        }
    }
}
