//! Providers de recursos externos utilizados pela Imperative Shell das RPUs.
//!
//! O Functional Core recebe valores concretos (instantes e IDs), nunca Providers.

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Abstração para a obtenção explícita do instante UTC atual.
pub trait ClockProvider: Send + Sync {
    fn now_utc(&self) -> DateTime<Utc>;
}

/// Abstração para a geração de identificadores de domínio/técnicos.
pub trait IdProvider: Send + Sync {
    fn new_uuid(&self) -> Uuid;
}

/// Provider de produção que consulta o relógio do sistema.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClockProvider;

impl ClockProvider for SystemClockProvider {
    fn now_utc(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// Provider de produção que gera UUIDs v4.
#[derive(Debug, Clone, Copy, Default)]
pub struct UuidV4Provider;

impl IdProvider for UuidV4Provider {
    fn new_uuid(&self) -> Uuid {
        Uuid::new_v4()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_v4_provider_returns_a_non_nil_id() {
        assert!(!UuidV4Provider.new_uuid().is_nil());
    }
}
