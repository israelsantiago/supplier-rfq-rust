//! Adaptador FACTSTR/PostgreSQL. Este deve ser o único módulo de produção que
//! traduz StateQuery/ApplicationFact para EventQuery/EventRecord/NewEvent.
use crate::application_state::{
    ApplicationFact, ApplicationStateProvider, ContextVersion, StateQuery, StateReadError,
    StateSnapshot, StateWriteError, StoredFact,
};
use crate::events::{
    RFQ_CLOSED_V1, RFQ_OPENED_V1, RfqClosedFact, RfqOpenedFact, SUPPLIER_ELIGIBILITY_CHANGED_V1,
    SUPPLIER_QUOTE_SUBMITTED_V1, SUPPLIER_REGISTERED_V1, SupplierEligibilityChangedFact,
    SupplierQuoteSubmittedFact, SupplierRegisteredFact,
};
use factstr::{EventFilter, EventQuery, EventStore, EventStoreError, NewEvent};
use serde_json::{Value, json, to_value};
use std::ops::Deref;

#[derive(Clone)]
pub struct FactstrApplicationStateProvider<S> {
    store: S,
}
impl<S> FactstrApplicationStateProvider<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }
}

impl<S> ApplicationStateProvider for FactstrApplicationStateProvider<S>
where
    S: Deref + Send + Sync,
    S::Target: EventStore,
{
    fn read(&self, query: &StateQuery) -> Result<StateSnapshot, StateReadError> {
        let result = self
            .store
            .deref()
            .query(&to_event_query(query))
            .map_err(|error| StateReadError::Store(error.to_string()))?;
        let facts = result
            .event_records
            .into_iter()
            .map(|record| {
                let fact = decode_fact(&record.event_type, record.payload)?;
                Ok(StoredFact::new(record.sequence_number, fact))
            })
            .collect::<Result<Vec<_>, StateReadError>>()?;
        Ok(StateSnapshot {
            facts,
            version: ContextVersion::from_factstr(result.current_context_version),
        })
    }

    fn append_if(
        &self,
        context: &StateQuery,
        expected_version: &ContextVersion,
        consequences: Vec<ApplicationFact>,
    ) -> Result<(), StateWriteError> {
        if consequences.is_empty() {
            return Err(StateWriteError::Store(
                "append_if requires at least one consequence".to_owned(),
            ));
        }
        let events = consequences
            .into_iter()
            .map(encode_fact)
            .collect::<Result<Vec<_>, _>>()?;
        self.store
            .deref()
            .append_if(
                events,
                &to_event_query(context),
                expected_version.to_factstr()?,
            )
            .map(|_| ())
            .map_err(map_write_error)
    }
}

fn map_write_error(error: EventStoreError) -> StateWriteError {
    match error {
        EventStoreError::ConditionalAppendConflict { .. } => StateWriteError::ContextChanged,
        other => StateWriteError::Store(other.to_string()),
    }
}

fn to_event_query(query: &StateQuery) -> EventQuery {
    match query {
        StateQuery::RfqIdentity { rfq_id, reference } => {
            EventQuery::all().with_filters([EventFilter::for_event_types([RFQ_OPENED_V1])
                .with_payload_predicates([
                    json!({"rfq_id": rfq_id}),
                    json!({"reference": reference}),
                ])])
        }
        StateQuery::SupplierIdentity {
            supplier_id,
            reference,
        } => {
            EventQuery::all().with_filters([EventFilter::for_event_types([SUPPLIER_REGISTERED_V1])
                .with_payload_predicates([
                    json!({"supplier_id": supplier_id}),
                    json!({"reference": reference}),
                ])])
        }
        StateQuery::SupplierRegistration { supplier_id } => EventQuery::all()
            .with_filters([EventFilter::for_event_types([SUPPLIER_REGISTERED_V1])
                .with_payload_predicates([json!({"supplier_id": supplier_id})])]),
        StateQuery::RfqLifecycle { rfq_id } => EventQuery::all().with_filters([
            EventFilter::for_event_types([RFQ_OPENED_V1])
                .with_payload_predicates([json!({"rfq_id": rfq_id})]),
            EventFilter::for_event_types([RFQ_CLOSED_V1])
                .with_payload_predicates([json!({"rfq_id": rfq_id})]),
        ]),
        StateQuery::SupplierQuoteSubmission {
            rfq_id,
            supplier_id,
        } => EventQuery::all().with_filters([
            EventFilter::for_event_types([RFQ_OPENED_V1])
                .with_payload_predicates([json!({"rfq_id": rfq_id})]),
            EventFilter::for_event_types([RFQ_CLOSED_V1])
                .with_payload_predicates([json!({"rfq_id": rfq_id})]),
            EventFilter::for_event_types([SUPPLIER_REGISTERED_V1])
                .with_payload_predicates([json!({"supplier_id": supplier_id})]),
            EventFilter::for_event_types([SUPPLIER_ELIGIBILITY_CHANGED_V1])
                .with_payload_predicates([json!({"supplier_id": supplier_id})]),
            EventFilter::for_event_types([SUPPLIER_QUOTE_SUBMITTED_V1])
                .with_payload_predicates([json!({"rfq_id": rfq_id, "supplier_id": supplier_id})]),
        ]),
        StateQuery::SupplierQuotes { rfq_id } => EventQuery::all().with_filters([
            EventFilter::for_event_types([RFQ_OPENED_V1])
                .with_payload_predicates([json!({"rfq_id": rfq_id})]),
            EventFilter::for_event_types([SUPPLIER_QUOTE_SUBMITTED_V1])
                .with_payload_predicates([json!({"rfq_id": rfq_id})]),
        ]),
    }
}

fn decode_fact(event_type: &str, payload: Value) -> Result<ApplicationFact, StateReadError> {
    let invalid = |message: String| StateReadError::InvalidFact {
        fact_type: event_type.to_owned(),
        message,
    };
    match event_type {
        RFQ_OPENED_V1 => serde_json::from_value::<RfqOpenedFact>(payload)
            .map(ApplicationFact::RfqOpened)
            .map_err(|e| invalid(e.to_string())),
        RFQ_CLOSED_V1 => serde_json::from_value::<RfqClosedFact>(payload)
            .map(ApplicationFact::RfqClosed)
            .map_err(|e| invalid(e.to_string())),
        SUPPLIER_REGISTERED_V1 => serde_json::from_value::<SupplierRegisteredFact>(payload)
            .map(ApplicationFact::SupplierRegistered)
            .map_err(|e| invalid(e.to_string())),
        SUPPLIER_ELIGIBILITY_CHANGED_V1 => {
            serde_json::from_value::<SupplierEligibilityChangedFact>(payload)
                .map(ApplicationFact::SupplierEligibilityChanged)
                .map_err(|e| invalid(e.to_string()))
        }
        SUPPLIER_QUOTE_SUBMITTED_V1 => {
            serde_json::from_value::<SupplierQuoteSubmittedFact>(payload)
                .map(ApplicationFact::SupplierQuoteSubmitted)
                .map_err(|e| invalid(e.to_string()))
        }
        other => Err(StateReadError::UnsupportedFactType(other.to_owned())),
    }
}

fn encode_fact(fact: ApplicationFact) -> Result<NewEvent, StateWriteError> {
    let encode = |kind: &'static str, payload: Value| NewEvent::new(kind, payload);
    match fact {
        ApplicationFact::RfqOpened(f) => to_value(f).map(|v| encode(RFQ_OPENED_V1, v)),
        ApplicationFact::RfqClosed(f) => to_value(f).map(|v| encode(RFQ_CLOSED_V1, v)),
        ApplicationFact::SupplierRegistered(f) => {
            to_value(f).map(|v| encode(SUPPLIER_REGISTERED_V1, v))
        }
        ApplicationFact::SupplierEligibilityChanged(f) => {
            to_value(f).map(|v| encode(SUPPLIER_ELIGIBILITY_CHANGED_V1, v))
        }
        ApplicationFact::SupplierQuoteSubmitted(f) => {
            to_value(f).map(|v| encode(SUPPLIER_QUOTE_SUBMITTED_V1, v))
        }
    }
    .map_err(|error| StateWriteError::Encoding(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn version_token_preserves_empty_and_present_context() {
        let empty = ContextVersion::from_factstr(None);
        assert_eq!(empty.to_factstr().unwrap(), None);
        let present = ContextVersion::from_factstr(Some(42));
        assert_eq!(present.to_factstr().unwrap(), Some(42));
    }
    #[test]
    fn malformed_known_fact_fails_closed() {
        assert!(matches!(
            decode_fact(RFQ_OPENED_V1, json!({"rfq_id":"bad"})),
            Err(StateReadError::InvalidFact { .. })
        ));
    }
    #[test]
    fn unknown_fact_type_fails_closed() {
        assert!(matches!(
            decode_fact("unexpected.v1", json!({})),
            Err(StateReadError::UnsupportedFactType(_))
        ));
    }

    #[tokio::test]
    async fn provider_reads_typed_facts_and_rejects_stale_conditional_append() {
        use crate::application_state::StateWriteError;
        use chrono::Utc;
        use factstr_postgres::PostgresStore;
        use std::sync::Arc;
        use testcontainers_modules::{
            postgres::Postgres,
            testcontainers::{ImageExt, runners::AsyncRunner},
        };

        let container = Postgres::default()
            .with_db_name("supplier_rfq")
            .with_user("rfq_user")
            .with_password("rfq_password")
            .with_tag("16-alpine")
            .start()
            .await
            .expect("PostgreSQL container must start; Docker must be available");
        let host = container
            .get_host()
            .await
            .expect("container host must exist");
        let port = container
            .get_host_port_ipv4(5432)
            .await
            .expect("PostgreSQL port must be mapped");
        let url = format!("postgres://rfq_user:rfq_password@{host}:{port}/supplier_rfq");
        let url_for_connect = url.clone();
        let store = Arc::new(
            tokio::task::spawn_blocking(move || PostgresStore::connect(&url_for_connect))
                .await
                .expect("store connect task must complete")
                .expect("FACTSTR must initialize PostgreSQL"),
        );
        let provider = FactstrApplicationStateProvider::new(Arc::clone(&store));
        let supplier_id = uuid::Uuid::from_u128(0xabc001);
        let query = StateQuery::SupplierRegistration { supplier_id };
        let before = provider.read(&query).expect("empty context can be read");
        assert!(!before.version.is_present());
        let registered = SupplierRegisteredFact {
            supplier_id,
            reference: "ASP-SUP-1".to_owned(),
            name: "ASP test supplier".to_owned(),
            registered_at: Utc::now(),
        };
        provider
            .append_if(
                &query,
                &before.version,
                vec![ApplicationFact::SupplierRegistered(registered.clone())],
            )
            .expect("conditional append against the empty context must succeed");
        let after = provider
            .read(&query)
            .expect("registration context can be read");
        assert!(after.version.is_present());
        assert_eq!(after.facts.len(), 1);
        assert!(
            matches!(&after.facts[0].fact, ApplicationFact::SupplierRegistered(fact) if fact == &registered)
        );
        let stale = provider.append_if(
            &query,
            &before.version,
            vec![ApplicationFact::SupplierRegistered(
                SupplierRegisteredFact {
                    supplier_id,
                    reference: "SUP-STALE".to_owned(),
                    name: "Stale registration attempt".to_owned(),
                    registered_at: Utc::now(),
                },
            )],
        );
        assert!(matches!(stale, Err(StateWriteError::ContextChanged)));
        drop(provider);
        drop(store);
        drop(container);
    }
}
