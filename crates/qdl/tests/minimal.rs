use quent_qdl::load_str;

const MINIMAL: &str = r#"
// minimal model

model query_engine;

/// A network endpoint.
record Endpoint {
    host: String,
    port: u16,
}

record Plan {
    stage_count: u32,
    stages: Vec<Stage>,
    estimate: Option<CostEstimate>,
}

record Stage {
    name: String,
    operator_ids: Vec<Uuid>,
    cost: f64,
}

record CostEstimate {
    cpu: f64,
    mem_bytes: u64,
}

/// Free-form, runtime-keyed statistics.
record Stats {
    rows: u64,
    extra: Dynamic,
}

entity Cluster {
    once up: {
        region: String,
    },
    once down: {},
}

entity Engine {
    once started: {
        listen_on: Endpoint,
    },
    multi heartbeat: {
        load: f32,
        inflight: u32,
    },
    once stopped: {},
}

entity Query {
    once submitted: {
        text: String,
        plan: Option<Plan>,
    },
    multi progress: {
        stats: Stats,
    },
    once finished: {
        ok: bool,
        error: Option<String>,
    },
}
"#;

#[test]
fn loads_minimal_model() {
    let schema = load_str(MINIMAL).expect("should load");
    assert_eq!(schema.name(), "query_engine");
    assert_eq!(schema.records().count(), 5);
    assert_eq!(schema.entities().count(), 3);

    let engine = schema
        .entity(&"Engine".parse().unwrap())
        .expect("Engine entity");
    assert_eq!(engine.events().count(), 3);

    let endpoint = schema
        .record(&"Endpoint".parse().unwrap())
        .expect("Endpoint record");
    assert_eq!(endpoint.annotations().docs(), Some("A network endpoint."));
}

#[test]
fn rejects_reference_to_unknown_record() {
    let src = "model m; record A { b: Missing, }";
    let err = load_str(src).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("validation"), "unexpected error: {msg}");
}
