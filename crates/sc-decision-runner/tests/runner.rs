//! sc-decision-runner acceptance tests: in-process, scripted and blocking transports,
//! `MemoryLog`, no sleep, no network. Each test names the acceptance line it proves.
//!
//! One test crate, so the shared support is used whole. Clippy treats only `#[test]`
//! functions and `#[cfg(test)]` items as test code, so both modules are `#[cfg(test)]`.

#[cfg(test)]
mod suite;
#[cfg(test)]
mod support;
