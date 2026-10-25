//! Command RPU: RegisterSupplier.
//! Registra identidade de fornecedor com unicidade protegida por CCC.

mod append_consequences;
mod build_context;
mod generate_consequences;
mod load_context;
mod process_request;

pub use process_request::{RegisterSupplierError, RegisterSupplierRequest, process};
