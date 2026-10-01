//! Persisted file compatibility is independent of live model and terminal types.
//! Loading is a bootstrap operation; frame-time saves are delegated to the
//! bounded runtime writer.

pub mod workspace_state;
