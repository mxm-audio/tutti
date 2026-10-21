//! The keys a document carries that this version doesn't know (spec 8.2), found where the model
//! keeps them: each object's `unknown`. A reader keeps them and writes them back, so one is never
//! an error; but a misspelt field reads the same way (`voice` for a note's `audio`) and would pass
//! without a word, so `tutti check` notes each one.
//!
//! The walk is the document's own serialisation: a key of an `unknown` map serialises as a
//! [`Key`](crate::model::Key) newtype, which a field's name never does, so every object that keeps
//! unknown keys is covered as the model grows, with nothing to list here. An unknown key's value,
//! an extension's and an unknown event's are not looked into: they are kept whole.

use std::collections::HashMap;
use std::fmt;

use serde::Serialize;
use serde::ser::{self, Impossible};

use crate::model::{Document, KEY_NAME};

/// A key this version doesn't know, once for every object it was found in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownKey {
    pub key: String,
    /// The JSON pointer of the first place it was found, in the document's order.
    pub pointer: String,
    /// How many objects carry it.
    pub count: usize,
}

/// Every key the document carries that this version doesn't know, once each, in the order they
/// first appear.
pub fn unknown_keys(document: &Document) -> Vec<UnknownKey> {
    let mut walk = Walk::default();
    document
        .serialize(&mut walk)
        .expect("a document always serialises");
    walk.found
}

#[derive(Default)]
struct Walk {
    path: Vec<String>,
    found: Vec<UnknownKey>,
    index: HashMap<String, usize>,
}

impl Walk {
    fn pointer(&self) -> String {
        self.path
            .iter()
            .map(|part| format!("/{}", part.replace('~', "~0").replace('/', "~1")))
            .collect()
    }

    fn found(&mut self, key: String) {
        self.path.push(key.clone());
        let pointer = self.pointer();
        self.path.pop();
        match self.index.get(&key) {
            Some(&at) => self.found[at].count += 1,
            None => {
                self.index.insert(key.clone(), self.found.len());
                self.found.push(UnknownKey {
                    key,
                    pointer,
                    count: 1,
                });
            }
        }
    }
}

/// The walk never fails; serde asks for an error type all the same.
#[derive(Debug)]
struct Never;

impl fmt::Display for Never {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("not a key")
    }
}

impl std::error::Error for Never {}

impl ser::Error for Never {
    fn custom<T: fmt::Display>(_: T) -> Self {
        Never
    }
}

/// A sequence, a map or a struct being walked; `pop` when it was entered under a variant's name.
struct Inside<'a> {
    walk: &'a mut Walk,
    at: usize,
    key: Option<(String, bool)>,
    pop: bool,
}

impl<'a> Inside<'a> {
    fn new(walk: &'a mut Walk, pop: bool) -> Self {
        Self {
            walk,
            at: 0,
            key: None,
            pop,
        }
    }

    fn under<T: ?Sized + Serialize>(&mut self, name: String, value: &T) -> Result<(), Never> {
        self.walk.path.push(name);
        value.serialize(&mut *self.walk)?;
        self.walk.path.pop();
        Ok(())
    }

    fn element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Never> {
        self.at += 1;
        self.under((self.at - 1).to_string(), value)
    }

    fn close(self) -> Result<(), Never> {
        if self.pop {
            self.walk.path.pop();
        }
        Ok(())
    }
}

macro_rules! nothing {
    ($($name:ident: $ty:ty),* $(,)?) => {
        $(fn $name(self, _: $ty) -> Result<(), Never> { Ok(()) })*
    };
}

impl<'a> ser::Serializer for &'a mut Walk {
    type Ok = ();
    type Error = Never;
    type SerializeSeq = Inside<'a>;
    type SerializeTuple = Inside<'a>;
    type SerializeTupleStruct = Inside<'a>;
    type SerializeTupleVariant = Inside<'a>;
    type SerializeMap = Inside<'a>;
    type SerializeStruct = Inside<'a>;
    type SerializeStructVariant = Inside<'a>;

    nothing!(
        serialize_bool: bool, serialize_i8: i8, serialize_i16: i16, serialize_i32: i32,
        serialize_i64: i64, serialize_u8: u8, serialize_u16: u16, serialize_u32: u32,
        serialize_u64: u64, serialize_f32: f32, serialize_f64: f64, serialize_char: char,
        serialize_str: &str, serialize_bytes: &[u8], serialize_unit_struct: &'static str,
    );

    fn serialize_none(self) -> Result<(), Never> {
        Ok(())
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<(), Never> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<(), Never> {
        Ok(())
    }

    fn serialize_unit_variant(self, _: &'static str, _: u32, _: &'static str) -> Result<(), Never> {
        Ok(())
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        value: &T,
    ) -> Result<(), Never> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<(), Never> {
        self.path.push(variant.to_owned());
        value.serialize(&mut *self)?;
        self.path.pop();
        Ok(())
    }

    fn serialize_seq(self, _: Option<usize>) -> Result<Inside<'a>, Never> {
        Ok(Inside::new(self, false))
    }

    fn serialize_tuple(self, _: usize) -> Result<Inside<'a>, Never> {
        Ok(Inside::new(self, false))
    }

    fn serialize_tuple_struct(self, _: &'static str, _: usize) -> Result<Inside<'a>, Never> {
        Ok(Inside::new(self, false))
    }

    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        _: usize,
    ) -> Result<Inside<'a>, Never> {
        self.path.push(variant.to_owned());
        Ok(Inside::new(self, true))
    }

    fn serialize_map(self, _: Option<usize>) -> Result<Inside<'a>, Never> {
        Ok(Inside::new(self, false))
    }

    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Inside<'a>, Never> {
        Ok(Inside::new(self, false))
    }

    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        _: usize,
    ) -> Result<Inside<'a>, Never> {
        self.path.push(variant.to_owned());
        Ok(Inside::new(self, true))
    }
}

impl ser::SerializeSeq for Inside<'_> {
    type Ok = ();
    type Error = Never;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Never> {
        self.element(value)
    }

    fn end(self) -> Result<(), Never> {
        self.close()
    }
}

impl ser::SerializeTuple for Inside<'_> {
    type Ok = ();
    type Error = Never;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Never> {
        self.element(value)
    }

    fn end(self) -> Result<(), Never> {
        self.close()
    }
}

impl ser::SerializeTupleStruct for Inside<'_> {
    type Ok = ();
    type Error = Never;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Never> {
        self.element(value)
    }

    fn end(self) -> Result<(), Never> {
        self.close()
    }
}

impl ser::SerializeTupleVariant for Inside<'_> {
    type Ok = ();
    type Error = Never;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Never> {
        self.element(value)
    }

    fn end(self) -> Result<(), Never> {
        self.close()
    }
}

impl ser::SerializeMap for Inside<'_> {
    type Ok = ();
    type Error = Never;

    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<(), Never> {
        let (name, unknown) = key.serialize(KeyName)?;
        if unknown {
            self.walk.found(name.clone());
        }
        self.key = Some((name, unknown));
        Ok(())
    }

    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Never> {
        match self.key.take() {
            // An unknown key's value is kept whole, never looked into.
            Some((_, true)) => Ok(()),
            Some((name, false)) => self.under(name, value),
            None => Err(Never),
        }
    }

    fn end(self) -> Result<(), Never> {
        self.close()
    }
}

impl ser::SerializeStruct for Inside<'_> {
    type Ok = ();
    type Error = Never;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        name: &'static str,
        value: &T,
    ) -> Result<(), Never> {
        self.under(name.to_owned(), value)
    }

    fn end(self) -> Result<(), Never> {
        self.close()
    }
}

impl ser::SerializeStructVariant for Inside<'_> {
    type Ok = ();
    type Error = Never;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        name: &'static str,
        value: &T,
    ) -> Result<(), Never> {
        self.under(name.to_owned(), value)
    }

    fn end(self) -> Result<(), Never> {
        self.close()
    }
}

/// A map key as text, and whether it is an unknown field's ([`Key`](crate::model::Key)).
struct KeyName;

macro_rules! text {
    ($($name:ident: $ty:ty),* $(,)?) => {
        $(fn $name(self, v: $ty) -> Result<(String, bool), Never> { Ok((v.to_string(), false)) })*
    };
}

impl ser::Serializer for KeyName {
    type Ok = (String, bool);
    type Error = Never;
    type SerializeSeq = Impossible<(String, bool), Never>;
    type SerializeTuple = Impossible<(String, bool), Never>;
    type SerializeTupleStruct = Impossible<(String, bool), Never>;
    type SerializeTupleVariant = Impossible<(String, bool), Never>;
    type SerializeMap = Impossible<(String, bool), Never>;
    type SerializeStruct = Impossible<(String, bool), Never>;
    type SerializeStructVariant = Impossible<(String, bool), Never>;

    text!(
        serialize_bool: bool, serialize_i8: i8, serialize_i16: i16, serialize_i32: i32,
        serialize_i64: i64, serialize_u8: u8, serialize_u16: u16, serialize_u32: u32,
        serialize_u64: u64, serialize_f32: f32, serialize_f64: f64, serialize_char: char,
        serialize_str: &str,
    );

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<(String, bool), Never> {
        let (text, unknown) = value.serialize(self)?;
        Ok((text, unknown || name == KEY_NAME))
    }

    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
    ) -> Result<(String, bool), Never> {
        Ok((variant.to_owned(), false))
    }

    fn serialize_bytes(self, _: &[u8]) -> Result<(String, bool), Never> {
        Err(Never)
    }

    fn serialize_none(self) -> Result<(String, bool), Never> {
        Err(Never)
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<(String, bool), Never> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<(String, bool), Never> {
        Err(Never)
    }

    fn serialize_unit_struct(self, _: &'static str) -> Result<(String, bool), Never> {
        Err(Never)
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: &T,
    ) -> Result<(String, bool), Never> {
        Err(Never)
    }

    fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq, Never> {
        Err(Never)
    }

    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, Never> {
        Err(Never)
    }

    fn serialize_tuple_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleStruct, Never> {
        Err(Never)
    }

    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleVariant, Never> {
        Err(Never)
    }

    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Never> {
        Err(Never)
    }

    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self::SerializeStruct, Never> {
        Err(Never)
    }

    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStructVariant, Never> {
        Err(Never)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read;

    #[test]
    fn each_unknown_key_is_found_once_with_where_it_first_is() {
        let text = r#"{
  "format": "tutti", "version": "1.0", "generator": {"name": "t", "build": 7}, "ppq": 960,
  "tempo": [{"at": 0, "bpm": 120}], "meter": [{"at": 0, "numerator": 4, "denominator": 4}],
  "alpha": {"voice": 1},
  "extensions": {"dk.mxm.x": {"voice": 2}},
  "tracks": [{"id": "trk_a", "name": "A", "patterns": [{"id": "pat_1", "length": 3840, "events": [
      {"type": "note", "id": "evt_1", "at": 0, "length": 480, "pitch": 60, "velocity": 1, "voice": "a.wav"},
      {"type": "note", "id": "evt_2", "at": 480, "length": 480, "pitch": 62, "velocity": 1, "voice": "b.wav"},
      {"type": "hum", "id": "evt_3", "at": 0, "voice": 3}
    ]}]}]
}"#;
        let doc = read(text.as_bytes()).unwrap();
        let found = unknown_keys(&doc);
        let shown: Vec<(&str, &str, usize)> = found
            .iter()
            .map(|k| (k.key.as_str(), k.pointer.as_str(), k.count))
            .collect();
        // The generator's, the document's, and the notes' once for both; not inside an unknown
        // key's value, an extension or an event of a kind this version doesn't know.
        assert_eq!(
            shown,
            [
                ("build", "/generator/build", 1),
                ("voice", "/tracks/0/patterns/0/events/0/voice", 2),
                ("alpha", "/alpha", 1),
            ]
        );
    }

    #[test]
    fn a_document_of_known_keys_has_none() {
        let doc = Document::new(crate::Generator::new("t", "1", "https://example.com/t"));
        assert_eq!(unknown_keys(&doc), []);
    }

    #[test]
    fn a_pointer_escapes_its_parts() {
        let text = r#"{"format": "tutti", "version": "1.0", "generator": {"name": "t"}, "ppq": 960,
  "tempo": [{"at": 0, "bpm": 120}], "meter": [{"at": 0, "numerator": 4, "denominator": 4}],
  "a/b~c": true}"#;
        let found = unknown_keys(&read(text.as_bytes()).unwrap());
        assert_eq!(found[0].pointer, "/a~1b~0c");
    }
}
