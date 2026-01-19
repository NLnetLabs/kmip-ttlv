//! Dynamic traits for sync or async use depending on the Cargo features used.
//!
//! By default the [AnySyncRead] trait is equivalent to `std::io::Read`.
//!
//! However, if this crate is built with either the `async-with-async-std` or `async-with-tokio` feature enabled then
//! this trait instead becomes `async_std::io::ReadExt` or `tokio::io::AsyncReadExt` respectively.
//!
//! This enables code that is otherwise identical to be re-used.

cfg_if::cfg_if! {
    if #[cfg(feature = "sync")] {
        pub trait AnySyncRead: std::io::Read {}

        impl<T: ?Sized + std::io::Read> AnySyncRead for T {}
    } else if #[cfg(feature = "async-with-tokio")] {
        pub trait AnySyncRead: tokio::io::AsyncReadExt + std::marker::Unpin {}

        impl<T: ?Sized + tokio::io::AsyncReadExt + std::marker::Unpin> AnySyncRead for T {}
    } else if #[cfg(feature = "async-with-async-std")] {
        pub trait AnySyncRead: async_std::io::ReadExt + std::marker::Unpin {}

        impl<T: ?Sized + async_std::io::ReadExt + std::marker::Unpin> AnySyncRead for T {}
    }
}
