//! Deserialize any serde type out of an owned [`Value`] — the back half
//! of `Loader::load<T>`.

use std::collections::btree_map;
use std::fmt;

use serde::de::{
    self, DeserializeSeed, Deserializer, EnumAccess, MapAccess, SeqAccess, VariantAccess, Visitor,
};

use crate::Value;

/// Error type of the [`Value`] deserializer; converted to
/// [`crate::Error::Deserialize`] by `Loader::load`. `path` collects the
/// keys (`db`) and indexes (`[2]`) the error bubbled out of, outermost
/// first.
#[derive(Debug)]
pub(crate) struct DeError {
    path: Vec<String>,
    message: String,
}

impl DeError {
    fn at(mut self, segment: String) -> Self {
        self.path.insert(0, segment);
        self
    }
}

impl fmt::Display for DeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, segment) in self.path.iter().enumerate() {
            if i > 0 && !segment.starts_with('[') {
                f.write_str(".")?;
            }
            f.write_str(segment)?;
        }
        if !self.path.is_empty() {
            f.write_str(": ")?;
        }
        f.write_str(&self.message)
    }
}

impl std::error::Error for DeError {}

impl de::Error for DeError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        DeError {
            path: Vec::new(),
            message: msg.to_string(),
        }
    }
}

pub(crate) struct ValueDeserializer(Value);

/// A string reaching a bool/number field (ini, env and cnf values are all
/// strings) parses with the table type-id rules; anything else, and every
/// untyped target, goes through `deserialize_any` unchanged.
macro_rules! coerce {
    ($($method:ident => $ty:literal),* $(,)?) => {$(
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
            match self.0 {
                Value::String(s) => match crate::format::table::scalar(&s, $ty) {
                    Some(value) => ValueDeserializer(value).deserialize_any(visitor),
                    None => Err(de::Error::custom(format!("'{s}' is not a valid {}", $ty))),
                },
                other => ValueDeserializer(other).deserialize_any(visitor),
            }
        }
    )*};
}

impl ValueDeserializer {
    pub(crate) fn new(value: Value) -> Self {
        ValueDeserializer(value)
    }
}

impl<'de> Deserializer<'de> for ValueDeserializer {
    type Error = DeError;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        match self.0 {
            Value::Null => visitor.visit_unit(),
            Value::Bool(b) => visitor.visit_bool(b),
            Value::Int(i) => visitor.visit_i64(i),
            Value::Uint(u) => visitor.visit_u64(u),
            Value::Int128(i) => visitor.visit_i128(i),
            Value::Uint128(u) => visitor.visit_u128(u),
            Value::Float(f) => visitor.visit_f64(f),
            Value::String(s)
            | Value::DateTime(s)
            | Value::Date(s)
            | Value::Time(s)
            | Value::Inet(s)
            | Value::Cidr(s)
            | Value::MacAddr(s)
            | Value::MacAddr8(s)
            | Value::Uuid(s)
            | Value::Ipv4(s)
            | Value::Ipv6(s) => visitor.visit_string(s),
            Value::Array(items) => visitor.visit_seq(SeqDe {
                iter: items.into_iter(),
                index: 0,
            }),
            Value::Object(map) => visitor.visit_map(MapDe {
                iter: map.into_iter(),
                value: None,
            }),
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        match self.0 {
            Value::Null => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, DeError> {
        if name == crate::value::PASS_THROUGH_NAME {
            // only `Value`'s visitor asks for this name; it takes the value
            // back out of the slot straight away
            crate::value::PASS_THROUGH.with(|slot| slot.set(Some(self.0)));
            return visitor.visit_newtype_struct(ValueDeserializer(Value::Null));
        }
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, DeError> {
        match self.0 {
            // a plain string is a unit variant
            Value::String(s) => visitor.visit_enum(EnumDe {
                variant: s,
                value: None,
            }),
            // { "Variant": value } carries variant data
            Value::Object(map) if map.len() == 1 => {
                let (variant, value) = map.into_iter().next().unwrap();
                visitor.visit_enum(EnumDe {
                    variant,
                    value: Some(value),
                })
            }
            other => Err(de::Error::custom(format!(
                "cannot deserialize enum from {other:?}"
            ))),
        }
    }

    coerce! {
        deserialize_bool => "bool", deserialize_i8 => "i8", deserialize_i16 => "i16",
        deserialize_i32 => "i32", deserialize_i64 => "i64", deserialize_i128 => "i128",
        deserialize_u8 => "u8", deserialize_u16 => "u16", deserialize_u32 => "u32",
        deserialize_u64 => "u64", deserialize_u128 => "u128", deserialize_f32 => "f32",
        deserialize_f64 => "f64",
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        self.deserialize_string(visitor)
    }

    /// A number or bool reaching a string field becomes its text; floats
    /// use `{:?}`, the shortest round-trip form that keeps `1.0`'s `.0`.
    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        match self.0 {
            Value::Bool(b) => visitor.visit_string(b.to_string()),
            Value::Int(i) => visitor.visit_string(i.to_string()),
            Value::Uint(u) => visitor.visit_string(u.to_string()),
            Value::Int128(i) => visitor.visit_string(i.to_string()),
            Value::Uint128(u) => visitor.visit_string(u.to_string()),
            Value::Float(f) => visitor.visit_string(format!("{f:?}")),
            other => ValueDeserializer(other).deserialize_any(visitor),
        }
    }

    serde::forward_to_deserialize_any! {
        char bytes byte_buf unit unit_struct seq tuple tuple_struct
        map struct identifier ignored_any
    }
}

struct SeqDe {
    iter: std::vec::IntoIter<Value>,
    index: usize,
}

impl<'de> SeqAccess<'de> for SeqDe {
    type Error = DeError;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, DeError> {
        let Some(value) = self.iter.next() else {
            return Ok(None);
        };
        let index = self.index;
        self.index += 1;
        seed.deserialize(ValueDeserializer(value))
            .map(Some)
            .map_err(|e| e.at(format!("[{index}]")))
    }
}

struct MapDe {
    iter: btree_map::IntoIter<String, Value>,
    value: Option<(String, Value)>,
}

impl<'de> MapAccess<'de> for MapDe {
    type Error = DeError;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, DeError> {
        match self.iter.next() {
            Some((key, value)) => {
                self.value = Some((key.clone(), value));
                seed.deserialize(ValueDeserializer(Value::String(key)))
                    .map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, DeError> {
        let (key, value) = self
            .value
            .take()
            .expect("next_value_seed before next_key_seed");
        seed.deserialize(ValueDeserializer(value))
            .map_err(|e| e.at(key))
    }
}

struct EnumDe {
    variant: String,
    value: Option<Value>,
}

impl<'de> EnumAccess<'de> for EnumDe {
    type Error = DeError;
    type Variant = VariantDe;

    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, VariantDe), DeError> {
        let variant = seed.deserialize(ValueDeserializer(Value::String(self.variant)))?;
        Ok((variant, VariantDe(self.value)))
    }
}

struct VariantDe(Option<Value>);

impl<'de> VariantAccess<'de> for VariantDe {
    type Error = DeError;

    fn unit_variant(self) -> Result<(), DeError> {
        match self.0 {
            None | Some(Value::Null) => Ok(()),
            Some(other) => Err(de::Error::custom(format!(
                "unexpected data for unit variant: {other:?}"
            ))),
        }
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, DeError> {
        seed.deserialize(ValueDeserializer(self.0.unwrap_or(Value::Null)))
    }

    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value, DeError> {
        ValueDeserializer(self.0.unwrap_or(Value::Null)).deserialize_any(visitor)
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, DeError> {
        ValueDeserializer(self.0.unwrap_or(Value::Null)).deserialize_any(visitor)
    }
}
