//! Shared, pure domain decisions used by autonomous RPUs and AMRI participants.
//!
//! These modules own business rules only. They do not access providers, Application State,
//! FACTSTR, PostgreSQL, HTTP, clocks or random-number generators.

pub mod rfq_lifecycle;
pub mod sourcing;
