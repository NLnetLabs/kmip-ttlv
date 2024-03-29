//! A crate to (de)serialize Rust data types from/to bytes in the KMIP TTLV format.
//!
//! This is the detailed API documentation. For a higher level introduction see the [README].
//!
//! [README]: https://crates.io/crates/kmip-ttlv/
//!
//! Note that this crate only supports (de)serialization of primitive TTLV types, it does **NOT** send or receive data.
//! See the [kmip-protocol](https://crates.io/crates/kmip-protocol/) crate for support for (de)serializing KMIP
//! specification defined objects composed from TTLV primitives and for an example TLS client.
//!
//! # Usage, features and APIs
//!
//! Add the following to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! kmip-ttlv = "0.3.5"
//! serde = "1.0.126"
//! serde_derive = "1.0.126"
//! ```
//!
//! ## High level API
//!
//! Assuming that you have already defined your Rust types with the required attributes (more on this below) you can
//! serialize and deserialize them using the high level Serde Derive based API as follows:
//!
//! ```ignore
//! use kmip_ttlv::{Config, from_slice, to_vec};
//!
//! // Serialize some struct variable (whose type is correctly
//! // attributed) to bytes in TTLV format:
//! let bytes = to_vec(&my_struct)?;
//!
//! // Deserialize the byte vec back to a struct:
//! let my_other_struct: MyStruct = from_slice(&mut bytes)?;
//! ```
//!
//! ## Low level API
//!
//! There is also a low-level API which is much more labourious to use. The high-level API should be
//! sufficient unless you wish to avoid depending on the Serde crates. You can disable the dependence on Serde by
//! setting `default-features = false` in `Cargo.toml`, e.g.:
//!
//! ```toml
//! [dependencies]
//! kmip-ttlv = { version = "0.3.1", default-features = false }
//! ```
//!
//! To learn more about the low-level API see the [types] module.
//!
//! ## Async API
//!
//! This crate also supports _deserialization_ from an async reader via the feature flags `async-with-async-std` and
//! `async-with-tokio`. Only one of these flags can be specified at once and neither can be mixed with the default
//! 'sync' feature flag. The example below also enables the high level API which is disabled otherwise when you
//! use `default-features = false`.
//!
//! ```toml
//! [dependencies.kmip-ttlv]
//! version = "0.3.1"
//! default-features = false
//! features = ["async-with-async-std", "high-level"]
//! ```
//!
//! Without an async feature enabled you can only pass something that implements the `Read` trait to [de::from_reader].
//!
//! With an async feature enabled you can pass something that implements `async_std::io::ReadExt` or
//! `tokio::io::AsyncReadExt`. You'll also need to then suffix the call to [de::from_reader] with `.await` and call
//! it from an `async` function or block.
//!
//! # TTLV format
//!
//! TTLV stands for Tag-Type-Length-Value which represents the format of each node in a tree when serialized to bytes:
//!
//!   - The TTLV format is defined as part of the [Oasis Key Management Interoperability Protocol Specification Version
//!     1.0] (aka KMIP) in [Section 9.1 TTLV Encoding].
//!   - The byte representation of a TTLV item consists of a 3 byte tag, a 1 byte type, a 4 byte length followed by zero
//!     or more "Value" bytes.
//!   - Leaf nodes in the tree are TTLV items whose "Type" denotes them to be a primitive value of some kind (e.g.
//!     Integer, Boolean, etc) and whose "Value" is a single primitive value in serialized form, followed by any
//!     required padding bytes.
//!   - All other tree nodes are "Structure" TTLV items whose value consists of zero or more TTLV items.
//!  
//! Think of a TTLV "Structure" item as a Rust struct and all other TTLV items as fields within that struct but, unlike
//! Rust data types which have a string name, TTLV items are identified by a numeric "Tag".
//!
//! [Oasis Key Management Interoperability Protocol Specification Version 1.0]: https://docs.oasis-open.org/kmip/spec/v1.0/os/kmip-spec-1.0-os.html
//! [Section 9.1 TTLV Encoding]: https://docs.oasis-open.org/kmip/spec/v1.0/os/kmip-spec-1.0-os.html#_Toc262581260
//!
//! # Mapping names to tags
//!
//! Rust identifies structs and struct fields by name but TTLV identifies items by numeric "Tag". We must therefore
//! provide a way to map from name to tag and vice versa. As this crate is Serde (Derive) based we can take advantage of
//! the [Serde Derive atribute] `#[serde(rename = "...")]` to handle this for us:
//!
//! [Serde Derive attribute]: https://serde.rs/attributes.html
//!
//! ```ignore
//! use serde_derive::Serialize;
//!
//! #[derive(Serialize)]
//! #[serde(rename = "0x123456")]
//! struct MyTtlv { }
//!
//! println!("{:0X?}", kmip_ttlv::to_vec(&MyTtlv {}));
//!
//! // prints:
//! // Ok([12, 34, 56, 1, 0, 0, 0, 0])
//! ```
//!
//! You can see the TTLV byte format here: a 3 byte "tag", a 1 byte "type" (type code 1 means a TTLV Structure) and
//! a 4 byte "length". There is no "value" part in this case because the struct doesn't have any fields so the value
//! length is zero.
//!
//! > **NOTE:** If we omit the `#[serde(rename = "...")]` attribute this code will print an error.
//!
//! # Choosing tag values
//!
//! When implementing one of the KMIP specifications the tag value to use for each KMIP object is defined by the spec.
//! The KMIP specifications reserve tag value range 0x420000 - 0x42FFFF for official KMIP tags and reserve tag value
//! range 0x540000 - 0x54FFFF for custom extensions. If using TTLV as a serialization format for your own data you are
//! free to choose your own tag values anywhere in the range 0x000000 - 0xFFFFFF.
//!
//! # Supported data types
//!
//! The following gives a rough indication of the mapping of TTLV types to Rust types by this crate and vice versa:
//!
//! | TTLV data type      | Serializes from     | Deserializes to     |
//! |---------------------|---------------------|---------------------|
//! | Structure (0x01)    | `SomeStruct { .. }`, `SomeStruct( .. )`, tuple variant | `SomeStruct { .. }` |
//! | Integer (0x02)      | `i8`, `i16`, `i32`  | `i32`               |
//! | Long Integer (0x03) | `i64`               | `i64`               |
//! | Big Integer (0x04)  | **UNSUPPORTED**     | `Vec<u8>`           |
//! | Enumeration (0x05)  | `u32`               | See above           |
//! | Boolean (0x06)      | `bool`              | `bool`              |
//! | Text String (0x07)  | `str``              | `String`            |
//! | Byte String (0x08)  | `&[u8]`             | `Vec<u8>`           |
//! | Date Time (0x09)    | `u64`               | `i64`               |
//! | Interval (0x0A)     | **UNSUPPORTED**     | **UNSUPPORTED**     |
//!
//! # Unsupported data types
//!
//! Not all Rust and TTLV data types are supported by this crate, either because there is no obvious mapping from one to
//! the other or because support for it wasn't needed yet:
//!
//! - The following Rust types **CANNOT** be _serialized_ to TTLV as TTLV has no concept of unsigned
//!   integers, floating point, character or 'missing' values : `u8`, `u16`, `f32`, `f64`, `char`, `()`, `None` _(but
//!   see below for a special note about `None`)_.
//!
//! - The following Rust types **CANNOT** be _deserialized_ from TTLV: `()`, `u8`, `u16`, `u32`, `u64`, `i8`, `i16`,
//!  `f32`, `f64`, `char`, `str`, map, `&[u8]`, `()`. `char`,
//!
//! - The following TTLV types **CANNOT** _yet_ be serialized to TTLV: Big Integer (0x04), Interval (0x0A).
//!
//! - The following TTLV types **CANNOT** _yet_ be deserialized from TTLV: Interval (0x0A).
//!
//! - The following Rust types **CANNOT** be deserialized as this crate is opinionated and prefers to
//!   deserialize only into named fields, not nameless groups of values: unit struct, tuple struct, tuple.
//!
//! # Data types treated specially
//!
//! - The Rust `struct` type by default serializes to a TTLV Structure However sometimes it is useful to be able to use a
//!   newtype struct as a wrapper around a primitive type so that you can associate a TTLV tag value with it. This can be
//!   done by using the `Transparent:` prefix when renaming the type, e.g. `#[serde(rename = "Transparent:0xNNNNNN")]`.
//!
//! - The Rust `Some` type is handled as if it were only the value inside the Option, the `Some` wrapper is ignored.
//!
//! - The Rust `None` type cannot be serialized to TTLV. Instead use `#[serde(skip_serializing_if = "Option::is_none")]`
//!   on the `Option` field to be serialized so that Serde skips it if it has value `None` when serializing. When
//!   deserializing into an `Option` if no value with the specified tag is present in the TTLV bytes the Option will be
//!   set to `None`.
//!
//! - The Rust `Vec` type can be used to (de)serialize sequences of TTLV items. To serialize a `Vec` of bytes to a TTLV
//!   Byte String however you should annotate the field with the Serde derive attribute `#[serde(with = "serde_bytes")]`.
//!
//! - The Rust `enum` type is serialized differently depending on the type of the variant being serialized. For unit
//!   variants a `#[serde(rename = "0xNNNNNNNN")]` attribute should be used to cause this crate to serialize the value
//!   as a TTLV Enumeration. A tuple or struct variant will be serialized to a TTLV Structure.
//!
//! - In order to _deserialize_ into a Rust `enum` you must guide this crate to the correct variant to deserialize into.
//!   To support the KMIP specifications this crate supports choosing the variant based on the value of a TTLV item that
//!   was encountered earlier in the deserialization process. To handle this case each candidate `enum` variant must be
//!   specially renamed with Serde derive using one of several supported special matcher syntaxes:
//!
//!   - `#[serde(rename = "if 0xNNNNNN==0xMMMMMMMM")]` syntax will cause this crate to look for a previously encountered
//!     TTLV Enumeration with tag value 0xNNNNNN and to select this `enum` variant if that Enumeration had value
//!     0xMMMMMMMM.
//!   - `#[serde(rename = "if 0xNNNNNN in [0xAAAAAAAA, 0xBBBBBBBB, ..]")]` is like the previous syntax but can match
//!     against more than one possible value.
//!   - `#[serde(rename = "if 0xNNNNNN >= 0xMMMMMMMM")]` can be used to select the variant if a previously seen value
//!     for the specified tag was at least the given value.
//!   - `#[serde(rename = "if 0xNNNNNN==Textual Content")]` syntax will cause this crate to look for a previously
//!     encountered TTLV Text String with tag value 0xNNNNNN and to select this `enum` variant if that Text String had
//!     value `Textual Content`.
//!   - `#[serde(rename = "if type==XXX")]` syntax (where `XXX` is a camel case TTLV type name without spaces such as
//!     `LongInteger`) will cause this crate to select the enum variant if the TTLV type encountered while deserializing
//!     has the specified type.
//!
//! - TTLV Big Integer values can be deserialized to a `Vec<u8>` in their raw byte format. Using a crate like
//!   `num_bigint` you can work with these byte sequences as if they were normal Rust integers. For example, To convert
//!   from a `Vec<u8>` obtained from a TTLV Big Integer to a `num_bigint::BigInt` use the
//!   `num_bigint::BigInt::from_signed_bytes_be` function.
//!
//! # Examples
//!
//! For detailed examples of how to annotate your data types with Serde derive attributes for use with this crate look
//! at the [tests in the source repository for this crate](https://github.com/NLnetLabs/kmip-ttlv/tree/main/src/tests/).
//!
//! For much richer examples see the code and tests in the source repository for the
//! [kmip-protocol](https://crates.io/crates/kmip-protocol/) crate.
//!
//! The `examples/` folder contains a simple `hex_to_txt` tool which can pretty print a human readable tree structure
//! form of the given hexadecimal encoded TTLV bytes. You can run the example with the command:
//!
//! ```bash
//! cargo run --example hex_to_txt </path/to/hex_string_input_file>
//! ```
//!
//! The tool will ignore any line breaks, spaces, double quotes and commas that are present in the file. Try it out by
//! copying the quoted hex input and output strings in the [tests for this crate](https://github.com/NLnetLabs/kmip-ttlv/tree/main/src/tests/)
//! to a file and passing that file to the `hex_to_txt` tool.
//!
//! # Error handling
//!
//! By default Serde ignores any items present in the TTLV byte stream that do not correspond to a tagged field in the
//! Rust struct being deserialized into. You can disable this behaviour and make the presence of unexpected TTLV items
//! into a deserialization error by using the `#[serde(deny_unknown_fields)]` container level Serde derive attribute.
//! You can also explicitly ignore an unsupported item by using the `#[serde(skip_deserializing)]` field level
//! attribute.
//!
//! This crate does not try to be clone free or to support `no_std` scenarios. Memory is allocated to serialize and
//! deserialize into. In particular when deserializing bytes received from an untrusted source with `from_reader()` this
//! could cause allocation of a large amount of memory at which point Rust will panic if the allocation fails. When
//! deserializing with `from_reader()` you are strongly advised to use a `Config` object that specifies a maximum byte
//! length to deserialize to prevent such abuse.
//!
//! If serialization or deserialization fails this crate tries to return sufficient contextual information to aid
//! diagnosing where the problem in the data is and why.
//!
//! For logging or storing of requests and responses for later diagnostic purposes use the
//! [PrettyPrinter::to_diag_string()] function to render TTLV bytes in a compact textual representation with most
//! values redacted (only enumeration values are included in the generated string).
#[cfg(all(
    feature = "sync",
    any(feature = "async-with-async-std", feature = "async-with-tokio")
))]
compile_error!("feature \"sync\" cannot be enabled at the same time as either of the \"async-with-async-std\" or \"async-with-tokio\" features");

#[cfg(all(feature = "async-std", not(feature = "async-with-async-std")))]
compile_error!("do not enable the \"async-std\" feature directly, instead enable the \"async-with-async-std\" feature");

#[cfg(all(feature = "tokio", not(feature = "async-with-tokio")))]
compile_error!("do not enable the \"tokio\" feature directly, instead enable the \"async-with-tokio\" feature");

#[cfg(feature = "high-level")]
#[macro_use]
mod macros;

#[cfg(feature = "high-level")]
pub mod de;
#[cfg(feature = "high-level")]
pub mod error;
#[cfg(feature = "high-level")]
pub mod ser;
#[cfg(feature = "high-level")]
pub mod traits;
pub mod types;
#[cfg(feature = "high-level")]
pub mod util;

#[cfg(feature = "high-level")]
#[doc(inline)]
pub use de::{from_reader, from_slice, Config};

#[cfg(feature = "high-level")]
#[doc(inline)]
pub use ser::{to_vec, to_writer};

#[cfg(feature = "high-level")]
#[doc(inline)]
pub use util::PrettyPrinter;

#[cfg(test)]
mod tests;
