//! Low-level APIs for (de)serializing Rust primitives from/to TTLV bytes.
//!
//! Using the types in this module you can deserialize TTLV bytes to Rust equivalents of the TTLV header fields and
//! primitive TTLV value types, and vice versa.
//!
//! For example:
//!
//! ```
//! use kmip_ttlv::types::{TtlvTag, TtlvType, TtlvLength, TtlvInteger};
//! use kmip_ttlv::types::SerializableTtlvType;
//! # fn main() -> kmip_ttlv::types::Result<()> {
//!
//! // Hand craft some TTLV bytes to deserialize
//! let mut ttlv_wire = Vec::new();
//! ttlv_wire.extend(b"\x66\x00\x01");     // 3-byte tag
//! ttlv_wire.extend(b"\x02");             // 1-byte type with value 2 (for Integer)
//! ttlv_wire.extend(b"\x00\x00\x00\x04"); // 4-byte length with value 4 (for a 4-byte value length)
//! ttlv_wire.extend(b"\x00\x00\x00\x03"); // 4-byte big-endian integer value 3
//! ttlv_wire.extend(b"\x00\x00\x00\x00"); // 4-byte padding
//!
//! // Create a cursor for "Read"ing from the buffer
//! let mut cursor = std::io::Cursor::new(&ttlv_wire);
//!
//! // Deserialize the TTLV bytes
//! let tag = TtlvTag::read(&mut cursor)?;
//! let typ = TtlvType::read(&mut cursor)?;
//! let val = TtlvInteger::read(&mut cursor)?; // reads the length and padding bytes as well
//!
//! // Verify the result
//! assert_eq!(*tag, 0x660001);
//! assert_eq!(typ, TtlvType::Integer);
//! assert_eq!(*val, 3);
//!
//! // Serialize the value back to TTLV bytes
//! let mut buf = Vec::new();
//! tag.write(&mut buf);
//! val.write(&mut buf); // writes the type, length, value and padding bytes
//!
//! // Verify that the serialized bytes match our handcrafted bytes
//! assert_eq!(&ttlv_wire, &buf);
//! # Ok(())
//! # }
//! ```
use std::{
    convert::TryFrom,
    fmt::{Debug, Display},
    io::{Read, Write},
    ops::Deref,
    str::FromStr,
};

// --- FieldType ------------------------------------------------------------------------------------------------------

/// The type of TTLV header or value field represented by some TTLV bytes.
///
/// This field is also used by the [TtlvStateMachine] to represent the next expected field type or types.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum FieldType {
    #[default]
    Tag,
    Type,
    Length,
    Value,
    LengthAndValue,        // used when deserializing
    TypeAndLengthAndValue, // used when serializing
}

impl Display for FieldType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldType::Tag => f.write_str("Tag"),
            FieldType::Type => f.write_str("Type"),
            FieldType::Length => f.write_str("Length"),
            FieldType::Value => f.write_str("Value"),
            FieldType::LengthAndValue => f.write_str("LengthAndValue"),
            FieldType::TypeAndLengthAndValue => {
                f.write_str("TypeAndLengthAndValue")
            }
        }
    }
}

// --- ByteOffset -----------------------------------------------------------------------------------------------------

/// An offset into a collection of TTLV bytes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ByteOffset(pub u64);

impl std::ops::Deref for ByteOffset {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<&u64> for ByteOffset {
    fn from(v: &u64) -> Self {
        ByteOffset(*v)
    }
}

impl From<u64> for ByteOffset {
    fn from(v: u64) -> Self {
        ByteOffset(v)
    }
}

impl TryFrom<usize> for ByteOffset {
    type Error = ();

    fn try_from(value: usize) -> std::result::Result<Self, Self::Error> {
        if value < (u64::MAX as usize) {
            Ok(ByteOffset(value as u64))
        } else {
            Err(())
        }
    }
}

impl<T> From<&std::io::Cursor<T>> for ByteOffset {
    fn from(cursor: &std::io::Cursor<T>) -> Self {
        ByteOffset(cursor.position())
    }
}

impl<T> From<std::io::Cursor<T>> for ByteOffset {
    fn from(cursor: std::io::Cursor<T>) -> Self {
        ByteOffset(cursor.position())
    }
}

/// Errors reported by the low-level (de)serialization API.
#[derive(Debug)]
#[allow(clippy::enum_variant_names)]
pub enum Error {
    IoError(std::io::Error),
    InvalidTtlvTag(String),
    UnexpectedTtlvField {
        expected: FieldType,
        actual: FieldType,
    },
    UnsupportedTtlvType(u8),
    InvalidTtlvType(u8),
    InvalidTtlvValueLength {
        expected: u32,
        actual: u32,
        r#type: TtlvType,
    },
    InvalidTtlvValue(TtlvType),
    InvalidStateMachineOperation,
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::IoError(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

// --- TtlvTag --------------------------------------------------------------------------------------------------------

/// A type for (de)serializing a TTLV Tag.
///
/// According to the [KMIP specification 1.0 section 9.1.1.1 Item Tag](http://docs.oasis-open.org/kmip/spec/v1.0/os/kmip-spec-1.0-os.html#_toc8560):
/// > _An Item Tag is a three-byte binary unsigned integer, transmitted big endian, which contains a number that
/// > designates the specific Protocol Field or Object that the TTLV object represents._
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TtlvTag(u32);

impl TtlvTag {
    pub fn read<T: Read>(src: &mut T) -> Result<Self> {
        let mut raw_item_tag = [0u8; 3];
        src.read_exact(&mut raw_item_tag)?;
        Ok(TtlvTag::from(raw_item_tag))
    }

    pub fn write<T: Write>(&self, dst: &mut T) -> Result<()> {
        dst.write_all(&<[u8; 3]>::from(self))
            .map_err(Error::IoError)
    }
}

impl Debug for TtlvTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("0x{:0X}", &self.0))
    }
}

impl Deref for TtlvTag {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for TtlvTag {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let v = u32::from_str_radix(s.trim_start_matches("0x"), 16)
            .map_err(|_| Error::InvalidTtlvTag(s.to_string()))?;
        Ok(TtlvTag(v))
    }
}

impl std::fmt::Display for TtlvTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:06X}", self)
    }
}

impl std::fmt::UpperHex for TtlvTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:X}", self.0)
    }
}

impl From<TtlvTag> for [u8; 3] {
    fn from(tag: TtlvTag) -> Self {
        <[u8; 3]>::from(&tag)
    }
}

impl From<&TtlvTag> for [u8; 3] {
    fn from(tag: &TtlvTag) -> Self {
        let b: [u8; 4] = tag.to_be_bytes();
        [b[1], b[2], b[3]]
    }
}

impl From<[u8; 3]> for TtlvTag {
    fn from(b: [u8; 3]) -> Self {
        TtlvTag(u32::from_be_bytes([0x00u8, b[0], b[1], b[2]]))
    }
}

impl From<&[u8; 3]> for TtlvTag {
    fn from(b: &[u8; 3]) -> Self {
        TtlvTag(u32::from_be_bytes([0x00u8, b[0], b[1], b[2]]))
    }
}

// --- TtlvType -------------------------------------------------------------------------------------------------------

/// A type for (de)serializing a TTLV Type.
///
/// According to the [KMIP specification 1.0 section 9.1.1.2 Item Type](http://docs.oasis-open.org/kmip/spec/v1.0/os/kmip-spec-1.0-os.html#_toc8562):
/// > _An Item Type is a byte containing a coded value that indicates the data type of the data object._
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TtlvType {
    Structure = 0x01,
    Integer = 0x02,
    LongInteger = 0x03,
    BigInteger = 0x04,
    Enumeration = 0x05,
    Boolean = 0x06,
    TextString = 0x07,
    ByteString = 0x08,
    DateTime = 0x09,
    // Interval = 0x0A,
}

impl TtlvType {
    pub fn read<T: Read>(src: &mut T) -> Result<Self> {
        let mut raw_item_type = [0u8; 1];
        src.read_exact(&mut raw_item_type)?;
        TtlvType::try_from(raw_item_type[0])
    }

    pub fn write<T: Write>(&self, dst: &mut T) -> Result<()> {
        dst.write_all(&[*self as u8]).map_err(Error::IoError)
    }
}

impl std::fmt::Display for TtlvType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TtlvType::Structure => f.write_str("Structure (0x01)"),
            TtlvType::Integer => f.write_str("Integer (0x02)"),
            TtlvType::LongInteger => f.write_str("LongInteger (0x03)"),
            TtlvType::BigInteger => f.write_str("BigInteger (0x04)"),
            TtlvType::Enumeration => f.write_str("Enumeration (0x05)"),
            TtlvType::Boolean => f.write_str("Boolean (0x06)"),
            TtlvType::TextString => f.write_str("TextString (0x07)"),
            TtlvType::ByteString => f.write_str("ByteString (0x08)"),
            TtlvType::DateTime => f.write_str("DateTime (0x09)"),
        }
    }
}

impl TryFrom<u8> for TtlvType {
    type Error = Error;

    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        match value {
            0x01 => Ok(TtlvType::Structure),
            0x02 => Ok(TtlvType::Integer),
            0x03 => Ok(TtlvType::LongInteger),
            0x04 => Ok(TtlvType::BigInteger),
            0x05 => Ok(TtlvType::Enumeration),
            0x06 => Ok(TtlvType::Boolean),
            0x07 => Ok(TtlvType::TextString),
            0x08 => Ok(TtlvType::ByteString),
            0x09 => Ok(TtlvType::DateTime),
            // 0x0A => Ok(TtlvType::Interval),
            0x0A => Err(Error::UnsupportedTtlvType(0x0A)),
            _ => Err(Error::InvalidTtlvType(value)),
        }
    }
}

impl From<TtlvType> for [u8; 1] {
    fn from(item_type: TtlvType) -> Self {
        [item_type as u8]
    }
}

// --- TtlvLength -----------------------------------------------------------------------------------------------------

/// A type for (de)serializing a TTLV Length.
///
/// According to the [KMIP specification 1.0 section 9.1.1.3 Item Length](http://docs.oasis-open.org/kmip/spec/v1.0/os/kmip-spec-1.0-os.html#_Toc236497868):
/// > _An Item Length is a 32-bit binary integer, transmitted big-endian, containing the number of bytes in the Item
/// > Value._
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TtlvLength(u32);

impl TtlvLength {
    pub fn new(value: u32) -> Self {
        Self(value)
    }

    pub fn read<T: Read>(src: &mut T) -> Result<Self> {
        let mut value_length = [0u8; 4];
        src.read_exact(&mut value_length)?;
        Ok(Self(u32::from_be_bytes(value_length)))
    }

    pub fn write<T: Write>(&self, dst: &mut T) -> Result<()> {
        dst.write_all(&self.0.to_be_bytes()).map_err(Error::IoError)
    }
}

impl Debug for TtlvLength {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("0x{:0X}", &self.0))
    }
}

impl Deref for TtlvLength {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for TtlvLength {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:08X}", self)
    }
}

impl std::fmt::UpperHex for TtlvLength {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:X}", self.0)
    }
}

// --- SerializableTtlvType ------------------------------------------------------------------------------------------------------

/// A type that knows how to (de)serialize itself from/to TTLV byte format.
///
/// This type provides a common interface for (de)serializing Rust companion types to their TTLV byte form equivalents.
///
/// It is also provides default implementations that handle the TTLV padding byte rules.
///
/// According to the [KMIP specification 1.0 section 9.1.1.3 Item Length](http://docs.oasis-open.org/kmip/spec/v1.0/os/kmip-spec-1.0-os.html#_Toc236497868):
/// > An Item Length is a 32-bit binary integer, transmitted big-endian, containing the number of bytes in the
/// > Item Value. The allowed values are:
/// >
/// >   Data Type    | Length
/// >   -------------|----------------------
/// >   Structure    | Varies, multiple of 8
/// >   Integer      | 4
/// >   Long Integer | 8
/// >   Big Integer  | Varies, multiple of 8
/// >   Enumeration  | 4
/// >   Boolean      | 8
/// >   Text String  | Varies
/// >   Byte String  | Varies
/// >   Date-Time    | 8
/// >   Interval     | 4
/// >
/// >   Table 192: Allowed Item Length Values
/// >
/// > If the Item Type is Structure, then the Item Length is the total length of all of the sub-items contained in
/// > the structure, including any padding. If the Item Type is Integer, Enumeration, Text String, Byte String, or
/// > Interval, then the Item Length is the number of bytes excluding the padding bytes. Text Strings and Byte
/// > Strings SHALL be padded with the minimal number of bytes following the Item Value to obtain a multiple
/// > of eight bytes. Integers, Enumerations, and Intervals SHALL be padded with four bytes following the Item
/// > Value.
pub trait SerializableTtlvType: Sized + Deref {
    const TTLV_TYPE: TtlvType;

    fn ttlv_type(&self) -> TtlvType {
        Self::TTLV_TYPE
    }

    fn calc_pad_bytes(value_len: u32) -> u32 {
        // pad to the next higher multiple of eight
        let remainder = value_len % 8;

        if remainder == 0 {
            // already on the alignment boundary, no need to add pad bytes to reach the boundary
            0
        } else {
            // for a shorter value, say 6 bytes, this calculates 8-(6%8) = 8-6 = 2, i.e. after having read 6 bytes the
            // next pad boundary is 2 bytes away.
            // for a longer value, say 10 bytes, this calcualtes 8-(10%8) = 8-2 = 6, i.e. after having read 10 bytes the
            // next pad boundary is 6 bytes away.
            8 - remainder
        }
    }

    fn read_pad_bytes<T: Read>(src: &mut T, value_len: u32) -> Result<()> {
        let num_pad_bytes = Self::calc_pad_bytes(value_len) as usize;
        if num_pad_bytes > 0 {
            let mut dst = [0u8; 8];
            src.read_exact(&mut dst[..num_pad_bytes])?;
        }
        Ok(())
    }

    fn write_pad_bytes<T: Write>(dst: &mut T, value_len: u32) -> Result<()> {
        let num_pad_bytes = Self::calc_pad_bytes(value_len) as usize;
        if num_pad_bytes > 0 {
            const PADDING_BYTES: [u8; 8] = [0; 8];
            dst.write_all(&PADDING_BYTES[..num_pad_bytes])?;
        }
        Ok(())
    }

    fn read<T: Read>(src: &mut T) -> Result<Self> {
        // The TTLV T_ype has already been read by the caller in order to determine which Primitive struct to use so
        // we only have to read the L_ength and and the V_alue.
        let mut value_len = [0u8; 4];
        src.read_exact(&mut value_len)?; // read L_ength
        let value_len = u32::from_be_bytes(value_len);
        let v = Self::read_value(src, value_len)?; // read V_alue
        Self::read_pad_bytes(src, value_len)?; // read 8-byte alignment padding bytes
        Ok(v)
    }

    // Writes the TLV part of TTLV, i.e. the type, length and value. It doesn't write the preceeding tag as that is
    // not part of the primitive value but is part of the callers context and only they can know which tag value to
    // write.
    fn write<T: Write>(&self, dst: &mut T) -> Result<()> {
        dst.write_all(&[Self::TTLV_TYPE as u8])?; // write T_ype
        let value_len = self.write_length_and_value(dst)?; // write L_ength and V_alue
        Self::write_pad_bytes(dst, value_len) // Write 8-byte alignment padding bytes
    }

    fn read_value<T: Read>(src: &mut T, value_len: u32) -> Result<Self>;

    fn write_length_and_value<T: Write>(&self, dst: &mut T) -> Result<u32>;
}

// E.g. simple_primitive!(MyType, ItemType::Integer, i32, 4) would define a new Rust struct called MyType which wraps an
// i32 value and implements the SerializableTtlvType trait to define how to read/write from/to a sequence of 4
// big-endian encoded bytes prefixed by a TTLV item type byte of value ItemType::Integer.
macro_rules! define_fixed_value_length_serializable_ttlv_type {
    ($(#[$meta:meta])* $NEW_TYPE_NAME:ident, $TTLV_ITEM_TYPE:expr, $RUST_TYPE:ty, $TTLV_VALUE_LEN:literal) => {
        #[derive(Clone, Debug)]
        $(#[$meta])*
        pub struct $NEW_TYPE_NAME(pub $RUST_TYPE);
        impl $NEW_TYPE_NAME {
            const TTLV_FIXED_VALUE_LENGTH: u32 = $TTLV_VALUE_LEN;
        }
        impl Deref for $NEW_TYPE_NAME {
            type Target = $RUST_TYPE;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
        impl SerializableTtlvType for $NEW_TYPE_NAME {
            const TTLV_TYPE: TtlvType = $TTLV_ITEM_TYPE;

            fn read_value<T: Read>(src: &mut T, value_len: u32) -> Result<Self> {
                if value_len != Self::TTLV_FIXED_VALUE_LENGTH {
                    Err(Error::InvalidTtlvValueLength {
                        expected: Self::TTLV_FIXED_VALUE_LENGTH,
                        actual: value_len,
                        r#type: Self::TTLV_TYPE,
                    })
                } else {
                    let mut dst = [0u8; Self::TTLV_FIXED_VALUE_LENGTH as usize];
                    src.read_exact(&mut dst)?;
                    let v: $RUST_TYPE = <$RUST_TYPE>::from_be_bytes(dst);
                    Ok($NEW_TYPE_NAME(v))
                }
            }

            fn write_length_and_value<T: Write>(&self, dst: &mut T) -> Result<u32> {
                dst.write_all(&Self::TTLV_FIXED_VALUE_LENGTH.to_be_bytes())?; // Write L_ength
                dst.write_all(&self.0.to_be_bytes())?; // Write V_alue
                Ok(Self::TTLV_FIXED_VALUE_LENGTH)
            }
        }
    };
}

// --- TtlvInteger ----------------------------------------------------------------------------------------------------

define_fixed_value_length_serializable_ttlv_type!(
    /// A type for (de)serializing a TTLV Integer.
    ///
    /// According to the [KMIP specification 1.0 section 9.1.1.4 Item Value](http://docs.oasis-open.org/kmip/spec/v1.0/os/kmip-spec-1.0-os.html#_Ref262577330):
    /// > _Integers are encoded as four-byte long (32 bit) binary signed numbers in 2's complement notation,
    /// > transmitted big-endian._
    TtlvInteger,
    TtlvType::Integer,
    i32,
    4
);

// --- TtlvLongInteger ------------------------------------------------------------------------------------------------

define_fixed_value_length_serializable_ttlv_type!(
    /// A type for (de)serializing a TTLV Long Integer.
    ///
    /// According to the [KMIP specification 1.0 section 9.1.1.4 Item Value](http://docs.oasis-open.org/kmip/spec/v1.0/os/kmip-spec-1.0-os.html#_Ref262577330):
    /// > _Long Integers are encoded as eight-byte long (64 bit) binary signed numbers in 2's complement
    /// > notation, transmitted big-endian._
    TtlvLongInteger,
    TtlvType::LongInteger,
    i64,
    8
);

// --- TtlvBigInteger -------------------------------------------------------------------------------------------------

/// A type for (de)serializing a TTLV Big Integer.
///
/// According to the [KMIP specification 1.0 section 9.1.1.4 Item Value](http://docs.oasis-open.org/kmip/spec/v1.0/os/kmip-spec-1.0-os.html#_Ref262577330):
/// > _Big Integers are encoded as a sequence of eight-bit bytes, in two's complement notation,
/// > transmitted big-endian. If the length of the sequence is not a multiple of eight bytes, then Big
/// > Integers SHALL be padded with the minimal number of leading sign-extended bytes to make the
/// > length a multiple of eight bytes. These padding bytes are part of the Item Value and SHALL be
/// > counted in the Item Length._
#[derive(Clone, Debug)]
pub struct TtlvBigInteger(pub Vec<u8>);
impl Deref for TtlvBigInteger {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl SerializableTtlvType for TtlvBigInteger {
    const TTLV_TYPE: TtlvType = TtlvType::BigInteger;

    fn read_value<T: Read>(src: &mut T, value_len: u32) -> Result<Self> {
        let mut dst = vec![0; value_len as usize];
        src.read_exact(&mut dst)?;
        Ok(TtlvBigInteger(dst))
    }

    fn write_length_and_value<T: Write>(&self, dst: &mut T) -> Result<u32> {
        let v = self.0.as_slice();
        let v_len = v.len() as u32;
        let num_pad_bytes = Self::calc_pad_bytes(v_len);
        let v_len = v_len + num_pad_bytes;
        dst.write_all(&v_len.to_be_bytes())?; // Write L_ength
        // Write pad bytes out as leading sign extending bytes, i.e. if the sign is positive then pad with zeros
        // otherwise pad with ones.
        let pad_byte = if v_len > 0 && v[0] & 0b1000_0000 == 0b1000_0000 {
            0b1111_1111
        } else {
            0b0000_0000
        };
        for _ in 1..=num_pad_bytes {
            dst.write_all(&[pad_byte])?;
        }
        dst.write_all(v)?; // Write V_alue
        Ok(v_len)
    }
}

// --- TtlvEnumeration ------------------------------------------------------------------------------------------------

define_fixed_value_length_serializable_ttlv_type!(
    /// A type for (de)serializing a TTLV Enumeration.
    ///
    /// According to the [KMIP specification 1.0 section 9.1.1.4 Item Value](http://docs.oasis-open.org/kmip/spec/v1.0/os/kmip-spec-1.0-os.html#_Ref262577330):
    /// > _Enumerations are encoded as four-byte long (32 bit) binary unsigned numbers transmitted big-
    ///   endian. Extensions, which are permitted, but are not defined in this specification, contain the
    ///   value 8 hex in the first nibble of the first byte._
    TtlvEnumeration,
    TtlvType::Enumeration,
    u32,
    4
);

// --- TtlvBoolean ----------------------------------------------------------------------------------------------------

/// A type for (de)serializing a TTLV Boolean.
///
/// According to the [KMIP specification 1.0 section 9.1.1.4 Item Value](http://docs.oasis-open.org/kmip/spec/v1.0/os/kmip-spec-1.0-os.html#_Ref262577330):
/// > _Booleans are encoded as an eight-byte value that SHALL either contain the hex value
/// > 0000000000000000, indicating the Boolean value False, or the hex value 0000000000000001,
/// > transmitted big-endian, indicating the Boolean value True._
/// > Boolean cannot be implemented using the define_fixed_value_length_serializable_ttlv_type! macro because it has
/// > special value verification rules.
#[derive(Clone, Debug)]
pub struct TtlvBoolean(pub bool);
impl TtlvBoolean {
    const TTLV_FIXED_VALUE_LENGTH: u32 = 8;
}
impl Deref for TtlvBoolean {
    type Target = bool;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl SerializableTtlvType for TtlvBoolean {
    const TTLV_TYPE: TtlvType = TtlvType::Boolean;

    fn read_value<T: Read>(src: &mut T, value_len: u32) -> Result<Self> {
        if value_len != Self::TTLV_FIXED_VALUE_LENGTH {
            Err(Error::InvalidTtlvValueLength {
                expected: Self::TTLV_FIXED_VALUE_LENGTH,
                actual: value_len,
                r#type: Self::TTLV_TYPE,
            })
        } else {
            let mut dst = [0u8; Self::TTLV_FIXED_VALUE_LENGTH as usize];
            src.read_exact(&mut dst)?;
            match u64::from_be_bytes(dst) {
                0 => Ok(TtlvBoolean(false)),
                1 => Ok(TtlvBoolean(true)),
                _ => Err(Error::InvalidTtlvValue(Self::TTLV_TYPE)),
            }
        }
    }

    fn write_length_and_value<T: Write>(&self, dst: &mut T) -> Result<u32> {
        let v = match self.0 {
            true => 1u64,
            false => 0u64,
        };
        dst.write_all(&Self::TTLV_FIXED_VALUE_LENGTH.to_be_bytes())?; // Write L_ength
        dst.write_all(&v.to_be_bytes())?; // Write V_alue
        Ok(Self::TTLV_FIXED_VALUE_LENGTH)
    }
}

// --- TtlvTextString -------------------------------------------------------------------------------------------------

// TextString cannot be implemented using the define_fixed_value_length_serializable_ttlv_type! macro because it has a
// dynamic length._

/// A type for (de)serializing a TTLV Text String.
///
/// According to the [KMIP specification 1.0 section 9.1.1.4 Item Value](http://docs.oasis-open.org/kmip/spec/v1.0/os/kmip-spec-1.0-os.html#_Ref262577330):
/// > _Text Strings are sequences of bytes that encode character values according to the UTF-8
/// > encoding standard. There SHALL NOT be null-termination at the end of such strings._
#[derive(Clone, Debug)]
pub struct TtlvTextString(pub String);
impl Deref for TtlvTextString {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl SerializableTtlvType for TtlvTextString {
    const TTLV_TYPE: TtlvType = TtlvType::TextString;

    fn read_value<T: Read>(src: &mut T, value_len: u32) -> Result<Self> {
        // AnySyncRead the UTF-8 bytes, without knowing if they are valid UTF-8
        let mut dst = vec![0; value_len as usize];
        src.read_exact(&mut dst)?;

        // Use the bytes as-is as the internal buffer for a String, verifying that the bytes are indeed valid
        // UTF-8
        let new_str = String::from_utf8(dst)
            .map_err(|_| Error::InvalidTtlvValue(Self::TTLV_TYPE))?;

        Ok(TtlvTextString(new_str))
    }

    fn write_length_and_value<T: Write>(&self, dst: &mut T) -> Result<u32> {
        let v = self.0.as_bytes();
        let v_len = v.len() as u32;
        dst.write_all(&v_len.to_be_bytes())?; // Write L_ength
        dst.write_all(v)?; // Write V_alue
        Ok(v_len)
    }
}

// --- TtlvByteString -------------------------------------------------------------------------------------------------

// ByteString cannot be implemented using the define_fixed_value_length_serializable_ttlv_type! macro because it has a
// dynamic length.

/// A type for (de)serializing a TTLV Byte String.
///
/// According to the [KMIP specification 1.0 section 9.1.1.4 Item Value](http://docs.oasis-open.org/kmip/spec/v1.0/os/kmip-spec-1.0-os.html#_Ref262577330):
/// > _Byte Strings are sequences of bytes containing individual unspecified eight-bit binary values, and are interpreted
/// > in the same sequence order._
#[derive(Clone, Debug)]
pub struct TtlvByteString(pub Vec<u8>);
impl Deref for TtlvByteString {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl SerializableTtlvType for TtlvByteString {
    const TTLV_TYPE: TtlvType = TtlvType::ByteString;

    fn read_value<T: Read>(src: &mut T, value_len: u32) -> Result<Self> {
        // AnySyncRead the UTF-8 bytes, without knowing if they are valid UTF-8
        let mut dst = vec![0; value_len as usize];
        src.read_exact(&mut dst)?;
        Ok(TtlvByteString(dst))
    }

    fn write_length_and_value<T: Write>(&self, dst: &mut T) -> Result<u32> {
        let v = self.0.as_slice();
        let v_len = v.len() as u32;
        dst.write_all(&v_len.to_be_bytes())?; // Write L_ength
        dst.write_all(v)?; // Write V_alue
        Ok(v_len)
    }
}

// --- TtlvDateTime ---------------------------------------------------------------------------------------------------

define_fixed_value_length_serializable_ttlv_type!(
    /// A type for (de)serializing a TTLV Date-Time.
    ///
    /// According to the [KMIP specification 1.0 section 9.1.1.4 Item Value](http://docs.oasis-open.org/kmip/spec/v1.0/os/kmip-spec-1.0-os.html#_Ref262577330):
    /// > _Date-Time values are POSIX Time values encoded as Long Integers. POSIX Time, as described
    ///   in IEEE Standard 1003.1 [IEEE1003-1], is the number of seconds since the Epoch (1970 Jan 1,
    ///   00:00:00 UTC), not counting leap seconds._
    TtlvDateTime,
    TtlvType::DateTime,
    i64,
    8
);

// --- TtlvInterval ---------------------------------------------------------------------------------------------------

/// A type for (de)serializing a TTLV Interval.
///
/// According to the [KMIP specification 1.0 section 9.1.1.4 Item Value](http://docs.oasis-open.org/kmip/spec/v1.0/os/kmip-spec-1.0-os.html#_Ref262577330):
/// > _Intervals are encoded as four-byte long (32 bit) binary unsigned numbers, transmitted big-endian.
/// > They have a resolution of one second._
#[allow(dead_code)]
pub type TtlvInterval = TtlvEnumeration;

// --- TtlvStateMachine ---------------------------------------------------------------------------------------------

/// A flag used by [TtlvStateMachine] to know which rules to apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TtlvStateMachineMode {
    Deserializing,
    Serializing,
}

/// A state machine for enforcing TTLV field order rules.
pub struct TtlvStateMachine {
    mode: TtlvStateMachineMode,
    expected_next_field_type: FieldType,
    ignore_next_tag: bool,
}

impl TtlvStateMachine {
    pub fn new(mode: TtlvStateMachineMode) -> Self {
        Self {
            mode,
            expected_next_field_type: FieldType::default(),
            ignore_next_tag: false,
        }
    }

    pub fn advance(
        &mut self,
        next_field_type: FieldType,
    ) -> std::result::Result<bool, Error> {
        use TtlvStateMachineMode as Mode;

        let next_expected_next_field_type =
            match (self.mode, self.expected_next_field_type, next_field_type) {
                // First, the normal cases: expect a certain field type to be written next and that is what is indicated
                (_, FieldType::Tag, FieldType::Tag) => FieldType::Type,
                (_, FieldType::Type, FieldType::Type) => FieldType::Length,
                (
                    Mode::Serializing,
                    FieldType::Type,
                    FieldType::TypeAndLengthAndValue,
                ) => FieldType::Tag,
                (_, FieldType::Length, FieldType::Length) => FieldType::Value,
                (
                    Mode::Deserializing,
                    FieldType::Length,
                    FieldType::LengthAndValue,
                ) => FieldType::Tag,
                (_, FieldType::Value, FieldType::Value) => FieldType::Tag,

                // In the leaf case a V always follows TTL, but higher in the TTLV structure hierarchy the first item in
                // a structure can be another TTLV item (i.e. we see a tag being written instead of a value)
                (_, FieldType::Value, FieldType::Tag) => FieldType::Type,

                // Special case: we've been explicitly asked after writing a tag to ignore a subsequent attempt to write
                // another tag. Normally attempting to write TT would be an error, but in this case the second T should be
                // silently ignored. This supports use cases like the KMIP Attribute Value which is of the form XTLV where
                // X is constant tag value and not the normal tag associated with the item being serialized.
                (Mode::Serializing, FieldType::Type, FieldType::Tag)
                    if self.ignore_next_tag =>
                {
                    self.ignore_next_tag = false;
                    FieldType::Type
                }

                // Error, don't permit invalid things like TTVL etc.
                (_, expected, actual) => {
                    return Err(Error::UnexpectedTtlvField {
                        expected,
                        actual,
                    });
                }
            };

        // Advance the state machine if needed
        if self.mode == Mode::Deserializing
            || next_expected_next_field_type != self.expected_next_field_type
        {
            self.expected_next_field_type = next_expected_next_field_type;
            Ok(true)
        } else {
            // It was permitted to stay in the current state. Signalling this allows calling code to know that it should
            // NOT write out the next field, which normally would be an error and we would abort but in this case it is
            // going to be okay as long as the caller respects this return value.
            Ok(false)
        }
    }

    pub fn ignore_next_tag(&mut self) -> std::result::Result<(), Error> {
        if matches!(self.mode, TtlvStateMachineMode::Serializing) {
            self.ignore_next_tag = true;
            Ok(())
        } else {
            Err(Error::InvalidStateMachineOperation)
        }
    }

    pub fn reset(&mut self) {
        self.expected_next_field_type = FieldType::default();
        self.ignore_next_tag = false;
    }
}
