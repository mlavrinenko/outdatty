//! `outdatty` catches outdated artifacts via a declared dependency graph.
//!
//! Declare, in a YAML manifest, which dependent artifacts must be re-confirmed
//! whenever a source artifact changes. `outdatty` records `blake3` hashes in a
//! committed lockfile and fails a `check` when a source changed but its
//! dependents were not re-confirmed. It runs no build commands: it only
//! validates synchronisation, so it works for any file — code, docs, `docx`,
//! configuration.
//!
//! The modules expose the manifest ([`manifest`]) and lockfile ([`lock`])
//! formats, content hashing ([`hashing`]), pattern resolution ([`resolve`]),
//! the evaluation engine ([`engine`]), and report rendering ([`report`]).

pub mod cli;
pub mod commands;
pub mod engine;
pub mod error;
pub mod hashing;
pub mod lock;
pub mod manifest;
pub mod report;
pub mod resolve;

pub use cli::run;
pub use error::{Error, Result};
