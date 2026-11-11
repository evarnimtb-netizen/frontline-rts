//! Load-time bounds shared by the save validators (found by the save
//! mutation fuzz, `crates/sim/tests/save_fuzz.rs`). Values a real match can
//! never reach are rejected, so the `+=` on counters, ticks and id counters
//! (overflow-checked in every build) cannot panic after a load.

use serde::Serialize;
use serde::ser::{self, Impossible};

/// Highest tick a save may be at (about 580 years at 60 Hz). Validators add
/// tick-relative windows to it.
pub(crate) const MAX_TICK: u64 = 1 << 40;

/// Highest value a `next_*` id counter may hold in a save: half the range,
/// so ids handed out after the load never reach `u32::MAX`.
pub(crate) const MAX_ID_COUNTER: u32 = 1 << 31;

/// Whether every integer in the counter struct `v` is at most half its
/// type's range (`u32` ≤ 2^31, `u64` ≤ 2^62), so later increments cannot
/// overflow. For plain counter structs only: other value kinds fail.
pub(crate) fn counters_ok<T: Serialize + ?Sized>(v: &T) -> bool {
    v.serialize(Counters).is_ok()
}

#[derive(Debug)]
struct Over;
impl std::fmt::Display for Over {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("counter out of range")
    }
}
impl std::error::Error for Over {}
impl ser::Error for Over {
    fn custom<T: std::fmt::Display>(_: T) -> Self {
        Over
    }
}

#[derive(Clone, Copy)]
struct Counters;

type R = Result<(), Over>;

fn at_most(v: u64, max: u64) -> R {
    if v <= max { Ok(()) } else { Err(Over) }
}

impl ser::Serializer for Counters {
    type Ok = ();
    type Error = Over;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Impossible<(), Over>;
    type SerializeTupleVariant = Impossible<(), Over>;
    type SerializeMap = Impossible<(), Over>;
    type SerializeStruct = Self;
    type SerializeStructVariant = Impossible<(), Over>;

    fn serialize_u8(self, _: u8) -> R {
        Ok(())
    }
    fn serialize_u16(self, _: u16) -> R {
        Ok(())
    }
    fn serialize_u32(self, v: u32) -> R {
        at_most(v as u64, 1 << 31)
    }
    fn serialize_u64(self, v: u64) -> R {
        at_most(v, 1 << 62)
    }
    fn serialize_bool(self, _: bool) -> R {
        Err(Over)
    }
    fn serialize_i8(self, _: i8) -> R {
        Err(Over)
    }
    fn serialize_i16(self, _: i16) -> R {
        Err(Over)
    }
    fn serialize_i32(self, _: i32) -> R {
        Err(Over)
    }
    fn serialize_i64(self, _: i64) -> R {
        Err(Over)
    }
    fn serialize_f32(self, _: f32) -> R {
        Err(Over)
    }
    fn serialize_f64(self, _: f64) -> R {
        Err(Over)
    }
    fn serialize_char(self, _: char) -> R {
        Err(Over)
    }
    fn serialize_str(self, _: &str) -> R {
        Err(Over)
    }
    fn serialize_bytes(self, _: &[u8]) -> R {
        Err(Over)
    }
    fn serialize_none(self) -> R {
        Err(Over)
    }
    fn serialize_some<T: Serialize + ?Sized>(self, _: &T) -> R {
        Err(Over)
    }
    fn serialize_unit(self) -> R {
        Err(Over)
    }
    fn serialize_unit_struct(self, _: &'static str) -> R {
        Err(Over)
    }
    fn serialize_unit_variant(self, _: &'static str, _: u32, _: &'static str) -> R {
        Err(Over)
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(self, _: &'static str, v: &T) -> R {
        v.serialize(self)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: &T,
    ) -> R {
        Err(Over)
    }
    fn serialize_seq(self, _: Option<usize>) -> Result<Self, Over> {
        Ok(self)
    }
    fn serialize_tuple(self, _: usize) -> Result<Self, Over> {
        Ok(self)
    }
    fn serialize_tuple_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleStruct, Over> {
        Err(Over)
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleVariant, Over> {
        Err(Over)
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Over> {
        Err(Over)
    }
    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self, Over> {
        Ok(self)
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStructVariant, Over> {
        Err(Over)
    }
}

impl ser::SerializeSeq for Counters {
    type Ok = ();
    type Error = Over;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, v: &T) -> R {
        v.serialize(*self)
    }
    fn end(self) -> R {
        Ok(())
    }
}

impl ser::SerializeTuple for Counters {
    type Ok = ();
    type Error = Over;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, v: &T) -> R {
        v.serialize(*self)
    }
    fn end(self) -> R {
        Ok(())
    }
}

impl ser::SerializeStruct for Counters {
    type Ok = ();
    type Error = Over;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, _: &'static str, v: &T) -> R {
        v.serialize(*self)
    }
    fn end(self) -> R {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_ok_bounds_each_integer_kind() {
        #[derive(Serialize, Clone)]
        struct C {
            a: u32,
            b: u64,
            c: [u32; 2],
            d: Vec<u64>,
        }
        let ok = C {
            a: 1 << 31,
            b: 1 << 62,
            c: [0, 5],
            d: vec![7],
        };
        assert!(counters_ok(&ok));
        assert!(!counters_ok(&C { a: u32::MAX, ..ok }));
        let ok = C {
            a: 0,
            b: 1 << 62,
            c: [0, 5],
            d: vec![7],
        };
        assert!(!counters_ok(&C {
            b: u64::MAX - 1,
            ..ok
        }));
        let ok = C {
            a: 0,
            b: 0,
            c: [0, 5],
            d: vec![7],
        };
        assert!(!counters_ok(&C {
            c: [0, u32::MAX - 1],
            ..ok.clone()
        }));
        assert!(!counters_ok(&C {
            d: vec![1, u64::MAX],
            ..ok
        }));
        assert!(!counters_ok(&Some(1u32)), "only plain counters");
    }
}
