//! Command RPU: RegisterSupplier.
//! Registra identidade de fornecedor com unicidade protegida por CCC.

mod append_consequences;
mod build_context;
mod generate_consequences;
mod load_context;
mod process_request;

pub(crate) use process_request::process_with_providers;
pub use process_request::{RegisterSupplierError, RegisterSupplierRequest};
