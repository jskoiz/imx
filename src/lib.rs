//! Workspace-level integration harness for the standalone Rust image tool.
//!
//! Product code lives in `standalone/crates/*`; this package owns cross-crate
//! tests and benches that compare the product slice with ImageMagick as an
//! external oracle.
