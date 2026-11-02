use std::sync::{Arc, Barrier};
use std::thread;

use factstr::{EventStore, NewEvent};
use factstr_postgres::PostgresStore;
use serde_json::{Value, json};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ImageExt, runners::AsyncRunner},
};

use supplier_rfq_rpu_rust::{
    amri::ParticipantId,
    application_state::atomic_interaction::{
        context::{CompositeContextSpecification, ContextClause, ContextRequirement},
        factstr::FactstrAtomicInteractionState,
        state_port::{AtomicInteractionStatePort, NewAtomicFact, StatePortError, StoreIdentity},
    },
};

fn event(event_type: &str, payload: Value) -> NewEvent {
    NewEvent::new(event_type.to_owned(), payload)
}

fn atomic_fact(event_type: &str, payload: Value) -> NewAtomicFact {
    NewAtomicFact::new(event_type.to_owned(), payload).expect("test fact is valid")
}

fn rfq_supplier_specification() -> CompositeContextSpecification {
    CompositeContextSpecification::union(&[
        ContextRequirement::new(
            ParticipantId::new("close-rfq").unwrap(),
            vec![
                ContextClause::new("rfq.opened.v1", json!({"rfq_id":"RFQ-1","tenant":"T1"}))
                    .unwrap(),
            ],
        ),
        ContextRequirement::new(
            ParticipantId::new("award-quote").unwrap(),
            vec![
                ContextClause::new("supplier.registered.v1", json!({"supplier_id":"SUP-1"}))
                    .unwrap(),
            ],
        ),
    ])
    .unwrap()
}

async fn connect(url: String) -> Arc<PostgresStore> {
    Arc::new(
        tokio::task::spawn_blocking(move || PostgresStore::connect(&url))
            .await
            .expect("FACTSTR connect task must complete")
            .expect("FACTSTR PostgreSQL store must connect"),
    )
}

#[tokio::test]
async fn factstr_adapter_proves_union_query_unchanged_token_and_atomic_conditional_batches() {
    let container = Postgres::default()
        .with_db_name("amri_stage3")
        .with_user("amri_stage3")
        .with_password("amri_stage3_password")
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
    let url = format!("postgres://amri_stage3:amri_stage3_password@{host}:{port}/amri_stage3");
    let store_a = connect(url.clone()).await;
    let store_b = connect(url).await;
    let identity = StoreIdentity::new("supplier-rfq-stage3-test-db").unwrap();
    let adapter_a = Arc::new(FactstrAtomicInteractionState::new(
        Arc::clone(&store_a),
        identity.clone(),
    ));
    let adapter_b = Arc::new(FactstrAtomicInteractionState::new(
        Arc::clone(&store_b),
        identity.clone(),
    ));

    // Seed positive and negative controls. EventQuery's filters are OR; event type and
    // its single object-subset payload predicate are AND.
    store_a
        .append(vec![
            event(
                "rfq.opened.v1",
                json!({"rfq_id":"RFQ-1","tenant":"T1","status":"open"}),
            ),
            event("rfq.opened.v1", json!({"rfq_id":"RFQ-1","tenant":"T2"})),
            event("rfq.closed.v1", json!({"rfq_id":"RFQ-1","tenant":"T1"})),
            event(
                "supplier.registered.v1",
                json!({"supplier_id":"SUP-1","name":"A"}),
            ),
            event(
                "supplier.registered.v1",
                json!({"supplier_id":"SUP-2","name":"B"}),
            ),
        ])
        .expect("seed batch must commit atomically");

    let specification = rfq_supplier_specification();
    let initial = adapter_a.read_composite(&specification).unwrap();
    assert_eq!(initial.snapshot().facts().len(), 2);
    assert_eq!(
        initial.snapshot().specification_fingerprint(),
        specification.fingerprint()
    );
    assert_eq!(
        initial.token().specification_fingerprint(),
        specification.fingerprint()
    );
    assert_eq!(initial.snapshot().facts()[0].event_type(), "rfq.opened.v1");
    assert_eq!(
        initial.snapshot().facts()[1].event_type(),
        "supplier.registered.v1"
    );

    // FACTSTR 0.5.2 also treats array predicates as order-insensitive subsets.
    // Prove that an object selector inside an array matches a richer stored item.
    store_a
        .append(vec![
            event(
                "order.created.v1",
                json!({"items":[{"sku":"SKU-A","quantity":2},{"sku":"SKU-B"}]}),
            ),
            event("order.created.v1", json!({"items":[{"sku":"SKU-C"}]})),
        ])
        .expect("array-subset controls must append");
    let array_specification = CompositeContextSpecification::union(&[ContextRequirement::new(
        ParticipantId::new("array-selector").unwrap(),
        vec![ContextClause::new("order.created.v1", json!({"items":[{"sku":"SKU-A"}]})).unwrap()],
    )])
    .unwrap();
    let array_snapshot = adapter_a.read_composite(&array_specification).unwrap();
    assert_eq!(array_snapshot.snapshot().facts().len(), 1);
    assert_eq!(
        array_snapshot.snapshot().facts()[0].payload(),
        &json!({"items":[{"sku":"SKU-A","quantity":2},{"sku":"SKU-B"}]})
    );

    // An unrelated event must not invalidate the original token.
    store_b
        .append(vec![event(
            "supplier.registered.v1",
            json!({"supplier_id":"SUP-OTHER","name":"Other"}),
        )])
        .expect("irrelevant event must append");

    // Both proposed events belong to the protected union. A successful call returns one
    // contiguous receipt for the complete batch.
    let receipt = adapter_a
        .append_if_unchanged(
            initial.token(),
            vec![
                atomic_fact(
                    "rfq.opened.v1",
                    json!({"rfq_id":"RFQ-1","tenant":"T1","stage3_batch":"one"}),
                ),
                atomic_fact(
                    "supplier.registered.v1",
                    json!({"supplier_id":"SUP-1","stage3_batch":"one"}),
                ),
            ],
        )
        .expect("irrelevant change must not create a false conflict");
    assert_eq!(receipt.committed_count(), 2);
    assert_eq!(
        receipt.last_sequence_number() - receipt.first_sequence_number(),
        1
    );

    // A relevant change after the snapshot invalidates the token. The attempted two-fact
    // batch must contribute zero records.
    let stale = adapter_b
        .read_composite(&rfq_supplier_specification())
        .unwrap();
    store_a
        .append(vec![event(
            "rfq.opened.v1",
            json!({"rfq_id":"RFQ-1","tenant":"T1","external_revision":99}),
        )])
        .expect("relevant concurrent change must append");
    let before_failed_batch = adapter_a
        .read_composite(&rfq_supplier_specification())
        .unwrap()
        .snapshot()
        .facts()
        .len();
    let stale_result = adapter_b.append_if_unchanged(
        stale.token(),
        vec![
            atomic_fact(
                "rfq.opened.v1",
                json!({"rfq_id":"RFQ-1","tenant":"T1","stage3_batch":"must-not-commit"}),
            ),
            atomic_fact(
                "supplier.registered.v1",
                json!({"supplier_id":"SUP-1","stage3_batch":"must-not-commit"}),
            ),
        ],
    );
    assert_eq!(stale_result, Err(StatePortError::ContextChanged));
    let after_failed_batch = adapter_a
        .read_composite(&rfq_supplier_specification())
        .unwrap();
    assert_eq!(
        after_failed_batch.snapshot().facts().len(),
        before_failed_batch
    );
    assert!(
        after_failed_batch.snapshot().facts().iter().all(|record| {
            record.payload().get("stage3_batch") != Some(&json!("must-not-commit"))
        })
    );

    // Independent PostgresStore connections that read the same relevant context may
    // race, but the original token permits at most one conditional batch to commit.
    let race_clause =
        ContextClause::new("race.reservation.v1", json!({"resource":"RESOURCE-1"})).unwrap();
    let race_spec = CompositeContextSpecification::union(&[ContextRequirement::new(
        ParticipantId::new("reserve-resource").unwrap(),
        vec![race_clause],
    )])
    .unwrap();
    store_a
        .append(vec![event(
            "race.reservation.v1",
            json!({"resource":"RESOURCE-1","seed":true}),
        )])
        .expect("race seed must append");
    let race_token_a = adapter_a.read_composite(&race_spec).unwrap().into_parts().1;
    let race_token_b = adapter_b.read_composite(&race_spec).unwrap().into_parts().1;
    let barrier = Arc::new(Barrier::new(2));

    let left_adapter = Arc::clone(&adapter_a);
    let left_barrier = Arc::clone(&barrier);
    let left = thread::spawn(move || {
        left_barrier.wait();
        left_adapter.append_if_unchanged(
            &race_token_a,
            vec![atomic_fact(
                "race.reservation.v1",
                json!({"resource":"RESOURCE-1","winner":"left"}),
            )],
        )
    });
    let right_adapter = Arc::clone(&adapter_b);
    let right_barrier = Arc::clone(&barrier);
    let right = thread::spawn(move || {
        right_barrier.wait();
        right_adapter.append_if_unchanged(
            &race_token_b,
            vec![atomic_fact(
                "race.reservation.v1",
                json!({"resource":"RESOURCE-1","winner":"right"}),
            )],
        )
    });

    let outcomes = [
        left.join().expect("left contender must finish"),
        right.join().expect("right contender must finish"),
    ];
    assert_eq!(outcomes.iter().filter(|outcome| outcome.is_ok()).count(), 1);
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| {
                matches!(
                    outcome.as_ref(),
                    Err(error) if error == &StatePortError::ContextChanged
                )
            })
            .count(),
        1
    );

    let race_snapshot = adapter_a.read_composite(&race_spec).unwrap();
    let winners = race_snapshot
        .snapshot()
        .facts()
        .iter()
        .filter(|fact| fact.payload().get("winner").is_some())
        .count();
    assert_eq!(winners, 1);

    drop(adapter_a);
    drop(adapter_b);
    drop(store_a);
    drop(store_b);
    drop(container);
}
