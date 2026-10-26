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
