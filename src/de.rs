use std::io::Read;

use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};

use crate::error::Error;
use crate::{parse, ValueBuilder};
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

/// Deserialize a `T` from a phig reader.
pub fn from_reader<T: serde::de::DeserializeOwned>(reader: impl Read) -> Result<T, Error> {
    let mut builder = ValueBuilder::new();
    parse::parse(reader, &mut builder)?;
    from_value(builder.finish())
}

/// Deserialize a `T` from a phig string.
///
/// ```
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct Config { name: String, port: u16 }
///
/// let cfg: Config = phig::from_str("name app\nport 8080").unwrap();
/// assert_eq!(cfg.port, 8080);
/// ```
pub fn from_str<T: serde::de::DeserializeOwned>(s: &str) -> Result<T, Error> {
    from_reader(s.as_bytes())
}

/// Deserialize a `T` from a [`Value`].
pub fn from_value<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, Error> {
    if !matches!(value, Value::Map(_)) {
        return Err(Error::new("top-level value must be a map"));
    }
    T::deserialize(Deserializer { value })
}

struct Deserializer {
    value: Value,
}

impl<'de> de::Deserializer<'de> for Deserializer {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.value {
            Value::String(s) => visitor.visit_string(s),
            Value::List(items) => visitor.visit_seq(SeqAccessor {
                iter: items.into_iter(),
            }),
            Value::Map(pairs) => visitor.visit_map(MapAccessor {
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
            Value::List(items) => visitor.visit_seq(SeqAccessor {
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
            Value::Map(pairs) => visitor.visit_map(MapAccessor {
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
            Value::String(s) => visitor.visit_enum(EnumDeserializer {
                variant: s,
                value: None,
            }),
            Value::Map(mut pairs) => {
                if pairs.len() != 1 {
                    return Err(Error::new("expected single-entry map for enum"));
                }
                let (variant, value) = pairs.remove(0);
                visitor.visit_enum(EnumDeserializer {
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

impl Deserializer {
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

struct SeqAccessor {
    iter: std::vec::IntoIter<Value>,
}

impl<'de> SeqAccess<'de> for SeqAccessor {
    type Error = Error;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Error> {
        match self.iter.next() {
            Some(v) => seed.deserialize(Deserializer { value: v }).map(Some),
            None => Ok(None),
        }
    }
}

struct MapAccessor {
    iter: std::vec::IntoIter<(String, Value)>,
    value: Option<Value>,
}

impl<'de> MapAccess<'de> for MapAccessor {
    type Error = Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Error> {
        match self.iter.next() {
            Some((key, value)) => {
                self.value = Some(value);
                seed.deserialize(Deserializer {
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
        seed.deserialize(Deserializer { value })
    }
}

struct EnumDeserializer {
    variant: String,
    value: Option<Value>,
}

impl<'de> de::EnumAccess<'de> for EnumDeserializer {
    type Error = Error;
    type Variant = VariantDeserializer;

    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), Error> {
        let variant = seed.deserialize(Deserializer {
            value: Value::String(self.variant),
        })?;
        Ok((variant, VariantDeserializer { value: self.value }))
    }
}

struct VariantDeserializer {
    value: Option<Value>,
}

impl<'de> de::VariantAccess<'de> for VariantDeserializer {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Error> {
        Ok(())
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, Error> {
        match self.value {
            Some(v) => seed.deserialize(Deserializer { value: v }),
            None => Err(Error::new("expected data for newtype variant")),
        }
    }

    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value, Error> {
        match self.value {
            Some(v) => de::Deserializer::deserialize_seq(Deserializer { value: v }, visitor),
            None => Err(Error::new("expected data for tuple variant")),
        }
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        match self.value {
            Some(v) => de::Deserializer::deserialize_map(Deserializer { value: v }, visitor),
            None => Err(Error::new("expected data for struct variant")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[test]
    fn struct_basic() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct Config {
            name: String,
            version: String,
        }

        let cfg: Config = from_str("name myapp\nversion 1.0").unwrap();
        assert_eq!(
            cfg,
            Config {
                name: "myapp".into(),
                version: "1.0".into(),
            }
        );
    }

    #[test]
    fn numeric_fields() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct Server {
            port: u16,
            workers: i32,
        }

        let s: Server = from_str("port 8080\nworkers 4").unwrap();
        assert_eq!(
            s,
            Server {
                port: 8080,
                workers: 4
            }
        );
    }

    #[test]
    fn bool_fields() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct Flags {
            debug: bool,
            verbose: bool,
        }

        let f: Flags = from_str("debug true\nverbose false").unwrap();
        assert_eq!(
            f,
            Flags {
                debug: true,
                verbose: false
            }
        );
    }

    #[test]
    fn vec_field() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct Config {
            tags: Vec<String>,
        }

        let c: Config = from_str("tags [web prod v2]").unwrap();
        assert_eq!(
            c,
            Config {
                tags: vec!["web".into(), "prod".into(), "v2".into()],
            }
        );
    }

    #[test]
    fn nested_struct() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct Inner {
            host: String,
            port: u16,
        }

        #[derive(Deserialize, Debug, PartialEq)]
        struct Outer {
            server: Inner,
        }

        let o: Outer = from_str("server { host localhost; port 3000 }").unwrap();
        assert_eq!(
            o,
            Outer {
                server: Inner {
                    host: "localhost".into(),
                    port: 3000,
                },
            }
        );
    }

    #[test]
    fn optional_present() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct Config {
            name: String,
            debug: Option<bool>,
        }

        let c: Config = from_str("name app\ndebug true").unwrap();
        assert_eq!(
            c,
            Config {
                name: "app".into(),
                debug: Some(true),
            }
        );
    }

    #[test]
    fn optional_missing() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct Config {
            name: String,
            #[serde(default)]
            debug: Option<bool>,
        }

        let c: Config = from_str("name app").unwrap();
        assert_eq!(
            c,
            Config {
                name: "app".into(),
                debug: None,
            }
        );
    }

    #[test]
    fn hashmap() {
        use std::collections::HashMap;
        let m: HashMap<String, String> = from_str("a 1\nb 2\nc 3").unwrap();
        assert_eq!(m.len(), 3);
        assert_eq!(m["a"], "1");
        assert_eq!(m["b"], "2");
        assert_eq!(m["c"], "3");
    }

    #[test]
    fn enum_unit() {
        #[derive(Deserialize, Debug, PartialEq)]
        #[serde(rename_all = "lowercase")]
        enum Level {
            Debug,
            Info,
            Warn,
        }

        #[derive(Deserialize, Debug, PartialEq)]
        struct Config {
            level: Level,
        }

        let c: Config = from_str("level debug").unwrap();
        assert_eq!(
            c,
            Config {
                level: Level::Debug
            }
        );
    }

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
}
