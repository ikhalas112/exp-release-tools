//! Phase 2 Integration Tests Module
//!
//! This module contains integration tests for Phase 2 (Full DLL Embedding).
//! Tests verify the complete workflow from DLL analysis to embedding into
//! protected executables.

// Full DLL embedding integration tests
mod full_dll_integration;

// Re-export for convenience
pub use full_dll_integration::*;
