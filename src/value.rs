/// The type of a value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Type {
    Blob,
    Float,
    Integer,
    Text,
    Null,
}

/// A dynamic value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Blob(Vec<u8>),
    Float(f64),
    Integer(i64),
    Text(String),
    Null,
}

impl Value {
    /// Return the binary data if the value is `Binary`.
    #[inline]
    pub fn as_blob(&self) -> Option<&[u8]> {
        if let &Value::Blob(ref value) = self {
            return Some(value);
        }
        None
    }

    /// Return the floating-point number if the value is `Float`.
    #[inline]
    pub fn as_float(&self) -> Option<f64> {
        if let &Value::Float(value) = self {
            return Some(value);
        }
        None
    }

    /// Return the integer number if the value is `Integer`.
    #[inline]
    pub fn as_integer(&self) -> Option<i64> {
        if let &Value::Integer(value) = self {
            return Some(value);
        }
        None
    }

    /// Return the string if the value is `String`.
    #[inline]
    pub fn as_string(&self) -> Option<&str> {
        if let &Value::Text(ref value) = self {
            return Some(value);
        }
        None
    }

    /// Return the type.
    pub fn kind(&self) -> Type {
        match self {
            Value::Blob(_) => Type::Blob,
            Value::Float(_) => Type::Float,
            Value::Integer(_) => Type::Integer,
            Value::Text(_) => Type::Text,
            Value::Null => Type::Null,
        }
    }
}
