//! Load tree-sitter grammars and run any query against them.
//!
//! [`run_query`] runs one caller-written query against one source with any loaded grammar
//! ([`languages`], named by a checked [`LanguageId`]) and returns its captures sorted by (start byte, name, end byte). The crate
//! also ships predefined queries ([`queries`], [`query_source`]), a size-limited file reader
//! ([`read_source`]) and a JSON encoder ([`to_json_bytes`]).
//!
//! The library is stand-alone: synchronous, no threads, no logging, and no sc-agent
//! dependency. Feature `python` adds the PyO3 support types (module [`python`]) used by the
//! `sc-treesitter-py` extension crate. The `sitr` CLI is the separate **`sitr`** crate.
//!
//! # Examples
//!
//! ```
//! # #[cfg(feature = "grammar-python")] {
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
//! # }
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
pub use error::{Error, Limit};
#[doc(inline)]
pub use grammar::{LanguageId, languages};
#[doc(inline)]
pub use query::{QueryInfo, queries, query_source};
#[doc(inline)]
pub use run::{Capture, MATCH_LIMIT, MAX_CAPTURE_TEXT_BYTES, MAX_CAPTURES, run_query};
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

#[cfg(test)]
mod tests {
    use super::*;

    /// AC: REQ-SAG-0002 errors.
    #[test]
    fn to_json_bytes_is_pretty_with_a_trailing_newline() {
        let bytes = to_json_bytes(&["a", "b"]).expect("encode");
        assert_eq!(bytes, b"[\n  \"a\",\n  \"b\"\n]\n");
    }

    #[test]
    fn encode_failure_is_typed() {
        // serde_json refuses maps whose keys are not strings.
        let map = std::collections::BTreeMap::from([((1, 2), 3)]);
        let error = to_json_bytes(&map).expect_err("non-string key");
        assert_eq!(error.id(), "treesitter.encode");
    }
}
