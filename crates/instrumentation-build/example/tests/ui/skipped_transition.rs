use quent_instrumentation_build_example::demo::{Context, Demo, Noop, Query};

fn main() {
    let context = Context::<Demo>::try_new(Noop).unwrap();
    let _query = context.observer::<Query>().handle().running(10);
}
