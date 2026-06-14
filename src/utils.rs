use core::ffi::CStr;

use crate::Text;

/// Helper to evaluate sqlite3 statements.
macro_rules! __sqlite3_try {
    ($db:expr, $expr:expr) => {{
        let code = $expr;

        if code != $crate::ffi::SQLITE_OK {
            return Err($crate::Error::new(
                $crate::Code::new(code),
                $db.error_message(),
            ));
        }
    }};
}

pub(crate) use __sqlite3_try as sqlite3_try;

macro_rules! __repeat {
    ($macro:path) => {
        $macro!(1, A a 0 1);
        $macro!(2, A a 0 1, B b 1 2);
        $macro!(3, A a 0 1, B b 1 2, C c 2 3);
        $macro!(4, A a 0 1, B b 1 2, C c 2 3, D d 3 4);
        $macro!(5, A a 0 1, B b 1 2, C c 2 3, D d 3 4, E e 4 5);
        $macro!(6, A a 0 1, B b 1 2, C c 2 3, D d 3 4, E e 4 5, F f 5 6);
        $macro!(7, A a 0 1, B b 1 2, C c 2 3, D d 3 4, E e 4 5, F f 5 6, G g 6 7);
        $macro!(8, A a 0 1, B b 1 2, C c 2 3, D d 3 4, E e 4 5, F f 5 6, G g 6 7, H h 7 8);
        $macro!(9, A a 0 1, B b 1 2, C c 2 3, D d 3 4, E e 4 5, F f 5 6, G g 6 7, H h 7 8, I i 8 9);
        $macro!(10, A a 0 1, B b 1 2, C c 2 3, D d 3 4, E e 4 5, F f 5 6, G g 6 7, H h 7 8, I i 8 9, J j 9 10);
        $macro!(11, A a 0 1, B b 1 2, C c 2 3, D d 3 4, E e 4 5, F f 5 6, G g 6 7, H h 7 8, I i 8 9, J j 9 10, K k 10 11);
        $macro!(12, A a 0 1, B b 1 2, C c 2 3, D d 3 4, E e 4 5, F f 5 6, G g 6 7, H h 7 8, I i 8 9, J j 9 10, K k 10 11, L l 11 12);
        $macro!(13, A a 0 1, B b 1 2, C c 2 3, D d 3 4, E e 4 5, F f 5 6, G g 6 7, H h 7 8, I i 8 9, J j 9 10, K k 10 11, L l 11 12, M m 12 13);
        $macro!(14, A a 0 1, B b 1 2, C c 2 3, D d 3 4, E e 4 5, F f 5 6, G g 6 7, H h 7 8, I i 8 9, J j 9 10, K k 10 11, L l 11 12, M m 12 13, N n 13 14);
        $macro!(15, A a 0 1, B b 1 2, C c 2 3, D d 3 4, E e 4 5, F f 5 6, G g 6 7, H h 7 8, I i 8 9, J j 9 10, K k 10 11, L l 11 12, M m 12 13, N n 13 14, O o 14 15);
    };
}

/// Repeat a macro to build a tuple implementation.
///
/// This supports up to 15 elements.
pub(crate) use __repeat as repeat;

/// Coerce a null-terminated string into UTF-8, returning `None` if the pointer
/// is null.
pub(crate) unsafe fn c_to_text<'a>(ptr: *const i8) -> Option<&'a Text> {
    if ptr.is_null() {
        return None;
    }

    // SAFETY: The one assumption we are allowed to make about strings is that
    // they are null-terminated.
    let c_str = unsafe { CStr::from_ptr(ptr) };
    Some(Text::new(c_str.to_bytes()))
}

pub(crate) unsafe fn c_to_error_text(ptr: *const i8) -> &'static Text {
    // NB: This is the same message as set by sqlite.
    static DEFAULT_MESSAGE: &Text = Text::from_bytes(b"not an error");
    unsafe { c_to_text(ptr).unwrap_or(DEFAULT_MESSAGE) }
}
