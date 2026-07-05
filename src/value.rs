//! The dynamic config value: the [`Value`] enum, its accessors and its
//! serde bridges. `Value::format_id` names the parser a value came from
//! (shown by traces); typed variants beyond JSON's set (datetime, IPs,
//! MACs, …) are produced by the feature-gated table value parsers.

use std::collections::BTreeMap;

/// Dynamic config value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Uint(u64),
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
    /// Table `ipv4` type (`ipv4` feature); serializes canonically. The
    /// `inet` type (`inet` feature) also produces this or [`Value::Ipv6`],
    /// whichever family parses.
    Ipv4(std::net::Ipv4Addr),
    /// Table `ipv6` type (`ipv6` feature); serializes canonically.
    Ipv6(std::net::Ipv6Addr),
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
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Value::Uint(u) => Some(*u),
            Value::Int(i) => u64::try_from(*i).ok(),
            _ => None,
        }
    }

    /// Any numeric value as a float.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            Value::Int(i) => Some(*i as f64),
            Value::Uint(u) => Some(*u as f64),
            _ => None,
        }
    }

    /// The text of a string — or of a datetime/date/time value, which
    /// keep their text form.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s)
            | Value::DateTime(s)
            | Value::Date(s)
            | Value::Time(s)
            | Value::Inet(s)
            | Value::Cidr(s)
            | Value::MacAddr(s)
            | Value::MacAddr8(s)
            | Value::Uuid(s) => Some(s),
            _ => None,
        }
    }

    /// The format id of this value — which parser produced it: `null`,
    /// `bool`, `i64`, `u64`, `f64`, `str`, the typed ids (`dt`, `date`,
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

            fn visit_f64<E>(self, v: f64) -> std::result::Result<Value, E> {
                Ok(Value::Float(v))
            }

            fn visit_str<E>(self, v: &str) -> std::result::Result<Value, E> {
                Ok(Value::String(v.to_owned()))
            }

            fn visit_string<E>(self, v: String) -> std::result::Result<Value, E> {
                Ok(Value::String(v))
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

        deserializer.deserialize_any(ValueVisitor)
    }
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
            Value::Ipv4(ip) => serializer.collect_str(ip),
            Value::Ipv6(ip) => serializer.collect_str(ip),
            Value::Array(items) => serializer.collect_seq(items),
            Value::Object(map) => serializer.collect_map(map),
        }
    }
}
