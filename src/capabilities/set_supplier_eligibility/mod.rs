//! Command RPU: SetSupplierEligibility.
//! A elegibilidade é registrada como fato separado do cadastro do fornecedor.

mod append_consequences;
mod build_context;
mod generate_consequences;
mod load_context;
mod process_request;

pub(crate) use process_request::process_with_providers;
pub use process_request::{SetSupplierEligibilityError, SetSupplierEligibilityRequest};
