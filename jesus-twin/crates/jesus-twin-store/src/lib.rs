//! Storage and retrieval for the twin, behind a [`Store`] trait.
//!
//! The trait boundary is load-bearing: start embedded (SurrealDB 3.1, single binary,
//! edge) and move to a remote node later without changing the agent core — "embedded" and
//! "horizontally scalable" are different deployments (ARCHITECTURE.md §3, §9).
//!
//! One store does vector + graph + BM25 + RRF in a single query (the reason SurrealDB was
//! chosen over pgvector for this project — README §2, ARCHITECTURE.md §7).

pub mod embed;
pub mod ingest;
pub mod mindmap;
pub mod retrieve;
pub mod schema;
pub mod stopwords;
pub mod store;
pub mod surreal;

pub use embed::Embed;
pub use retrieve::{Passage, RagRecord, RetrievalSet};
pub use store::{Store, StoreError};
pub use surreal::SurrealStore;
