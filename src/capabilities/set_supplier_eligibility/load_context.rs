use uuid::Uuid;

use crate::application_state::{
    ApplicationFact, ApplicationStateProvider, StateQuery, StateReadError,
};

use super::{build_context::EligibilityContextFact, process_request::SetSupplierEligibilityError};

pub(super) struct LoadedEligibilityContext {
    pub(super) facts: Vec<EligibilityContextFact>,
    pub(super) current_context_version: crate::application_state::ContextVersion,
}

/// A atribuição depende da existência do cadastro; o histórico de elegibilidade fica fora do CCC.
pub(super) fn eligibility_context_query(supplier_id: Uuid) -> StateQuery {
    StateQuery::SupplierRegistration { supplier_id }
}

pub(super) fn load_context(
    state: &dyn ApplicationStateProvider,
    query: &StateQuery,
) -> Result<LoadedEligibilityContext, SetSupplierEligibilityError> {
    let snapshot = state
        .read(query)
        .map_err(|error: StateReadError| SetSupplierEligibilityError::Store(error.to_string()))?;
    let facts = snapshot
        .facts
        .into_iter()
        .filter_map(|record| match record.fact {
            ApplicationFact::SupplierRegistered(fact) => {
                Some(EligibilityContextFact::Registered(fact.supplier_id))
            }
            _ => None,
        })
        .collect();
    Ok(LoadedEligibilityContext {
        facts,
        current_context_version: snapshot.version,
    })
}

#[cfg(test)]
mod context_scope_tests {
    use super::eligibility_context_query;
    use crate::application_state::{
        ApplicationStateProvider, factstr::FactstrApplicationStateProvider,
    };
    use crate::events::{
        SUPPLIER_ELIGIBILITY_CHANGED_V1, SUPPLIER_REGISTERED_V1, SupplierEligibilityChangedFact,
        SupplierStatus,
    };
    use factstr::{EventStore, NewEvent};
    use factstr_postgres::PostgresStore;
    use std::sync::Arc;
    use testcontainers_modules::{
        postgres::Postgres,
        testcontainers::{ImageExt, runners::AsyncRunner},
    };
    use uuid::Uuid;

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
        let provider = Arc::new(FactstrApplicationStateProvider::new(Arc::clone(&store)));
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
                    .expect("registration fact must append")
            }
        })
        .await
        .expect("registration append task must finish");

        let query = eligibility_context_query(supplier_id);
        let query_for_read = query.clone();
        let before = tokio::task::spawn_blocking({
            let provider = Arc::clone(&provider);
            move || {
                provider
                    .read(&query_for_read)
                    .expect("context must be queryable")
                    .version
            }
        })
        .await
        .expect("context read task must finish");

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
                    .expect("eligibility fact must append")
            }
        })
        .await
        .expect("eligibility append task must finish");

        let after = tokio::task::spawn_blocking({
            let provider = Arc::clone(&provider);
            move || {
                provider
                    .read(&query)
                    .expect("context must remain queryable")
                    .version
            }
        })
        .await
        .expect("second context read task must finish");
        assert!(
            before.is_present(),
            "registration must establish a context version"
        );
        assert_eq!(
            before, after,
            "eligibility history is outside this assignment command's context"
        );
        drop(provider);
        drop(store);
        drop(container);
    }
}
