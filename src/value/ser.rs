use serde::ser::{self, Serialize, SerializeMap, SerializeSeq};

use crate::error::Error;
use crate::Value;

impl Serialize for Value {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Value::String(s) => serializer.serialize_str(s),
            Value::List(items) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            Value::Map(pairs) => {
                let mut map = serializer.serialize_map(Some(pairs.len()))?;
                for (k, v) in pairs {
                    map.serialize_entry(k, v)?;
                }
                map.end()
            }
        }
    }
}

/// Serialize a `T` to a [`Value`].
///
/// The value must serialize as a map.
pub fn to_value<T: Serialize>(value: &T) -> Result<Value, Error> {
    let val = value.serialize(ValueSerializer)?;
    if !matches!(val, Value::Map(_)) {
        return Err(Error::new("top-level value must be a map"));
    }
    Ok(val)
}

struct ValueSerializer;

impl ser::Serializer for ValueSerializer {
    type Ok = Value;
    type Error = Error;
    type SerializeSeq = SeqCollector;
    type SerializeTuple = SeqCollector;
    type SerializeTupleStruct = SeqCollector;
    type SerializeTupleVariant = TupleVariantCollector;
    type SerializeMap = MapCollector;
    type SerializeStruct = MapCollector;
    type SerializeStructVariant = StructVariantCollector;

    fn serialize_bool(self, v: bool) -> Result<Value, Error> {
        Ok(Value::String(v.to_string()))
    }

    fn serialize_i8(self, v: i8) -> Result<Value, Error> {
        Ok(Value::String(v.to_string()))
    }

    fn serialize_i16(self, v: i16) -> Result<Value, Error> {
        Ok(Value::String(v.to_string()))
    }

    fn serialize_i32(self, v: i32) -> Result<Value, Error> {
        Ok(Value::String(v.to_string()))
    }

    fn serialize_i64(self, v: i64) -> Result<Value, Error> {
        Ok(Value::String(v.to_string()))
    }

    fn serialize_u8(self, v: u8) -> Result<Value, Error> {
        Ok(Value::String(v.to_string()))
    }

    fn serialize_u16(self, v: u16) -> Result<Value, Error> {
        Ok(Value::String(v.to_string()))
    }

    fn serialize_u32(self, v: u32) -> Result<Value, Error> {
        Ok(Value::String(v.to_string()))
    }

    fn serialize_u64(self, v: u64) -> Result<Value, Error> {
        Ok(Value::String(v.to_string()))
    }

    fn serialize_f32(self, v: f32) -> Result<Value, Error> {
        Ok(Value::String(v.to_string()))
    }

    fn serialize_f64(self, v: f64) -> Result<Value, Error> {
        Ok(Value::String(v.to_string()))
    }

    fn serialize_char(self, v: char) -> Result<Value, Error> {
        Ok(Value::String(v.to_string()))
    }

    fn serialize_str(self, v: &str) -> Result<Value, Error> {
        Ok(Value::String(v.to_string()))
    }

    fn serialize_bytes(self, _v: &[u8]) -> Result<Value, Error> {
        Err(Error::new("byte arrays are not supported"))
    }

    fn serialize_none(self) -> Result<Value, Error> {
        Ok(Value::String(String::new()))
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<Value, Error> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Value, Error> {
        Ok(Value::String(String::new()))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Value, Error> {
        Ok(Value::String(String::new()))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _idx: u32,
        variant: &'static str,
    ) -> Result<Value, Error> {
        Ok(Value::String(variant.to_string()))
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Value, Error> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _idx: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Value, Error> {
        let inner = value.serialize(ValueSerializer)?;
        Ok(Value::Map(vec![(variant.to_string(), inner)]))
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<SeqCollector, Error> {
        Ok(SeqCollector {
            items: Vec::with_capacity(len.unwrap_or(0)),
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<SeqCollector, Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<SeqCollector, Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _idx: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<TupleVariantCollector, Error> {
        Ok(TupleVariantCollector {
            variant: variant.to_string(),
            items: Vec::with_capacity(len),
        })
    }

    fn serialize_map(self, len: Option<usize>) -> Result<MapCollector, Error> {
        Ok(MapCollector {
            pairs: Vec::with_capacity(len.unwrap_or(0)),
            key: None,
        })
    }

    fn serialize_struct(self, _name: &'static str, len: usize) -> Result<MapCollector, Error> {
        self.serialize_map(Some(len))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _idx: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<StructVariantCollector, Error> {
        Ok(StructVariantCollector {
            variant: variant.to_string(),
            pairs: Vec::with_capacity(len),
        })
    }
}

struct SeqCollector {
    items: Vec<Value>,
}

impl ser::SerializeSeq for SeqCollector {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.items.push(value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Value, Error> {
        Ok(Value::List(self.items))
    }
}

impl ser::SerializeTuple for SeqCollector {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Value, Error> {
        ser::SerializeSeq::end(self)
    }
}

impl ser::SerializeTupleStruct for SeqCollector {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Value, Error> {
        ser::SerializeSeq::end(self)
    }
}

struct TupleVariantCollector {
    variant: String,
    items: Vec<Value>,
}

impl ser::SerializeTupleVariant for TupleVariantCollector {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.items.push(value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Value, Error> {
        Ok(Value::Map(vec![(self.variant, Value::List(self.items))]))
    }
}

struct MapCollector {
    pairs: Vec<(String, Value)>,
    key: Option<String>,
}

impl ser::SerializeMap for MapCollector {
    type Ok = Value;
    type Error = Error;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Error> {
        let val = key.serialize(ValueSerializer)?;
        self.key = Some(match val {
            Value::String(s) => s,
            _ => return Err(Error::new("map keys must be strings")),
        });
        Ok(())
    }

    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        let key = self
            .key
            .take()
            .ok_or_else(|| Error::new("serialize_value without key"))?;
        self.pairs.push((key, value.serialize(ValueSerializer)?));
        Ok(())
    }

    fn end(self) -> Result<Value, Error> {
        Ok(Value::Map(self.pairs))
    }
}

impl ser::SerializeStruct for MapCollector {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.pairs
            .push((key.to_string(), value.serialize(ValueSerializer)?));
        Ok(())
    }

    fn end(self) -> Result<Value, Error> {
        Ok(Value::Map(self.pairs))
    }
}

struct StructVariantCollector {
    variant: String,
    pairs: Vec<(String, Value)>,
}

impl ser::SerializeStructVariant for StructVariantCollector {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.pairs
            .push((key.to_string(), value.serialize(ValueSerializer)?));
        Ok(())
    }

    fn end(self) -> Result<Value, Error> {
        Ok(Value::Map(vec![(self.variant, Value::Map(self.pairs))]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[test]
    fn to_value_struct() {
        #[derive(Serialize)]
        struct Config {
            name: String,
            port: u16,
        }

        let v = to_value(&Config {
            name: "app".into(),
            port: 8080,
        })
        .unwrap();
        assert_eq!(
            v,
            Value::Map(vec![
                ("name".into(), Value::String("app".into())),
                ("port".into(), Value::String("8080".into())),
            ])
        );
    }
}
