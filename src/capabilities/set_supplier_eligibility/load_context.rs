use factstr::{EventFilter, EventQuery, EventStore};
use serde_json::json;
use uuid::Uuid;

use crate::events::{SUPPLIER_REGISTERED_V1, SupplierRegisteredFact};

use super::{build_context::EligibilityContextFact, process_request::SetSupplierEligibilityError};

pub(super) struct LoadedEligibilityContext {
    pub(super) facts: Vec<EligibilityContextFact>,
    pub(super) current_context_version: Option<u64>,
}

/// CCC cobre apenas o fato em que a decisão atual se baseia: o cadastro do fornecedor.
/// O histórico de atribuições de elegibilidade não muda a validade dessa decisão de atribuição.
pub(super) fn eligibility_context_query(supplier_id: Uuid) -> EventQuery {
    EventQuery::all().with_filters([EventFilter::for_event_types([SUPPLIER_REGISTERED_V1])
        .with_payload_predicates([json!({ "supplier_id": supplier_id })])])
}

pub(super) fn load_context(
    store: &impl EventStore,
    query: &EventQuery,
) -> Result<LoadedEligibilityContext, SetSupplierEligibilityError> {
    let result = store
        .query(query)
        .map_err(|error| SetSupplierEligibilityError::Store(error.to_string()))?;
    let mut facts = Vec::with_capacity(result.event_records.len());

    for record in result.event_records {
        if record.event_type != SUPPLIER_REGISTERED_V1 {
            continue;
        }
        let fact: SupplierRegisteredFact =
            serde_json::from_value(record.payload).map_err(|error| {
                SetSupplierEligibilityError::Store(format!(
                    "invalid {SUPPLIER_REGISTERED_V1} fact: {error}"
                ))
            })?;
        facts.push(EligibilityContextFact::Registered(fact.supplier_id));
    }

    Ok(LoadedEligibilityContext {
        facts,
        current_context_version: result.current_context_version,
    })
}

#[cfg(test)]
mod context_scope_tests {
    use super::eligibility_context_query;
    use std::sync::Arc;
    use uuid::Uuid;

    use crate::events::{
        SUPPLIER_ELIGIBILITY_CHANGED_V1, SUPPLIER_REGISTERED_V1, SupplierEligibilityChangedFact,
        SupplierStatus,
    };
    use factstr::{EventStore, NewEvent};
    use factstr_postgres::PostgresStore;
    use testcontainers_modules::{
        postgres::Postgres,
        testcontainers::{ImageExt, runners::AsyncRunner},
    };

    #[tokio::test]
    async fn eligibility_history_does_not_change_assignment_context_version() {
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
        let database_url = format!("postgres://rfq_user:rfq_password@{host}:{port}/supplier_rfq");
        let url_for_store = database_url.clone();
        let store = Arc::new(
            tokio::task::spawn_blocking(move || PostgresStore::connect(&url_for_store))
                .await
                .expect("store connection task must finish")
                .expect("FACTSTR must initialize PostgreSQL schema"),
        );
        let supplier_id = Uuid::from_u128(981001);
        let registered = crate::events::SupplierRegisteredFact {
            supplier_id,
            reference: "SUP-CCC-SCOPE".to_owned(),
            name: "Supplier CCC scope test".to_owned(),
            registered_at: chrono::Utc::now(),
        };
        tokio::task::spawn_blocking({
            let store = Arc::clone(&store);
            move || {
                store
                    .append(vec![NewEvent::new(
                        SUPPLIER_REGISTERED_V1,
                        serde_json::json!(registered),
                    )])
                    .expect("registration fact must append");
            }
        })
        .await
        .expect("registration append task must finish");

        let before = tokio::task::spawn_blocking({
            let store = Arc::clone(&store);
            move || {
                store
                    .query(&eligibility_context_query(supplier_id))
                    .expect("registration context must be queryable")
                    .current_context_version
            }
        })
        .await
        .expect("context query task must finish");

        let changed = SupplierEligibilityChangedFact {
            supplier_id,
            status: SupplierStatus::Blocked,
            changed_at: chrono::Utc::now(),
        };
        tokio::task::spawn_blocking({
            let store = Arc::clone(&store);
            move || {
                store
                    .append(vec![NewEvent::new(
                        SUPPLIER_ELIGIBILITY_CHANGED_V1,
                        serde_json::json!(changed),
                    )])
                    .expect("eligibility fact must append");
            }
        })
        .await
        .expect("eligibility append task must finish");

        let after = tokio::task::spawn_blocking({
            let store = Arc::clone(&store);
            move || {
                store
                    .query(&eligibility_context_query(supplier_id))
                    .expect("registration context must remain queryable")
                    .current_context_version
            }
        })
        .await
        .expect("second context query task must finish");
        assert!(
            before.is_some(),
            "registration must establish a context version"
        );
        assert_eq!(
            before, after,
            "eligibility history is outside this assignment command's context"
        );

        drop(store);
        drop(container);
    }
}
