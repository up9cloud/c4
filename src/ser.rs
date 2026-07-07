//! Serialize any serde type into a [`Value`] — the engine behind
//! [`crate::Source::value`] (typed in-code overrides). Pure serde, no
//! format feature involved. Map keys must be strings; enums follow the
//! usual serde shapes (unit variant → string, data variant →
//! `{variant: …}`).

use std::collections::BTreeMap;
use std::fmt;

use serde::Serialize;
use serde::ser::{self, Serializer};

use crate::Value;

#[derive(Debug)]
pub(crate) struct SerError(String);

impl fmt::Display for SerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SerError {}

impl ser::Error for SerError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        SerError(msg.to_string())
    }
}

pub(crate) struct ValueSerializer;

impl Serializer for ValueSerializer {
    type Ok = Value;
    type Error = SerError;
    type SerializeSeq = SeqSer;
    type SerializeTuple = SeqSer;
    type SerializeTupleStruct = SeqSer;
    type SerializeTupleVariant = VariantSeqSer;
    type SerializeMap = MapSer;
    type SerializeStruct = MapSer;
    type SerializeStructVariant = VariantMapSer;

    fn serialize_bool(self, v: bool) -> Result<Value, SerError> {
        Ok(Value::Bool(v))
    }

    fn serialize_i8(self, v: i8) -> Result<Value, SerError> {
        Ok(Value::Int(v.into()))
    }

    fn serialize_i16(self, v: i16) -> Result<Value, SerError> {
        Ok(Value::Int(v.into()))
    }

    fn serialize_i32(self, v: i32) -> Result<Value, SerError> {
        Ok(Value::Int(v.into()))
    }

    fn serialize_i64(self, v: i64) -> Result<Value, SerError> {
        Ok(Value::Int(v))
    }

    fn serialize_i128(self, v: i128) -> Result<Value, SerError> {
        // narrow to the smaller variants when it fits (mirrors u64 → Int)
        Ok(i64::try_from(v).map_or_else(
            |_| u64::try_from(v).map_or(Value::Int128(v), Value::Uint),
            Value::Int,
        ))
    }

    fn serialize_u8(self, v: u8) -> Result<Value, SerError> {
        Ok(Value::Int(v.into()))
    }

    fn serialize_u16(self, v: u16) -> Result<Value, SerError> {
        Ok(Value::Int(v.into()))
    }

    fn serialize_u32(self, v: u32) -> Result<Value, SerError> {
        Ok(Value::Int(v.into()))
    }

    fn serialize_u64(self, v: u64) -> Result<Value, SerError> {
        // like parsing: fits in i64 → Int, otherwise Uint
        Ok(i64::try_from(v).map_or(Value::Uint(v), Value::Int))
    }

    fn serialize_u128(self, v: u128) -> Result<Value, SerError> {
        Ok(u64::try_from(v).map_or(Value::Uint128(v), Value::Uint))
    }

    fn serialize_f32(self, v: f32) -> Result<Value, SerError> {
        Ok(Value::Float(v.into()))
    }

    fn serialize_f64(self, v: f64) -> Result<Value, SerError> {
        Ok(Value::Float(v))
    }

    fn serialize_char(self, v: char) -> Result<Value, SerError> {
        Ok(Value::String(v.to_string()))
    }

    fn serialize_str(self, v: &str) -> Result<Value, SerError> {
        Ok(Value::String(v.to_owned()))
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Value, SerError> {
        Ok(Value::Array(
            v.iter().map(|b| Value::Int((*b).into())).collect(),
        ))
    }

    fn serialize_none(self) -> Result<Value, SerError> {
        Ok(Value::Null)
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<Value, SerError> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Value, SerError> {
        Ok(Value::Null)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Value, SerError> {
        Ok(Value::Null)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
    ) -> Result<Value, SerError> {
        Ok(Value::String(variant.to_owned()))
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Value, SerError> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Value, SerError> {
        let inner = value.serialize(ValueSerializer)?;
        Ok(Value::Object(BTreeMap::from([(variant.to_owned(), inner)])))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<SeqSer, SerError> {
        Ok(SeqSer(Vec::new()))
    }

    fn serialize_tuple(self, len: usize) -> Result<SeqSer, SerError> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(self, _name: &'static str, len: usize) -> Result<SeqSer, SerError> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<VariantSeqSer, SerError> {
        Ok(VariantSeqSer {
            variant,
            items: Vec::new(),
        })
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<MapSer, SerError> {
        Ok(MapSer {
            entries: BTreeMap::new(),
            key: None,
        })
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<MapSer, SerError> {
        self.serialize_map(None)
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<VariantMapSer, SerError> {
        Ok(VariantMapSer {
            variant,
            entries: BTreeMap::new(),
        })
    }
}

pub(crate) struct SeqSer(Vec<Value>);

impl ser::SerializeSeq for SeqSer {
    type Ok = Value;
    type Error = SerError;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), SerError> {
        self.0.push(value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Value, SerError> {
        Ok(Value::Array(self.0))
    }
}

impl ser::SerializeTuple for SeqSer {
    type Ok = Value;
    type Error = SerError;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), SerError> {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Value, SerError> {
        ser::SerializeSeq::end(self)
    }
}

impl ser::SerializeTupleStruct for SeqSer {
    type Ok = Value;
    type Error = SerError;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), SerError> {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Value, SerError> {
        ser::SerializeSeq::end(self)
    }
}

pub(crate) struct VariantSeqSer {
    variant: &'static str,
    items: Vec<Value>,
}

impl ser::SerializeTupleVariant for VariantSeqSer {
    type Ok = Value;
    type Error = SerError;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), SerError> {
        self.items.push(value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Value, SerError> {
        Ok(Value::Object(BTreeMap::from([(
            self.variant.to_owned(),
            Value::Array(self.items),
        )])))
    }
}

pub(crate) struct MapSer {
    entries: BTreeMap<String, Value>,
    key: Option<String>,
}

impl ser::SerializeMap for MapSer {
    type Ok = Value;
    type Error = SerError;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), SerError> {
        self.key = Some(key.serialize(KeySerializer)?);
        Ok(())
    }

    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), SerError> {
        let key = self
            .key
            .take()
            .expect("serialize_value before serialize_key");
        self.entries.insert(key, value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Value, SerError> {
        Ok(Value::Object(self.entries))
    }
}

impl ser::SerializeStruct for MapSer {
    type Ok = Value;
    type Error = SerError;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), SerError> {
        self.entries
            .insert(key.to_owned(), value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Value, SerError> {
        Ok(Value::Object(self.entries))
    }
}

pub(crate) struct VariantMapSer {
    variant: &'static str,
    entries: BTreeMap<String, Value>,
}

impl ser::SerializeStructVariant for VariantMapSer {
    type Ok = Value;
    type Error = SerError;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), SerError> {
        self.entries
            .insert(key.to_owned(), value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Value, SerError> {
        Ok(Value::Object(BTreeMap::from([(
            self.variant.to_owned(),
            Value::Object(self.entries),
        )])))
    }
}

/// Map keys must be strings (or chars); anything else is an error.
struct KeySerializer;

macro_rules! key_error {
    ($($method:ident: $ty:ty,)*) => {
        $(fn $method(self, _v: $ty) -> Result<String, SerError> {
            Err(ser::Error::custom("map keys must be strings"))
        })*
    };
}

impl Serializer for KeySerializer {
    type Ok = String;
    type Error = SerError;
    type SerializeSeq = ser::Impossible<String, SerError>;
    type SerializeTuple = ser::Impossible<String, SerError>;
    type SerializeTupleStruct = ser::Impossible<String, SerError>;
    type SerializeTupleVariant = ser::Impossible<String, SerError>;
    type SerializeMap = ser::Impossible<String, SerError>;
    type SerializeStruct = ser::Impossible<String, SerError>;
    type SerializeStructVariant = ser::Impossible<String, SerError>;

    fn serialize_str(self, v: &str) -> Result<String, SerError> {
        Ok(v.to_owned())
    }

    fn serialize_char(self, v: char) -> Result<String, SerError> {
        Ok(v.to_string())
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
    ) -> Result<String, SerError> {
        Ok(variant.to_owned())
    }

    key_error! {
        serialize_bool: bool,
        serialize_i8: i8,
        serialize_i16: i16,
        serialize_i32: i32,
        serialize_i64: i64,
        serialize_u8: u8,
        serialize_u16: u16,
        serialize_u32: u32,
        serialize_u64: u64,
        serialize_f32: f32,
        serialize_f64: f64,
        serialize_bytes: &[u8],
    }

    fn serialize_none(self) -> Result<String, SerError> {
        Err(ser::Error::custom("map keys must be strings"))
    }

    fn serialize_some<T: Serialize + ?Sized>(self, _value: &T) -> Result<String, SerError> {
        Err(ser::Error::custom("map keys must be strings"))
    }

    fn serialize_unit(self) -> Result<String, SerError> {
        Err(ser::Error::custom("map keys must be strings"))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<String, SerError> {
        Err(ser::Error::custom("map keys must be strings"))
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<String, SerError> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<String, SerError> {
        Err(ser::Error::custom("map keys must be strings"))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, SerError> {
        Err(ser::Error::custom("map keys must be strings"))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, SerError> {
        Err(ser::Error::custom("map keys must be strings"))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, SerError> {
        Err(ser::Error::custom("map keys must be strings"))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, SerError> {
        Err(ser::Error::custom("map keys must be strings"))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, SerError> {
        Err(ser::Error::custom("map keys must be strings"))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, SerError> {
        Err(ser::Error::custom("map keys must be strings"))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, SerError> {
        Err(ser::Error::custom("map keys must be strings"))
    }
}
