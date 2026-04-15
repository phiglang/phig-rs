use std::io::Read;

use serde::de::{self, DeserializeSeed, IntoDeserializer, MapAccess, SeqAccess, Visitor};

use crate::error::Error;
use crate::parse::{Event, Parser};

/// Deserialize a `T` from a phig reader.
pub fn from_reader<T: serde::de::DeserializeOwned>(reader: impl Read) -> Result<T, Error> {
    let mut de = StreamDeserializer::new(reader);
    T::deserialize(&mut de)
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

struct StreamDeserializer<R: Read> {
    parser: Parser<R>,
    peeked: Option<Event>,
}

impl<R: Read> StreamDeserializer<R> {
    fn new(reader: R) -> Self {
        StreamDeserializer {
            parser: Parser::new(reader),
            peeked: None,
        }
    }

    fn peek_event(&mut self) -> Result<Option<&Event>, Error> {
        if self.peeked.is_none() {
            self.peeked = self.parser.next().transpose()?;
        }
        Ok(self.peeked.as_ref())
    }

    fn take_event(&mut self) -> Result<Event, Error> {
        match self.peeked.take() {
            Some(event) => Ok(event),
            None => self
                .parser
                .next()
                .transpose()?
                .ok_or_else(|| Error::new("unexpected end of input")),
        }
    }

    fn take_string(&mut self) -> Result<String, Error> {
        match self.take_event()? {
            Event::String(s) => Ok(s),
            _ => Err(Error::new("expected string")),
        }
    }

    fn parse_number<T: std::str::FromStr>(&mut self) -> Result<T, Error>
    where
        T::Err: std::fmt::Display,
    {
        let s = self.take_string()?;
        s.parse()
            .map_err(|e: T::Err| Error::new(format!("invalid number '{}': {}", s, e)))
    }

    fn skip_value(&mut self) -> Result<(), Error> {
        match self.take_event()? {
            Event::String(_) => Ok(()),
            Event::StartMap | Event::StartList => {
                let mut depth = 1u32;
                while depth > 0 {
                    match self.take_event()? {
                        Event::StartMap | Event::StartList => depth += 1,
                        Event::EndMap | Event::EndList => depth -= 1,
                        _ => {}
                    }
                }
                Ok(())
            }
            _ => Err(Error::new("unexpected event")),
        }
    }
}

impl<'de, 'a, R: Read> de::Deserializer<'de> for &'a mut StreamDeserializer<R> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.take_event()? {
            Event::String(s) => visitor.visit_string(s),
            Event::StartMap => visitor.visit_map(StreamMapAccess { de: self }),
            Event::StartList => visitor.visit_seq(StreamSeqAccess { de: self }),
            _ => Err(Error::new("unexpected event")),
        }
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.take_string()?.as_str() {
            "true" => visitor.visit_bool(true),
            "false" => visitor.visit_bool(false),
            s => Err(Error::new(format!("expected bool, got '{}'", s))),
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
        let s = self.take_string()?;
        let mut chars = s.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) => visitor.visit_char(c),
            _ => Err(Error::new("expected single character")),
        }
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_string(visitor)
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_string(self.take_string()?)
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
        match self.take_event()? {
            Event::StartList => visitor.visit_seq(StreamSeqAccess { de: self }),
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
        match self.take_event()? {
            Event::StartMap => visitor.visit_map(StreamMapAccess { de: self }),
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
        match self.peek_event()? {
            Some(Event::String(_)) => {
                let Event::String(s) = self.take_event()? else {
                    unreachable!()
                };
                visitor.visit_enum(s.into_deserializer())
            }
            Some(Event::StartMap) => {
                self.take_event()?; // consume StartMap
                let Event::Key(variant) = self.take_event()? else {
                    return Err(Error::new("expected variant name"));
                };
                let result = visitor.visit_enum(StreamEnumAccess { variant, de: self })?;
                match self.take_event()? {
                    Event::EndMap => Ok(result),
                    _ => Err(Error::new("expected end of enum map")),
                }
            }
            _ => Err(Error::new("expected string or map for enum")),
        }
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_string(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.skip_value()?;
        visitor.visit_unit()
    }
}

struct StreamMapAccess<'a, R: Read> {
    de: &'a mut StreamDeserializer<R>,
}

impl<'de, 'a, R: Read> MapAccess<'de> for StreamMapAccess<'a, R> {
    type Error = Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Error> {
        match self.de.take_event()? {
            Event::Key(k) => seed
                .deserialize(k.into_deserializer())
                .map(Some)
                .map_err(Error::from_serde::<serde::de::value::Error>),
            Event::EndMap => Ok(None),
            _ => Err(Error::new("expected key or end of map")),
        }
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, Error> {
        seed.deserialize(&mut *self.de)
    }
}

struct StreamSeqAccess<'a, R: Read> {
    de: &'a mut StreamDeserializer<R>,
}

impl<'de, 'a, R: Read> SeqAccess<'de> for StreamSeqAccess<'a, R> {
    type Error = Error;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Error> {
        match self.de.peek_event()? {
            Some(Event::EndList) => {
                self.de.take_event()?;
                Ok(None)
            }
            Some(_) => seed.deserialize(&mut *self.de).map(Some),
            None => Err(Error::new("unexpected end of input in list")),
        }
    }
}

struct StreamEnumAccess<'a, R: Read> {
    variant: String,
    de: &'a mut StreamDeserializer<R>,
}

impl<'de, 'a, R: Read> de::EnumAccess<'de> for StreamEnumAccess<'a, R> {
    type Error = Error;
    type Variant = StreamVariantAccess<'a, R>;

    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), Error> {
        let variant = seed
            .deserialize(self.variant.into_deserializer())
            .map_err(Error::from_serde::<serde::de::value::Error>)?;
        Ok((variant, StreamVariantAccess { de: self.de }))
    }
}

struct StreamVariantAccess<'a, R: Read> {
    de: &'a mut StreamDeserializer<R>,
}

impl<'de, 'a, R: Read> de::VariantAccess<'de> for StreamVariantAccess<'a, R> {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Error> {
        self.de.skip_value()?;
        Ok(())
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, Error> {
        seed.deserialize(&mut *self.de)
    }

    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value, Error> {
        de::Deserializer::deserialize_seq(&mut *self.de, visitor)
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        de::Deserializer::deserialize_map(&mut *self.de, visitor)
    }
}

impl Error {
    fn from_serde<E: de::Error>(e: E) -> Self {
        Error::new(format!("{}", e))
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
}
