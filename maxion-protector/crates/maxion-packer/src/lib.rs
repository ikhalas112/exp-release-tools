//! Maxion Packer Library
//!
//! This library provides file protection and compression strategies for packing
//! game assets into virtual archives.

pub mod protection;

// Re-exports for convenience
pub use protection::{create_protection_config, FileProtectionConfig, ProtectionStrategy};
