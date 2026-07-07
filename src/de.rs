//! Deserialize any serde type out of an owned [`Value`] — the back half
//! of `Loader::load<T>`.

use std::collections::btree_map;
use std::fmt;

use serde::de::{
    self, DeserializeSeed, Deserializer, EnumAccess, MapAccess, SeqAccess, VariantAccess, Visitor,
};

use crate::Value;

/// Error type of the [`Value`] deserializer; converted to
/// [`crate::Error::Deserialize`] by `Loader::load`.
#[derive(Debug)]
pub(crate) struct DeError(String);

impl fmt::Display for DeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DeError {}

impl de::Error for DeError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        DeError(msg.to_string())
    }
}

pub(crate) struct ValueDeserializer(Value);

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
            | Value::Uuid(s) => visitor.visit_string(s),
            Value::Ipv4(ip) => visitor.visit_string(ip.to_string()),
            Value::Ipv6(ip) => visitor.visit_string(ip.to_string()),
            Value::Array(items) => visitor.visit_seq(SeqDe(items.into_iter())),
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
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, DeError> {
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

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct seq tuple tuple_struct map struct
        identifier ignored_any
    }
}

struct SeqDe(std::vec::IntoIter<Value>);

impl<'de> SeqAccess<'de> for SeqDe {
    type Error = DeError;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, DeError> {
        match self.0.next() {
            Some(value) => seed.deserialize(ValueDeserializer(value)).map(Some),
            None => Ok(None),
        }
    }
}

struct MapDe {
    iter: btree_map::IntoIter<String, Value>,
    value: Option<Value>,
}

impl<'de> MapAccess<'de> for MapDe {
    type Error = DeError;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, DeError> {
        match self.iter.next() {
            Some((key, value)) => {
                self.value = Some(value);
                seed.deserialize(ValueDeserializer(Value::String(key)))
                    .map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, DeError> {
        let value = self
            .value
            .take()
            .expect("next_value_seed before next_key_seed");
        seed.deserialize(ValueDeserializer(value))
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
