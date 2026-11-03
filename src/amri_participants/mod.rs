//! Supply Chain adapters for the generic AMRI Core.
//!
//! This module is deliberately separate from `crate::amri`: the AMRI Core remains
//! domain- and store-independent, while these adapters reuse Supply Chain decisions.

pub mod rfq_sourcing;
