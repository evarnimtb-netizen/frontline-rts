//! Byte layout of a postcard save: a serde serializer that writes the same
//! bytes as `postcard::to_allocvec` and records, for every primitive, its
//! offset, length, kind and field path. The fuzz test (`save_fuzz.rs`)
//! uses it to mutate one value at a time; the test checks the bytes against
//! `World::save` so a postcard change shows up as a failed self-check, not
//! as silently wrong offsets.

use std::collections::BTreeMap;
use std::fmt::Display;

use serde::ser::{self, Serialize};

/// What a recorded value is, as encoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Bool,
    /// Raw byte (`u8`).
    U8,
    I8,
    /// Unsigned varint of this many bits.
    U(u32),
    /// Zigzag varint of this many bits.
    I(u32),
    F32,
    F64,
    /// Sequence, map, string or byte-string length (varint).
    Len,
    /// `Option` tag byte.
    Tag,
    /// Enum variant index (varint).
    Variant,
}

#[derive(Clone, Copy, Debug)]
pub struct Field {
    pub offset: usize,
    pub len: usize,
    pub kind: Kind,
    /// Index into [`Layout::paths`].
    pub path: u32,
    /// Value as encoded (zigzag decoded for signed kinds).
    pub value: i128,
}

#[derive(Default, Clone)]
pub struct Layout {
    pub bytes: Vec<u8>,
    pub fields: Vec<Field>,
    /// Interned field paths, e.g. `state.game.fort.works.[].hp`.
    pub paths: Vec<String>,
    intern: BTreeMap<Vec<&'static str>, u32>,
    stack: Vec<&'static str>,
}

#[derive(Debug)]
pub struct Error(String);
impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}
impl ser::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error(msg.to_string())
    }
}

pub fn varint(mut v: u128, out: &mut Vec<u8>) {
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return;
        }
        out.push(b | 0x80);
    }
}

pub fn zigzag(v: i128) -> u128 {
    ((v << 1) ^ (v >> 127)) as u128
}

impl Layout {
    pub fn of<T: Serialize + ?Sized>(v: &T) -> Layout {
        let mut l = Layout::default();
        l.stack.push("save");
        v.serialize(&mut l).expect("layout serialiser");
        l
    }

    fn path_id(&mut self) -> u32 {
        if let Some(&i) = self.intern.get(&self.stack) {
            return i;
        }
        let i = self.paths.len() as u32;
        let p: Vec<&str> = self.stack[1..]
            .iter()
            .copied()
            .filter(|s| !s.is_empty())
            .collect();
        self.paths.push(p.join("."));
        self.intern.insert(self.stack.clone(), i);
        i
    }

    fn rec(&mut self, kind: Kind, value: i128, enc: impl FnOnce(&mut Vec<u8>)) {
        let offset = self.bytes.len();
        enc(&mut self.bytes);
        let path = self.path_id();
        self.fields.push(Field {
            offset,
            len: self.bytes.len() - offset,
            kind,
            path,
            value,
        });
    }

    fn uvar(&mut self, kind: Kind, v: u128) {
        self.rec(kind, v as i128, |b| varint(v, b));
    }

    fn ivar(&mut self, bits: u32, v: i128) {
        self.rec(Kind::I(bits), v, |b| varint(zigzag(v), b));
    }

    fn len(&mut self, n: Option<usize>) -> Result<(), Error> {
        let n = n.ok_or_else(|| Error("sequence without a length".into()))?;
        self.stack.push("len");
        self.uvar(Kind::Len, n as u128);
        self.stack.pop();
        Ok(())
    }

    fn variant(&mut self, name: &'static str, idx: u32) {
        self.stack.push(name);
        self.stack.push("variant");
        self.uvar(Kind::Variant, idx as u128);
        self.stack.pop();
    }
}

type R = Result<(), Error>;

impl ser::Serializer for &mut Layout {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fn serialize_bool(self, v: bool) -> R {
        self.rec(Kind::Bool, v as i128, |b| b.push(v as u8));
        Ok(())
    }
    fn serialize_i8(self, v: i8) -> R {
        self.rec(Kind::I8, v as i128, |b| b.push(v as u8));
        Ok(())
    }
    fn serialize_i16(self, v: i16) -> R {
        self.ivar(16, v as i128);
        Ok(())
    }
    fn serialize_i32(self, v: i32) -> R {
        self.ivar(32, v as i128);
        Ok(())
    }
    fn serialize_i64(self, v: i64) -> R {
        self.ivar(64, v as i128);
        Ok(())
    }
    fn serialize_i128(self, v: i128) -> R {
        self.ivar(128, v);
        Ok(())
    }
    fn serialize_u8(self, v: u8) -> R {
        self.rec(Kind::U8, v as i128, |b| b.push(v));
        Ok(())
    }
    fn serialize_u16(self, v: u16) -> R {
        self.uvar(Kind::U(16), v as u128);
        Ok(())
    }
    fn serialize_u32(self, v: u32) -> R {
        self.uvar(Kind::U(32), v as u128);
        Ok(())
    }
    fn serialize_u64(self, v: u64) -> R {
        self.uvar(Kind::U(64), v as u128);
        Ok(())
    }
    fn serialize_u128(self, v: u128) -> R {
        self.uvar(Kind::U(128), v);
        Ok(())
    }
    fn serialize_f32(self, v: f32) -> R {
        self.rec(Kind::F32, 0, |b| b.extend_from_slice(&v.to_le_bytes()));
        Ok(())
    }
    fn serialize_f64(self, v: f64) -> R {
        self.rec(Kind::F64, 0, |b| b.extend_from_slice(&v.to_le_bytes()));
        Ok(())
    }
    fn serialize_char(self, v: char) -> R {
        let mut buf = [0u8; 4];
        self.serialize_str(v.encode_utf8(&mut buf))
    }
    fn serialize_str(self, v: &str) -> R {
        self.serialize_bytes(v.as_bytes())
    }
    fn serialize_bytes(self, v: &[u8]) -> R {
        self.len(Some(v.len()))?;
        self.bytes.extend_from_slice(v);
        Ok(())
    }
    fn serialize_none(self) -> R {
        self.rec(Kind::Tag, 0, |b| b.push(0));
        Ok(())
    }
    fn serialize_some<T: Serialize + ?Sized>(self, v: &T) -> R {
        self.rec(Kind::Tag, 1, |b| b.push(1));
        v.serialize(self)
    }
    fn serialize_unit(self) -> R {
        Ok(())
    }
    fn serialize_unit_struct(self, _: &'static str) -> R {
        Ok(())
    }
    fn serialize_unit_variant(self, name: &'static str, idx: u32, _: &'static str) -> R {
        self.variant(name, idx);
        self.stack.pop();
        Ok(())
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(self, _: &'static str, v: &T) -> R {
        v.serialize(self)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        idx: u32,
        var: &'static str,
        v: &T,
    ) -> R {
        self.variant(name, idx);
        self.stack.push(var);
        v.serialize(&mut *self)?;
        self.stack.pop();
        self.stack.pop();
        Ok(())
    }
    fn serialize_seq(self, n: Option<usize>) -> Result<Self, Error> {
        self.len(n)?;
        self.stack.push("[]");
        Ok(self)
    }
    fn serialize_tuple(self, _: usize) -> Result<Self, Error> {
        self.stack.push("()");
        Ok(self)
    }
    fn serialize_tuple_struct(self, _: &'static str, _: usize) -> Result<Self, Error> {
        self.stack.push("()");
        Ok(self)
    }
    fn serialize_tuple_variant(
        self,
        name: &'static str,
        idx: u32,
        var: &'static str,
        _: usize,
    ) -> Result<Self, Error> {
        self.variant(name, idx);
        self.stack.push(var);
        Ok(self)
    }
    fn serialize_map(self, n: Option<usize>) -> Result<Self, Error> {
        self.len(n)?;
        self.stack.push("{}");
        Ok(self)
    }
    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self, Error> {
        self.stack.push("");
        Ok(self)
    }
    fn serialize_struct_variant(
        self,
        name: &'static str,
        idx: u32,
        var: &'static str,
        _: usize,
    ) -> Result<Self, Error> {
        self.variant(name, idx);
        self.stack.push(var);
        Ok(self)
    }
}

impl ser::SerializeSeq for &mut Layout {
    type Ok = ();
    type Error = Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, v: &T) -> R {
        v.serialize(&mut **self)
    }
    fn end(self) -> R {
        self.stack.pop();
        Ok(())
    }
}
impl ser::SerializeTuple for &mut Layout {
    type Ok = ();
    type Error = Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, v: &T) -> R {
        v.serialize(&mut **self)
    }
    fn end(self) -> R {
        self.stack.pop();
        Ok(())
    }
}
impl ser::SerializeTupleStruct for &mut Layout {
    type Ok = ();
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, v: &T) -> R {
        v.serialize(&mut **self)
    }
    fn end(self) -> R {
        self.stack.pop();
        Ok(())
    }
}
impl ser::SerializeTupleVariant for &mut Layout {
    type Ok = ();
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, v: &T) -> R {
        v.serialize(&mut **self)
    }
    fn end(self) -> R {
        self.stack.pop();
        self.stack.pop();
        Ok(())
    }
}
impl ser::SerializeMap for &mut Layout {
    type Ok = ();
    type Error = Error;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, v: &T) -> R {
        self.stack.push("key");
        v.serialize(&mut **self)?;
        self.stack.pop();
        Ok(())
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, v: &T) -> R {
        v.serialize(&mut **self)
    }
    fn end(self) -> R {
        self.stack.pop();
        Ok(())
    }
}
impl ser::SerializeStruct for &mut Layout {
    type Ok = ();
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, key: &'static str, v: &T) -> R {
        self.stack.push(key);
        v.serialize(&mut **self)?;
        self.stack.pop();
        Ok(())
    }
    fn end(self) -> R {
        self.stack.pop();
        Ok(())
    }
}
impl ser::SerializeStructVariant for &mut Layout {
    type Ok = ();
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, key: &'static str, v: &T) -> R {
        self.stack.push(key);
        v.serialize(&mut **self)?;
        self.stack.pop();
        Ok(())
    }
    fn end(self) -> R {
        self.stack.pop();
        self.stack.pop();
        Ok(())
    }
}
