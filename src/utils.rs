use std::ffi::{CStr, CString};
use std::path::Path;

use crate::error::Result;
use libc::c_char;

/// Helper to run sqlite3 statement.
macro_rules! sqlite3_try {
    ($c:expr, $expr:expr) => {
        match $expr {
            ::sqlite3_sys::SQLITE_OK => (),
            _ => {
                let code = sqlite3_sys::sqlite3_errcode($c);

                let m = sqlite3_sys::sqlite3_errmsg($c);

                let message = if !m.is_null() {
                    crate::utils::cstr_to_str(m).map(Box::<str>::from).ok()
                } else {
                    None
                };

                return Err(crate::error::Error::new(code, message));
            }
        }
    };
}

/// Convert a c-string into a rust string.
pub(crate) unsafe fn cstr_to_str<'a>(s: *const c_char) -> Result<&'a str> {
    match CStr::from_ptr(s).to_str() {
        Ok(s) => Ok(s),
        Err(..) => Err(crate::error::Error::from_code(sqlite3_sys::SQLITE_MISUSE)),
    }
}

/// Convert a rust string into a c-string.
///
/// This needs to allocate in order to append a null character at the end of the
/// string.
pub(crate) fn string_to_cstring(s: &str) -> Result<CString> {
    match CString::new(s) {
        Ok(string) => Ok(string),
        _ => return Err(crate::error::Error::from_code(sqlite3_sys::SQLITE_MISUSE)),
    }
}

#[cfg(unix)]
pub(crate) fn path_to_cstring(p: &Path) -> Result<CString> {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let p: &OsStr = p.as_ref();

    match CString::new(p.as_bytes()) {
        Ok(string) => Ok(string),
        Err(..) => return Err(crate::error::Error::from_code(sqlite3_sys::SQLITE_MISUSE)),
    }
}

#[cfg(not(unix))]
pub(crate) fn path_to_cstring(p: &Path) -> Result<CString> {
    let s = match p.to_str() {
        Some(s) => s,
        None => return Err(crate::error::Error::from_code(sqlite3_sys::SQLITE_MISUSE)),
    };

    match CString::new(p.as_bytes()) {
        Ok(string) => Ok(string),
        Err(..) => return Err(crate::error::Error::from_code(sqlite3_sys::SQLITE_MISUSE)),
    }
}
