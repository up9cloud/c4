//! The dynamic config value: the [`Value`] enum, its accessors and its
//! serde bridges. `Value::format_id` names the parser a value came from
//! (shown by traces); typed variants beyond JSON's set (datetime, IPs,
//! MACs, …) are produced by the feature-gated table value parsers.

use std::collections::BTreeMap;

/// Dynamic config value.
///
/// Non-exhaustive: new variants may come without a breaking release, so
/// a `match` needs a `_` arm.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Uint(u64),
    /// 128-bit signed — only from the table `i128` type (never `auto`,
    /// which stops at `i64`/`u64`/`f64`). Serializes as a plain number
    /// when it fits `i64` or `u64`, otherwise as its decimal **string**
    /// (JSON has no wider integer); [`format_id`](Value::format_id) still reports
    /// `i128`.
    Int128(i128),
    /// 128-bit unsigned — only from the table `u128` type. Serializes as
    /// a plain number when it fits `u64`, otherwise as its decimal
    /// **string**; [`format_id`](Value::format_id) still reports `u128`.
    Uint128(u128),
    Float(f64),
    String(String),
    /// An RFC 3339-style date or datetime, kept as its text. Produced by
    /// toml datetime literals, the table `dt` type and the `auto` guess —
    /// all gated by the `datetime` feature. Serializes as the string itself (so
    /// a serialize→deserialize round trip through a format without a
    /// datetime type yields a `String`).
    DateTime(String),
    /// `YYYY-MM-DD`, kept as its text (table `date` type, `date` feature).
    Date(String),
    /// `hh:mm:ss[.fraction]`, kept as its text (table `time` type, `time`
    /// feature).
    Time(String),
    /// An IPv4 address, kept as its validated **canonical** text (table
    /// `ipv4` type, `ipv4` feature) — text like every typed value, so
    /// [`as_str`](Value::as_str) works; [`as_ip`](Value::as_ip) gives the
    /// address. The `inet` type (`inet` feature) also produces this or
    /// [`Value::Ipv6`], whichever family parses.
    Ipv4(String),
    /// An IPv6 address, kept as its validated canonical text
    /// (`2001:0DB8::1` → `2001:db8::1`; table `ipv6` type, `ipv6` feature).
    Ipv6(String),
    /// A host address **with a netmask** (`10.0.0.1/24`), kept as its
    /// validated text — PostgreSQL `inet` semantics, host bits below the
    /// mask may be set (table `inet` type, `inet` feature; a bare `inet`
    /// address maps onto [`Value::Ipv4`]/[`Value::Ipv6`] instead).
    Inet(String),
    /// A network — optional netmask defaulting to the full address
    /// length, host bits below the mask must be zero — kept as its
    /// validated text; PostgreSQL `cidr` semantics (table `cidr` type,
    /// `cidr` feature).
    Cidr(String),
    /// 6-byte MAC address, kept as its validated text (table `macaddr`
    /// type, `macaddr` feature).
    MacAddr(String),
    /// 8-byte MAC address, kept as its validated text (table `macaddr8`
    /// type, `macaddr8` feature).
    MacAddr8(String),
    /// Hyphenated `8-4-4-4-12` UUID, kept as its validated text (table
    /// `uuid` type, `uuid` feature).
    Uuid(String),
    Array(Vec<Value>),
    Object(BTreeMap<String, Value>),
}

static NULL: Value = Value::Null;

impl Value {
    /// Object member by key; `None` when this is not an object or the key
    /// is absent.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(map) => map.get(key),
            _ => None,
        }
    }

    /// Array element by index; `None` when this is not an array or the
    /// index is out of bounds.
    pub fn get_index(&self, index: usize) -> Option<&Value> {
        match self {
            Value::Array(items) => items.get(index),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::Uint(u) => i64::try_from(*u).ok(),
            Value::Int128(i) => i64::try_from(*i).ok(),
            Value::Uint128(u) => i64::try_from(*u).ok(),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Value::Uint(u) => Some(*u),
            Value::Int(i) => u64::try_from(*i).ok(),
            Value::Int128(i) => u64::try_from(*i).ok(),
            Value::Uint128(u) => u64::try_from(*u).ok(),
            _ => None,
        }
    }

    /// Any integer value as an `i128`.
    pub fn as_i128(&self) -> Option<i128> {
        match self {
            Value::Int(i) => Some(i128::from(*i)),
            Value::Uint(u) => Some(i128::from(*u)),
            Value::Int128(i) => Some(*i),
            Value::Uint128(u) => i128::try_from(*u).ok(),
            _ => None,
        }
    }

    /// Any integer value as a `u128`.
    pub fn as_u128(&self) -> Option<u128> {
        match self {
            Value::Uint(u) => Some(u128::from(*u)),
            Value::Int(i) => u128::try_from(*i).ok(),
            Value::Int128(i) => u128::try_from(*i).ok(),
            Value::Uint128(u) => Some(*u),
            _ => None,
        }
    }

    /// Any numeric value as a float.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            Value::Int(i) => Some(*i as f64),
            Value::Uint(u) => Some(*u as f64),
            Value::Int128(i) => Some(*i as f64),
            Value::Uint128(u) => Some(*u as f64),
            _ => None,
        }
    }

    /// The text of a string — or of any typed value (datetime, IP, MAC,
    /// uuid, …), which all keep their text form.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s)
            | Value::DateTime(s)
            | Value::Date(s)
            | Value::Time(s)
            | Value::Ipv4(s)
            | Value::Ipv6(s)
            | Value::Inet(s)
            | Value::Cidr(s)
            | Value::MacAddr(s)
            | Value::MacAddr8(s)
            | Value::Uuid(s) => Some(s),
            _ => None,
        }
    }

    /// Convert this value into any serde type, with the same rules as
    /// [`Loader::load`](crate::Loader::load): strings coerce into numbers
    /// and bools (and back), and an error names the key path. It consumes
    /// the value; take one section of a loaded config with `.clone()`:
    ///
    /// ```
    /// #[derive(serde::Deserialize)]
    /// struct Db {
    ///     port: u16,
    /// }
    ///
    /// # fn main() -> Result<(), c4::Error> {
    /// let config = c4::Value::from(std::collections::BTreeMap::from([(
    ///     "db".to_owned(),
    ///     c4::Value::from(std::collections::BTreeMap::from([(
    ///         "port".to_owned(),
    ///         c4::Value::from("5432"),
    ///     )])),
    /// )]));
    /// let db: Db = config["db"].clone().into_typed()?;
    /// assert_eq!(db.port, 5432);
    /// # Ok(())
    /// # }
    /// ```
    pub fn into_typed<T: serde::de::DeserializeOwned>(self) -> crate::Result<T> {
        T::deserialize(crate::de::ValueDeserializer::new(self))
            .map_err(|e| crate::Error::Deserialize(e.to_string()))
    }

    /// The address of an [`Ipv4`](Value::Ipv4) or [`Ipv6`](Value::Ipv6)
    /// value; `None` for anything else, a string that looks like one
    /// included.
    pub fn as_ip(&self) -> Option<std::net::IpAddr> {
        match self {
            Value::Ipv4(s) | Value::Ipv6(s) => s.parse().ok(),
            _ => None,
        }
    }

    /// The format id of this value — which parser produced it: `null`,
    /// `bool`, `i64`, `u64`, `i128`, `u128`, `f64`, `str`, the typed ids (`dt`, `date`,
    /// `time`, `ipv4`, `ipv6`, `cidr`, `macaddr`, `macaddr8`, `uuid`),
    /// `arr:<t>` for homogeneous scalar arrays, `arr` for other arrays,
    /// `object` for objects. Integer widths normalize to the stored type
    /// (an `i8` cell stores an `i64`, so it reports `i64`), and `inet`
    /// reports as the family it parsed into. Shown by `trace()` as the
    /// `format` field of every serialized leaf.
    pub fn format_id(&self) -> std::borrow::Cow<'static, str> {
        use std::borrow::Cow;
        Cow::Borrowed(match self {
            Value::Null => "null",
            Value::Bool(_) => "bool",
            Value::Int(_) => "i64",
            Value::Uint(_) => "u64",
            Value::Int128(_) => "i128",
            Value::Uint128(_) => "u128",
            Value::Float(_) => "f64",
            Value::String(_) => "str",
            Value::DateTime(_) => "dt",
            Value::Date(_) => "date",
            Value::Time(_) => "time",
            Value::Ipv4(_) => "ipv4",
            Value::Ipv6(_) => "ipv6",
            Value::Inet(_) => "inet",
            Value::Cidr(_) => "cidr",
            Value::MacAddr(_) => "macaddr",
            Value::MacAddr8(_) => "macaddr8",
            Value::Uuid(_) => "uuid",
            Value::Object(_) => "object",
            Value::Array(items) => {
                let mut ids = items.iter().map(Value::format_id);
                return match ids.next() {
                    Some(first)
                        if first != "object"
                            && !first.starts_with("arr")
                            && ids.all(|id| id == first) =>
                    {
                        Cow::Owned(format!("arr:{first}"))
                    }
                    _ => Cow::Borrowed("arr"),
                };
            }
        })
    }

    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match self {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&BTreeMap<String, Value>> {
        match self {
            Value::Object(map) => Some(map),
            _ => None,
        }
    }
}

/// `value["key"]` — yields `Value::Null` for missing keys or non-objects,
/// so lookups chain without panicking: `value["db"]["port"].as_u64()`.
// Conversions into a `Value`, for custom formats, tests and overrides.
macro_rules! from_scalar {
    ($($ty:ty => $variant:ident via $conv:ty),* $(,)?) => {$(
        impl From<$ty> for Value {
            fn from(v: $ty) -> Value {
                Value::$variant(<$conv>::from(v))
            }
        }
    )*};
}

from_scalar! {
    bool => Bool via bool,
    i8 => Int via i64, i16 => Int via i64, i32 => Int via i64, i64 => Int via i64,
    u8 => Uint via u64, u16 => Uint via u64, u32 => Uint via u64, u64 => Uint via u64,
    i128 => Int128 via i128, u128 => Uint128 via u128,
    f32 => Float via f64, f64 => Float via f64,
    String => String via String,
}

impl From<&str> for Value {
    fn from(v: &str) -> Value {
        Value::String(v.to_owned())
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    /// `None` is [`Value::Null`].
    fn from(v: Option<T>) -> Value {
        v.map_or(Value::Null, Into::into)
    }
}

impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(v: Vec<T>) -> Value {
        Value::Array(v.into_iter().map(Into::into).collect())
    }
}

impl From<BTreeMap<String, Value>> for Value {
    fn from(v: BTreeMap<String, Value>) -> Value {
        Value::Object(v)
    }
}

// Comparisons with plain values, both ways, through the `as_*` accessors:
// text matches any typed text, integers match across widths, and a float
// matches any number.
macro_rules! eq_via {
    ($($ty:ty => |$v:ident, $o:ident| $cmp:expr),* $(,)?) => {$(
        impl PartialEq<$ty> for Value {
            fn eq(&self, other: &$ty) -> bool {
                let ($v, $o) = (self, other);
                $cmp
            }
        }
        impl PartialEq<Value> for $ty {
            fn eq(&self, other: &Value) -> bool {
                other == self
            }
        }
    )*};
}

eq_via! {
    str => |v, o| v.as_str() == Some(o),
    &str => |v, o| v.as_str() == Some(*o),
    String => |v, o| v.as_str() == Some(o.as_str()),
    bool => |v, o| v.as_bool() == Some(*o),
    i32 => |v, o| v.as_i128() == Some(i128::from(*o)),
    i64 => |v, o| v.as_i128() == Some(i128::from(*o)),
    u32 => |v, o| v.as_i128() == Some(i128::from(*o)),
    u64 => |v, o| v.as_i128() == Some(i128::from(*o)),
    f64 => |v, o| v.as_f64() == Some(*o),
}

/// Compact JSON: typed values print as their text, non-finite floats as
/// `null` (like any JSON serializer). Use [`Value::as_str`] for a string's
/// raw text.
impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn text(f: &mut std::fmt::Formatter<'_>, s: &str) -> std::fmt::Result {
            f.write_str("\"")?;
            for c in s.chars() {
                match c {
                    '"' => f.write_str("\\\"")?,
                    '\\' => f.write_str("\\\\")?,
                    '\n' => f.write_str("\\n")?,
                    '\r' => f.write_str("\\r")?,
                    '\t' => f.write_str("\\t")?,
                    c if c < ' ' => write!(f, "\\u{:04x}", c as u32)?,
                    c => write!(f, "{c}")?,
                }
            }
            f.write_str("\"")
        }
        match self {
            Value::Null => f.write_str("null"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Int(i) => write!(f, "{i}"),
            Value::Uint(u) => write!(f, "{u}"),
            Value::Int128(i) => write!(f, "{i}"),
            Value::Uint128(u) => write!(f, "{u}"),
            Value::Float(x) if x.is_finite() => write!(f, "{x:?}"),
            Value::Float(_) => f.write_str("null"),
            Value::Array(items) => {
                f.write_str("[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        f.write_str(",")?;
                    }
                    write!(f, "{item}")?;
                }
                f.write_str("]")
            }
            Value::Object(map) => {
                f.write_str("{")?;
                for (i, (key, value)) in map.iter().enumerate() {
                    if i > 0 {
                        f.write_str(",")?;
                    }
                    text(f, key)?;
                    write!(f, ":{value}")?;
                }
                f.write_str("}")
            }
            other => text(f, other.as_str().unwrap_or_default()),
        }
    }
}

impl std::ops::Index<&str> for Value {
    type Output = Value;

    fn index(&self, key: &str) -> &Value {
        self.get(key).unwrap_or(&NULL)
    }
}

/// `value[0]` — yields `Value::Null` out of bounds or on non-arrays.
impl std::ops::Index<usize> for Value {
    type Output = Value;

    fn index(&self, index: usize) -> &Value {
        self.get_index(index).unwrap_or(&NULL)
    }
}

impl<'de> serde::Deserialize<'de> for Value {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ValueVisitor;

        impl<'de> serde::de::Visitor<'de> for ValueVisitor {
            type Value = Value;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("any config value")
            }

            fn visit_bool<E>(self, v: bool) -> std::result::Result<Value, E> {
                Ok(Value::Bool(v))
            }

            fn visit_i64<E>(self, v: i64) -> std::result::Result<Value, E> {
                Ok(Value::Int(v))
            }

            fn visit_u64<E>(self, v: u64) -> std::result::Result<Value, E> {
                Ok(Value::Uint(v))
            }

            fn visit_i128<E>(self, v: i128) -> std::result::Result<Value, E> {
                // narrow back to the smaller variants when it fits, so a
                // round trip through i64/u64-only formats stays stable
                Ok(match i64::try_from(v) {
                    Ok(i) => Value::Int(i),
                    Err(_) => match u64::try_from(v) {
                        Ok(u) => Value::Uint(u),
                        Err(_) => Value::Int128(v),
                    },
                })
            }

            fn visit_u128<E>(self, v: u128) -> std::result::Result<Value, E> {
                Ok(match u64::try_from(v) {
                    Ok(u) => Value::Uint(u),
                    Err(_) => Value::Uint128(v),
                })
            }

            fn visit_f64<E>(self, v: f64) -> std::result::Result<Value, E> {
                Ok(Value::Float(v))
            }

            fn visit_str<E>(self, v: &str) -> std::result::Result<Value, E> {
                Ok(Value::String(v.to_owned()))
            }

            fn visit_string<E>(self, v: String) -> std::result::Result<Value, E> {
                Ok(Value::String(v))
            }

            fn visit_newtype_struct<D>(
                self,
                deserializer: D,
            ) -> std::result::Result<Value, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                // c4's own deserializer parked the value here (see
                // `PASS_THROUGH`); any other one is an ordinary newtype
                match PASS_THROUGH.with(|slot| slot.take()) {
                    Some(value) => Ok(value),
                    None => deserializer.deserialize_any(ValueVisitor),
                }
            }

            fn visit_unit<E>(self) -> std::result::Result<Value, E> {
                Ok(Value::Null)
            }

            fn visit_none<E>(self) -> std::result::Result<Value, E> {
                Ok(Value::Null)
            }

            fn visit_some<D>(self, deserializer: D) -> std::result::Result<Value, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                serde::Deserialize::deserialize(deserializer)
            }

            fn visit_seq<A>(self, mut seq: A) -> std::result::Result<Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut items = Vec::new();
                while let Some(item) = seq.next_element()? {
                    items.push(item);
                }
                Ok(Value::Array(items))
            }

            fn visit_map<A>(self, mut map: A) -> std::result::Result<Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut object = BTreeMap::new();
                while let Some((key, value)) = map.next_entry::<String, Value>()? {
                    object.insert(key, value);
                }
                Ok(Value::Object(object))
            }
        }

        deserializer.deserialize_newtype_struct(PASS_THROUGH_NAME, ValueVisitor)
    }
}

/// The newtype name `Value::deserialize` asks for. c4's `ValueDeserializer`
/// answers it by parking its owned value in [`PASS_THROUGH`] — no serde
/// round trip, which would flatten typed variants into strings.
pub(crate) const PASS_THROUGH_NAME: &str = "$c4::private::Value";

thread_local! {
    pub(crate) static PASS_THROUGH: std::cell::Cell<Option<Value>> =
        const { std::cell::Cell::new(None) };
}

impl serde::Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Value::Null => serializer.serialize_unit(),
            Value::Bool(b) => serializer.serialize_bool(*b),
            Value::Int(i) => serializer.serialize_i64(*i),
            Value::Uint(u) => serializer.serialize_u64(*u),
            // 128-bit narrows to the smaller numeric variants when it fits;
            // a value past u64 has no JSON number form, so it serializes as
            // its decimal string (a lossy round trip, like textual variants)
            Value::Int128(i) => match i64::try_from(*i) {
                Ok(v) => serializer.serialize_i64(v),
                Err(_) => match u64::try_from(*i) {
                    Ok(v) => serializer.serialize_u64(v),
                    Err(_) => serializer.collect_str(i),
                },
            },
            Value::Uint128(u) => match u64::try_from(*u) {
                Ok(v) => serializer.serialize_u64(v),
                Err(_) => serializer.collect_str(u),
            },
            Value::Float(f) => serializer.serialize_f64(*f),
            Value::String(s) => serializer.serialize_str(s),
            Value::DateTime(s)
            | Value::Date(s)
            | Value::Time(s)
            | Value::Inet(s)
            | Value::Cidr(s)
            | Value::MacAddr(s)
            | Value::MacAddr8(s)
            | Value::Uuid(s) => serializer.serialize_str(s),
            Value::Ipv4(ip) | Value::Ipv6(ip) => serializer.serialize_str(ip),
            Value::Array(items) => serializer.collect_seq(items),
            Value::Object(map) => serializer.collect_map(map),
        }
    }
}
