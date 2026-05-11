// Host-side property-based test crate for TreadLink.
// Re-exports the protocol and converter modules for testing without no_std/defmt constraints.

pub mod central;
pub mod converter;
pub mod peripheral;
pub mod protocol;
