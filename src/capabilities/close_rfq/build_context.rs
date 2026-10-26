#[derive(Debug, Clone, Copy)]
pub(super) enum RfqLifecycleFact {
    Opened,
    Closed,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct RfqLifecycleContext {
    pub(super) opened: bool,
    pub(super) closed: bool,
}

/// Reduz fatos tipados a um contexto puro para a decisão de encerramento.
pub(super) fn build_context(facts: &[RfqLifecycleFact]) -> RfqLifecycleContext {
    let mut context = RfqLifecycleContext::default();

    for fact in facts {
        match fact {
            RfqLifecycleFact::Opened => context.opened = true,
            RfqLifecycleFact::Closed => context.closed = true,
        }
    }

    context
}

#[cfg(test)]
mod functional_core_tests {
    use super::*;

    #[test]
    fn lifecycle_context_reduces_open_and_close_facts() {
        let empty = build_context(&[]);
        assert!(!empty.opened && !empty.closed);

        let opened = build_context(&[RfqLifecycleFact::Opened]);
        assert!(opened.opened && !opened.closed);

        let opened_and_closed =
            build_context(&[RfqLifecycleFact::Opened, RfqLifecycleFact::Closed]);
        assert!(opened_and_closed.opened && opened_and_closed.closed);

        let closed_without_open = build_context(&[RfqLifecycleFact::Closed]);
        assert!(!closed_without_open.opened && closed_without_open.closed);
    }
}
