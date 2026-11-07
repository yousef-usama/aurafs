//! # AuraFS Core Library
//!
//! Shared models, binary serialization, cryptographic hashing, and cloud schema definitions
//! used by both the local daemon (`aurafs-daemon`) and the Kubernetes cloud enrichment service (`aurafs-cloud`).
//!
//! Mirrors Poly's architecture of a unified, robust shared library compiled across multiple deployment targets.

pub mod dynamo_schema;
pub mod hashing;
pub mod models;

pub use dynamo_schema::*;
pub use hashing::*;
pub use models::*;
