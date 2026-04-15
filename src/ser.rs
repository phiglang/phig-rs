use std::io;

use serde::ser::{self, Impossible, Serialize};

use crate::error::Error;
use crate::fmt::Formatter;

/// Serialize a `T` to a phig writer.
///
/// The value must serialize as a map (struct, `HashMap`, etc.).
pub fn to_writer<T: Serialize>(value: &T, writer: impl io::Write) -> Result<(), Error> {
    let mut ser = StreamSerializer::new(writer);
    value.serialize(&mut ser)?;
    ser.fmt.into_inner().flush().map_err(Error::from)
}

/// Serialize a `T` to a phig string.
///
/// The value must serialize as a map (struct, `HashMap`, etc.).
///
/// ```
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Config { name: String, port: u16 }
///
/// let s = phig::to_string(&Config { name: "app".into(), port: 8080 }).unwrap();
/// assert_eq!(s, "name app\nport 8080\n");
/// ```
pub fn to_string<T: Serialize>(value: &T) -> Result<String, Error> {
    let mut buf = Vec::new();
    to_writer(value, &mut buf)?;
    Ok(String::from_utf8(buf).expect("phig output is always valid UTF-8"))
}

struct StreamSerializer<W: io::Write> {
    fmt: Formatter<W>,
    depth: u32,
}

impl<W: io::Write> StreamSerializer<W> {
    fn new(writer: W) -> Self {
        StreamSerializer {
            fmt: Formatter::new(writer),
            depth: 0,
        }
    }

    fn require_nested(&self) -> Result<(), Error> {
        if self.depth == 0 {
            Err(Error::new("top-level value must be a map"))
        } else {
            Ok(())
        }
    }
}

impl<'a, W: io::Write> ser::Serializer for &'a mut StreamSerializer<W> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = CompoundSeq<'a, W>;
    type SerializeTuple = CompoundSeq<'a, W>;
    type SerializeTupleStruct = CompoundSeq<'a, W>;
    type SerializeTupleVariant = CompoundTupleVariant<'a, W>;
    type SerializeMap = CompoundMap<'a, W>;
    type SerializeStruct = CompoundMap<'a, W>;
    type SerializeStructVariant = CompoundStructVariant<'a, W>;

    fn serialize_bool(self, v: bool) -> Result<(), Error> {
        self.require_nested()?;
        self.fmt.string(v.to_string())
    }

    fn serialize_i8(self, v: i8) -> Result<(), Error> {
        self.require_nested()?;
        self.fmt.string(v.to_string())
    }

    fn serialize_i16(self, v: i16) -> Result<(), Error> {
        self.require_nested()?;
        self.fmt.string(v.to_string())
    }

    fn serialize_i32(self, v: i32) -> Result<(), Error> {
        self.require_nested()?;
        self.fmt.string(v.to_string())
    }

    fn serialize_i64(self, v: i64) -> Result<(), Error> {
        self.require_nested()?;
        self.fmt.string(v.to_string())
    }

    fn serialize_u8(self, v: u8) -> Result<(), Error> {
        self.require_nested()?;
        self.fmt.string(v.to_string())
    }

    fn serialize_u16(self, v: u16) -> Result<(), Error> {
        self.require_nested()?;
        self.fmt.string(v.to_string())
    }

    fn serialize_u32(self, v: u32) -> Result<(), Error> {
        self.require_nested()?;
        self.fmt.string(v.to_string())
    }

    fn serialize_u64(self, v: u64) -> Result<(), Error> {
        self.require_nested()?;
        self.fmt.string(v.to_string())
    }

    fn serialize_f32(self, v: f32) -> Result<(), Error> {
        self.require_nested()?;
        self.fmt.string(v.to_string())
    }

    fn serialize_f64(self, v: f64) -> Result<(), Error> {
        self.require_nested()?;
        self.fmt.string(v.to_string())
    }

    fn serialize_char(self, v: char) -> Result<(), Error> {
        self.require_nested()?;
        self.fmt.string(v.to_string())
    }

    fn serialize_str(self, v: &str) -> Result<(), Error> {
        self.require_nested()?;
        self.fmt.string(v.to_string())
    }

    fn serialize_bytes(self, _v: &[u8]) -> Result<(), Error> {
        Err(Error::new("byte arrays are not supported"))
    }

    fn serialize_none(self) -> Result<(), Error> {
        self.require_nested()?;
        self.fmt.string(String::new())
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<(), Error> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<(), Error> {
        self.require_nested()?;
        self.fmt.string(String::new())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<(), Error> {
        self.require_nested()?;
        self.fmt.string(String::new())
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _idx: u32,
        variant: &'static str,
    ) -> Result<(), Error> {
        self.require_nested()?;
        self.fmt.string(variant.to_string())
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _idx: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.require_nested()?;
        self.fmt.map_start()?;
        self.depth += 1;
        self.fmt.key(variant.to_string())?;
        value.serialize(&mut *self)?;
        self.depth -= 1;
        self.fmt.map_end()
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<CompoundSeq<'a, W>, Error> {
        self.require_nested()?;
        self.fmt.list_start()?;
        self.depth += 1;
        Ok(CompoundSeq { ser: self })
    }

    fn serialize_tuple(self, _len: usize) -> Result<CompoundSeq<'a, W>, Error> {
        self.require_nested()?;
        self.fmt.list_start()?;
        self.depth += 1;
        Ok(CompoundSeq { ser: self })
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<CompoundSeq<'a, W>, Error> {
        self.require_nested()?;
        self.fmt.list_start()?;
        self.depth += 1;
        Ok(CompoundSeq { ser: self })
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _idx: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<CompoundTupleVariant<'a, W>, Error> {
        self.require_nested()?;
        self.fmt.map_start()?;
        self.depth += 1;
        self.fmt.key(variant.to_string())?;
        self.fmt.list_start()?;
        self.depth += 1;
        Ok(CompoundTupleVariant { ser: self })
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<CompoundMap<'a, W>, Error> {
        self.fmt.map_start()?;
        self.depth += 1;
        Ok(CompoundMap { ser: self })
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<CompoundMap<'a, W>, Error> {
        self.fmt.map_start()?;
        self.depth += 1;
        Ok(CompoundMap { ser: self })
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _idx: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<CompoundStructVariant<'a, W>, Error> {
        self.require_nested()?;
        self.fmt.map_start()?;
        self.depth += 1;
        self.fmt.key(variant.to_string())?;
        self.fmt.map_start()?;
        self.depth += 1;
        Ok(CompoundStructVariant { ser: self })
    }
}

struct CompoundSeq<'a, W: io::Write> {
    ser: &'a mut StreamSerializer<W>,
}

impl<W: io::Write> ser::SerializeSeq for CompoundSeq<'_, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<(), Error> {
        self.ser.depth -= 1;
        self.ser.fmt.list_end()
    }
}

impl<W: io::Write> ser::SerializeTuple for CompoundSeq<'_, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<(), Error> {
        ser::SerializeSeq::end(self)
    }
}

impl<W: io::Write> ser::SerializeTupleStruct for CompoundSeq<'_, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<(), Error> {
        ser::SerializeSeq::end(self)
    }
}

struct CompoundMap<'a, W: io::Write> {
    ser: &'a mut StreamSerializer<W>,
}

impl<W: io::Write> ser::SerializeMap for CompoundMap<'_, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Error> {
        let k = key.serialize(KeySerializer)?;
        self.ser.fmt.key(k)
    }

    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<(), Error> {
        self.ser.depth -= 1;
        self.ser.fmt.map_end()
    }
}

impl<W: io::Write> ser::SerializeStruct for CompoundMap<'_, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.ser.fmt.key(key.to_string())?;
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<(), Error> {
        self.ser.depth -= 1;
        self.ser.fmt.map_end()
    }
}

struct CompoundTupleVariant<'a, W: io::Write> {
    ser: &'a mut StreamSerializer<W>,
}

impl<W: io::Write> ser::SerializeTupleVariant for CompoundTupleVariant<'_, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<(), Error> {
        self.ser.depth -= 1;
        self.ser.fmt.list_end()?;
        self.ser.depth -= 1;
        self.ser.fmt.map_end()
    }
}

struct CompoundStructVariant<'a, W: io::Write> {
    ser: &'a mut StreamSerializer<W>,
}

impl<W: io::Write> ser::SerializeStructVariant for CompoundStructVariant<'_, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.ser.fmt.key(key.to_string())?;
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<(), Error> {
        self.ser.depth -= 1;
        self.ser.fmt.map_end()?;
        self.ser.depth -= 1;
        self.ser.fmt.map_end()
    }
}

struct KeySerializer;

impl ser::Serializer for KeySerializer {
    type Ok = String;
    type Error = Error;
    type SerializeSeq = Impossible<String, Error>;
    type SerializeTuple = Impossible<String, Error>;
    type SerializeTupleStruct = Impossible<String, Error>;
    type SerializeTupleVariant = Impossible<String, Error>;
    type SerializeMap = Impossible<String, Error>;
    type SerializeStruct = Impossible<String, Error>;
    type SerializeStructVariant = Impossible<String, Error>;

    fn serialize_bool(self, v: bool) -> Result<String, Error> {
        Ok(v.to_string())
    }

    fn serialize_i8(self, v: i8) -> Result<String, Error> {
        Ok(v.to_string())
    }

    fn serialize_i16(self, v: i16) -> Result<String, Error> {
        Ok(v.to_string())
    }

    fn serialize_i32(self, v: i32) -> Result<String, Error> {
        Ok(v.to_string())
    }

    fn serialize_i64(self, v: i64) -> Result<String, Error> {
        Ok(v.to_string())
    }

    fn serialize_u8(self, v: u8) -> Result<String, Error> {
        Ok(v.to_string())
    }

    fn serialize_u16(self, v: u16) -> Result<String, Error> {
        Ok(v.to_string())
    }

    fn serialize_u32(self, v: u32) -> Result<String, Error> {
        Ok(v.to_string())
    }

    fn serialize_u64(self, v: u64) -> Result<String, Error> {
        Ok(v.to_string())
    }

    fn serialize_f32(self, v: f32) -> Result<String, Error> {
        Ok(v.to_string())
    }

    fn serialize_f64(self, v: f64) -> Result<String, Error> {
        Ok(v.to_string())
    }

    fn serialize_char(self, v: char) -> Result<String, Error> {
        Ok(v.to_string())
    }

    fn serialize_str(self, v: &str) -> Result<String, Error> {
        Ok(v.to_string())
    }

    fn serialize_bytes(self, _v: &[u8]) -> Result<String, Error> {
        Err(Error::new("map keys must be strings"))
    }

    fn serialize_none(self) -> Result<String, Error> {
        Err(Error::new("map keys must be strings"))
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<String, Error> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<String, Error> {
        Err(Error::new("map keys must be strings"))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<String, Error> {
        Err(Error::new("map keys must be strings"))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _idx: u32,
        variant: &'static str,
    ) -> Result<String, Error> {
        Ok(variant.to_string())
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<String, Error> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _idx: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<String, Error> {
        Err(Error::new("map keys must be strings"))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Impossible<String, Error>, Error> {
        Err(Error::new("map keys must be strings"))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Impossible<String, Error>, Error> {
        Err(Error::new("map keys must be strings"))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Impossible<String, Error>, Error> {
        Err(Error::new("map keys must be strings"))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _idx: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Impossible<String, Error>, Error> {
        Err(Error::new("map keys must be strings"))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Impossible<String, Error>, Error> {
        Err(Error::new("map keys must be strings"))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Impossible<String, Error>, Error> {
        Err(Error::new("map keys must be strings"))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _idx: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Impossible<String, Error>, Error> {
        Err(Error::new("map keys must be strings"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[test]
    fn simple_struct() {
        #[derive(Serialize)]
        struct Config {
            name: String,
            port: u16,
        }

        let s = to_string(&Config {
            name: "app".into(),
            port: 8080,
        })
        .unwrap();
        assert_eq!(s, "name app\nport 8080\n");
    }

    #[test]
    fn nested_struct() {
        #[derive(Serialize)]
        struct Inner {
            host: String,
            port: u16,
        }

        #[derive(Serialize)]
        struct Config {
            server: Inner,
        }

        let s = to_string(&Config {
            server: Inner {
                host: "localhost".into(),
                port: 3000,
            },
        })
        .unwrap();
        assert_eq!(s, "server {\n  host localhost\n  port 3000\n}\n");
    }

    #[test]
    fn list_field() {
        #[derive(Serialize)]
        struct Config {
            tags: Vec<String>,
        }

        let s = to_string(&Config {
            tags: vec!["web".into(), "prod".into()],
        })
        .unwrap();
        assert_eq!(s, "tags [web prod]\n");
    }

    #[test]
    fn quoted_strings() {
        #[derive(Serialize)]
        struct Config {
            msg: String,
        }

        let s = to_string(&Config {
            msg: "hello world".into(),
        })
        .unwrap();
        assert_eq!(s, "msg \"hello world\"\n");
    }

    #[test]
    fn escape_sequences() {
        #[derive(Serialize)]
        struct Config {
            msg: String,
        }

        let s = to_string(&Config {
            msg: "line1\nline2".into(),
        })
        .unwrap();
        assert_eq!(s, "msg \"line1\\nline2\"\n");
    }

    #[test]
    fn bool_fields() {
        #[derive(Serialize)]
        struct Config {
            debug: bool,
        }

        let s = to_string(&Config { debug: true }).unwrap();
        assert_eq!(s, "debug true\n");
    }

    #[test]
    fn empty_map() {
        let s = to_string(&std::collections::HashMap::<String, String>::new()).unwrap();
        assert_eq!(s, "");
    }

    #[test]
    fn top_level_string_rejected() {
        assert!(to_string(&"hello").is_err());
    }

    #[test]
    fn top_level_list_rejected() {
        assert!(to_string(&vec!["a", "b"]).is_err());
    }

    #[test]
    fn empty_string_quoted() {
        #[derive(Serialize)]
        struct Config {
            name: String,
        }

        let s = to_string(&Config {
            name: String::new(),
        })
        .unwrap();
        assert_eq!(s, "name \"\"\n");
    }

    #[test]
    fn nbsp_quoted_roundtrip() {
        #[derive(Serialize, Deserialize, Debug, PartialEq)]
        struct Config {
            name: String,
        }

        let cfg = Config {
            name: "foo\u{00a0}bar".into(),
        };
        let s = to_string(&cfg).unwrap();
        assert!(s.contains('"'), "nbsp value must be quoted");
        let cfg2: Config = crate::from_str(&s).unwrap();
        assert_eq!(cfg, cfg2);
    }
}
