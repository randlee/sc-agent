//! Load tree-sitter grammars and run any query against them.
//!
//! [`run_query`] runs one caller-written query against one source with any loaded grammar
//! ([`languages`], named by a checked [`LanguageId`]) and returns its captures sorted by (start
//! byte, name, end byte). The crate also ships predefined queries ([`queries`], [`query_source`]),
//! a file reader ([`read_source`]) and a JSON encoder ([`to_json_bytes`]).
//!
//! The library is stand-alone: synchronous, no threads, no logging, and no sc-agent
//! dependency. Feature `python` adds the PyO3 support types (module [`python`]) used by the
//! `sc-treesitter-py` extension crate. The `sitr` CLI is the separate **`sitr`** crate.
//!
//! # Examples
//!
//! ```
//! let python = sc_treesitter::LanguageId::new("python")?;
//! let captures = sc_treesitter::run_query(
//!     python,
//!     "(identifier) @name",
//!     b"answer = 42\n",
//!     None,
//!     None,
//! )?;
//! let json = sc_treesitter::to_json_bytes(&captures)?;
//! assert!(json.ends_with(b"\n"));
//! # Ok::<(), sc_treesitter::Error>(())
//! ```
#![warn(missing_docs)]
// PyO3's macro expansions carry `#[allow(unsafe_code)]`, which `forbid` would reject.
#![cfg_attr(not(feature = "python"), forbid(unsafe_code))]

mod error;
mod grammar;
#[cfg(feature = "python")]
pub mod python;
mod query;
mod run;
mod source;

#[doc(inline)]
pub use error::Error;
#[doc(inline)]
pub use grammar::{LanguageId, languages};
#[doc(inline)]
pub use query::{QueryInfo, queries, query_source};
#[doc(inline)]
pub use run::{Capture, run_query};
#[doc(inline)]
pub use source::{MAX_SOURCE_BYTES, read_source};

/// Serializes `value` as pretty JSON plus a trailing newline.
///
/// # Errors
///
/// [`Error::Encode`] when `serde_json` cannot encode `value`.
pub fn to_json_bytes<T: serde::Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, Error> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|error| Error::Encode {
        detail: error.to_string(),
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}
