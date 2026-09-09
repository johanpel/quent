use quent_instrumentation_build_example::demo::{Connection, Context, Demo, Noop, Query};

fn main() {
    let context = Context::<Demo>::try_new(Noop).unwrap();
    let connection = context.observer::<Connection>().handle();
    let query = context
        .observer::<Query>()
        .handle()
        .submitted("select 1".to_string(), connection.as_entity_ref())
        .running(10)
        .ready(true);
    let _query = query.running(20);
}
