use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};

use crate::error::Error;
use crate::Value;

impl<'de> serde::Deserialize<'de> for Value {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(ValueVisitor)
    }
}

struct ValueVisitor;

impl<'de> Visitor<'de> for ValueVisitor {
    type Value = Value;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a phig value")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Value, E> {
        Ok(Value::String(v.to_string()))
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<Value, E> {
        Ok(Value::String(v))
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Value, E> {
        Ok(Value::String(v.to_string()))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Value, E> {
        Ok(Value::String(v.to_string()))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Value, E> {
        Ok(Value::String(v.to_string()))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Value, E> {
        Ok(Value::String(v.to_string()))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut items = Vec::new();
        while let Some(v) = seq.next_element()? {
            items.push(v);
        }
        Ok(Value::List(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut pairs = Vec::new();
        while let Some((k, v)) = map.next_entry()? {
            pairs.push((k, v));
        }
        Ok(Value::Map(pairs))
    }
}

/// Deserialize a `T` from a [`Value`].
pub fn from_value<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, Error> {
    if !matches!(value, Value::Map(_)) {
        return Err(Error::new("top-level value must be a map"));
    }
    T::deserialize(ValueDeserializer { value })
}

struct ValueDeserializer {
    value: Value,
}

impl<'de> de::Deserializer<'de> for ValueDeserializer {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.value {
            Value::String(s) => visitor.visit_string(s),
            Value::List(items) => visitor.visit_seq(ValueSeqAccess {
                iter: items.into_iter(),
            }),
            Value::Map(pairs) => visitor.visit_map(ValueMapAccess {
                iter: pairs.into_iter(),
                value: None,
            }),
        }
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.value {
            Value::String(ref s) => match s.as_str() {
                "true" => visitor.visit_bool(true),
                "false" => visitor.visit_bool(false),
                _ => Err(Error::new(format!("expected bool, got '{}'", s))),
            },
            _ => Err(Error::new("expected string for bool")),
        }
    }

    fn deserialize_i8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_i8(self.parse_number()?)
    }

    fn deserialize_i16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_i16(self.parse_number()?)
    }

    fn deserialize_i32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_i32(self.parse_number()?)
    }

    fn deserialize_i64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_i64(self.parse_number()?)
    }

    fn deserialize_u8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_u8(self.parse_number()?)
    }

    fn deserialize_u16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_u16(self.parse_number()?)
    }

    fn deserialize_u32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_u32(self.parse_number()?)
    }

    fn deserialize_u64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_u64(self.parse_number()?)
    }

    fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_f32(self.parse_number()?)
    }

    fn deserialize_f64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_f64(self.parse_number()?)
    }

    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.value {
            Value::String(s) => {
                let mut chars = s.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) => visitor.visit_char(c),
                    _ => Err(Error::new("expected single character")),
                }
            }
            _ => Err(Error::new("expected string for char")),
        }
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_string(visitor)
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.value {
            Value::String(s) => visitor.visit_string(s),
            _ => Err(Error::new("expected string")),
        }
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Error> {
        Err(Error::new("byte arrays are not supported"))
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Error> {
        Err(Error::new("byte arrays are not supported"))
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_some(self)
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_unit()
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.value {
            Value::List(items) => visitor.visit_seq(ValueSeqAccess {
                iter: items.into_iter(),
            }),
            _ => Err(Error::new("expected list")),
        }
    }

    fn deserialize_tuple<V: Visitor<'de>>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.value {
            Value::Map(pairs) => visitor.visit_map(ValueMapAccess {
                iter: pairs.into_iter(),
                value: None,
            }),
            _ => Err(Error::new("expected map")),
        }
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        match self.value {
            Value::String(s) => visitor.visit_enum(ValueEnumAccess {
                variant: s,
                value: None,
            }),
            Value::Map(mut pairs) => {
                if pairs.len() != 1 {
                    return Err(Error::new("expected single-entry map for enum"));
                }
                let (variant, value) = pairs.remove(0);
                visitor.visit_enum(ValueEnumAccess {
                    variant,
                    value: Some(value),
                })
            }
            _ => Err(Error::new("expected string or map for enum")),
        }
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_string(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_unit()
    }
}

impl ValueDeserializer {
    fn parse_number<T: std::str::FromStr>(&self) -> Result<T, Error>
    where
        T::Err: std::fmt::Display,
    {
        match &self.value {
            Value::String(s) => s
                .parse()
                .map_err(|e: T::Err| Error::new(format!("invalid number '{}': {}", s, e))),
            _ => Err(Error::new("expected string for number")),
        }
    }
}

struct ValueSeqAccess {
    iter: std::vec::IntoIter<Value>,
}

impl<'de> SeqAccess<'de> for ValueSeqAccess {
    type Error = Error;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Error> {
        match self.iter.next() {
            Some(v) => seed.deserialize(ValueDeserializer { value: v }).map(Some),
            None => Ok(None),
        }
    }
}

struct ValueMapAccess {
    iter: std::vec::IntoIter<(String, Value)>,
    value: Option<Value>,
}

impl<'de> MapAccess<'de> for ValueMapAccess {
    type Error = Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Error> {
        match self.iter.next() {
            Some((key, value)) => {
                self.value = Some(value);
                seed.deserialize(ValueDeserializer {
                    value: Value::String(key),
                })
                .map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, Error> {
        let value = self
            .value
            .take()
            .ok_or_else(|| Error::new("missing value"))?;
        seed.deserialize(ValueDeserializer { value })
    }
}

struct ValueEnumAccess {
    variant: String,
    value: Option<Value>,
}

impl<'de> de::EnumAccess<'de> for ValueEnumAccess {
    type Error = Error;
    type Variant = ValueVariantAccess;

    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), Error> {
        let variant = seed.deserialize(ValueDeserializer {
            value: Value::String(self.variant),
        })?;
        Ok((variant, ValueVariantAccess { value: self.value }))
    }
}

struct ValueVariantAccess {
    value: Option<Value>,
}

impl<'de> de::VariantAccess<'de> for ValueVariantAccess {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Error> {
        Ok(())
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, Error> {
        match self.value {
            Some(v) => seed.deserialize(ValueDeserializer { value: v }),
            None => Err(Error::new("expected data for newtype variant")),
        }
    }

    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value, Error> {
        match self.value {
            Some(v) => de::Deserializer::deserialize_seq(ValueDeserializer { value: v }, visitor),
            None => Err(Error::new("expected data for tuple variant")),
        }
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        match self.value {
            Some(v) => de::Deserializer::deserialize_map(ValueDeserializer { value: v }, visitor),
            None => Err(Error::new("expected data for struct variant")),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use crate::{from_str, Value};

    #[test]
    fn to_value() {
        let v: Value = from_str("name foo\ntags [a b]").unwrap();
        assert_eq!(
            v,
            Value::Map(vec![
                ("name".into(), Value::String("foo".into())),
                (
                    "tags".into(),
                    Value::List(vec![Value::String("a".into()), Value::String("b".into()),])
                ),
            ])
        );
    }

    #[test]
    fn from_value() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct Config {
            name: String,
            port: u16,
        }

        let v = Value::Map(vec![
            ("name".into(), Value::String("app".into())),
            ("port".into(), Value::String("8080".into())),
        ]);
        let cfg: Config = super::from_value(v).unwrap();
        assert_eq!(
            cfg,
            Config {
                name: "app".into(),
                port: 8080,
            }
        );
    }
}
