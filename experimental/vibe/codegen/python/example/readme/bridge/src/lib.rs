// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

include!(concat!(env!("OUT_DIR"), "/pyo3_bridge.rs"));

#[cfg(test)]
mod tests {
    use pyo3::prelude::*;
    use pyo3::types::{PyDict, PyModule};

    #[test]
    fn initializes_module_and_accepts_general_mappings() {
        Python::attach(|py| {
            let module = PyModule::new(py, "quent_demo").unwrap();
            super::__quent_pyo3_bridge::quent_demo(&module).unwrap();
            let locals = PyDict::new(py);
            locals.set_item("quent_demo", module).unwrap();
            py.run(
                cr#"
from collections import UserDict
import uuid

context = quent_demo.Context()
assert isinstance(context.id, uuid.UUID)
assert not context.closed
assert not hasattr(quent_demo, "Uuid")
assert not hasattr(quent_demo.ExporterOptions, "none")
cluster_observer = context.cluster_observer()
cluster_id = uuid.uuid4()
cluster = cluster_observer.handle(cluster_id)
assert cluster.uuid == cluster_id
cluster.declaration(instance_name="cluster")
assert cluster.declaration_emitted()
try:
    cluster.declaration(instance_name="duplicate")
except quent_demo.EventAlreadyEmittedError:
    pass
else:
    raise AssertionError("once-cardinality event was accepted twice")
worker = context.worker_observer().handle()
worker.declaration(
    instance_name="worker",
    cluster=cluster,
    details=UserDict({
        "version": "1.0",
        "custom": UserDict({
            "null": None,
            "bool": True,
            "u8": quent_demo.DynamicValue.u8(8),
            "u16": quent_demo.DynamicValue.u16(16),
            "u32": quent_demo.DynamicValue.u32(32),
            "u64": quent_demo.DynamicValue.u64(64),
            "i8": quent_demo.DynamicValue.i8(-8),
            "i16": quent_demo.DynamicValue.i16(-16),
            "i32": quent_demo.DynamicValue.i32(-32),
            "i64": quent_demo.DynamicValue.i64(-64),
            "f32": quent_demo.DynamicValue.f32(32.5),
            "f64": quent_demo.DynamicValue.f64(64.5),
            "string": quent_demo.DynamicValue.string("value"),
            "structure": quent_demo.DynamicValue.structure({
                "first": "alpha",
                "second": 2,
            }),
            "u8_list": quent_demo.DynamicValue.u8_list([1, 2]),
            "u16_list": quent_demo.DynamicValue.u16_list([1, 2]),
            "u32_list": quent_demo.DynamicValue.u32_list([1, 2]),
            "u64_list": quent_demo.DynamicValue.u64_list([1, 2]),
            "i8_list": quent_demo.DynamicValue.i8_list([-1, 2]),
            "i16_list": quent_demo.DynamicValue.i16_list([-1, 2]),
            "i32_list": quent_demo.DynamicValue.i32_list([-1, 2]),
            "i64_list": quent_demo.DynamicValue.i64_list([-1, 2]),
            "f32_list": quent_demo.DynamicValue.f32_list([1.5, 2.5]),
            "f64_list": quent_demo.DynamicValue.f64_list([1.5, 2.5]),
            "string_list": quent_demo.DynamicValue.string_list(["first", "second"]),
            "struct_list": quent_demo.DynamicValue.struct_list([
                {"name": "first"},
                {"name": "second"},
            ]),
        }),
    }),
)
queue = context.queue_observer().handle()
queue.declaration(instance_name="queue", worker=worker)
thread = context.thread_observer().handle()
try:
    thread.active()
except AttributeError:
    pass
else:
    raise AssertionError("invalid FSM transition was accepted")
idle_thread = thread.idle(worker=worker)
active_thread = idle_thread.active()
try:
    idle_thread.active()
except quent_demo.HandleConsumedError:
    pass
else:
    raise AssertionError("consumed FSM handle was accepted")
task = context.task_observer().handle()
queued_task = task.queued(
    instance_name="task",
    index=1,
    worker=worker,
    use_queue=UserDict({
        "target": queue,
        "data": UserDict({"entries": 1}),
    }),
)
computing_task = queued_task.computing(
    use_thread={"target": active_thread, "data": {}},
    use_memory=None,
)
exited_task = computing_task.exit()
idle_thread = active_thread.idle(worker=worker)
exited_thread = idle_thread.exit()
context.close()
assert context.closed
try:
    context.worker_observer()
except quent_demo.ContextClosedError:
    pass
else:
    raise AssertionError("closed context created an observer")
detached_cluster = cluster_observer.handle()
detached_cluster.declaration(instance_name="detached")
"#,
                Some(&locals),
                None,
            )
            .unwrap();
        });
    }

    #[test]
    fn preserves_dynamic_attribute_insertion_order() {
        let output_dir = std::env::temp_dir().join(format!(
            "quent-python-dynamic-order-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        std::fs::create_dir(&output_dir).unwrap();
        Python::attach(|py| {
            let module = PyModule::new(py, "quent_demo").unwrap();
            super::__quent_pyo3_bridge::quent_demo(&module).unwrap();
            let locals = PyDict::new(py);
            locals.set_item("quent_demo", module).unwrap();
            locals
                .set_item("output_dir", output_dir.to_str().unwrap())
                .unwrap();
            py.run(
                cr#"
from collections import UserDict

def emit():
    context = quent_demo.Context(quent_demo.ExporterOptions.ndjson(output_dir))
    worker = context.worker_observer().handle()
    worker.declaration(
        instance_name="ordered",
        cluster=context.id,
        details={
            "version": "1.0",
            "custom": UserDict({
                "first": 1,
                "u8": quent_demo.DynamicValue.u8(8),
                "u16": quent_demo.DynamicValue.u16(16),
                "u32": quent_demo.DynamicValue.u32(32),
                "u64": quent_demo.DynamicValue.u64(64),
                "i8": quent_demo.DynamicValue.i8(-8),
                "i16": quent_demo.DynamicValue.i16(-16),
                "i32": quent_demo.DynamicValue.i32(-32),
                "i64": quent_demo.DynamicValue.i64(-64),
                "f32": quent_demo.DynamicValue.f32(32.5),
                "f64": quent_demo.DynamicValue.f64(64.5),
                "string": quent_demo.DynamicValue.string("value"),
                "structure": UserDict({"nested_first": 2, "nested_second": 3}),
                "u8_list": quent_demo.DynamicValue.u8_list([1, 2]),
                "u16_list": quent_demo.DynamicValue.u16_list([1, 2]),
                "u32_list": quent_demo.DynamicValue.u32_list([1, 2]),
                "u64_list": quent_demo.DynamicValue.u64_list([1, 2]),
                "i8_list": quent_demo.DynamicValue.i8_list([-1, 2]),
                "i16_list": quent_demo.DynamicValue.i16_list([-1, 2]),
                "i32_list": quent_demo.DynamicValue.i32_list([-1, 2]),
                "i64_list": quent_demo.DynamicValue.i64_list([-1, 2]),
                "f32_list": quent_demo.DynamicValue.f32_list([1.5, 2.5]),
                "f64_list": quent_demo.DynamicValue.f64_list([1.5, 2.5]),
                "string_list": quent_demo.DynamicValue.string_list(["first", "second"]),
                "struct_list": quent_demo.DynamicValue.struct_list([
                    UserDict({"name": "first"}),
                    UserDict({"name": "second"}),
                ]),
                "nested_list": quent_demo.DynamicValue.list([
                    quent_demo.DynamicValue.u8_list([1, 2]),
                    quent_demo.DynamicValue.list([
                        quent_demo.DynamicValue.string_list(["nested"]),
                    ]),
                ]),
                "last": 4,
            }),
        },
    )
    context.close()

emit()
"#,
                Some(&locals),
                None,
            )
            .unwrap();
        });

        let serialized = read_tree(&output_dir);
        std::fs::remove_dir_all(&output_dir).unwrap();
        let keys = [
            "first",
            "u8",
            "u16",
            "u32",
            "u64",
            "i8",
            "i16",
            "i32",
            "i64",
            "f32",
            "f64",
            "string",
            "structure",
            "nested_first",
            "nested_second",
            "u8_list",
            "u16_list",
            "u32_list",
            "u64_list",
            "i8_list",
            "i16_list",
            "i32_list",
            "i64_list",
            "f32_list",
            "f64_list",
            "string_list",
            "struct_list",
            "nested_list",
            "last",
        ];
        let mut previous = 0;
        for key in keys {
            let position = serialized
                .find(&format!(r#""key":"{key}""#))
                .unwrap_or_else(|| panic!("missing dynamic attribute `{key}`: {serialized}"));
            assert!(
                position >= previous,
                "dynamic attribute `{key}` is out of order"
            );
            previous = position;
        }
        assert!(serialized.contains(
            r#""Struct":[[{"key":"name","value":{"String":"first"}}],[{"key":"name","value":{"String":"second"}}]]"#,
        ));
        for value in [
            r#""U8":8"#,
            r#""U16":16"#,
            r#""U32":32"#,
            r#""U64":64"#,
            r#""I8":-8"#,
            r#""I16":-16"#,
            r#""I32":-32"#,
            r#""I64":-64"#,
            r#""F32":32.5"#,
            r#""F64":64.5"#,
            r#""String":"value""#,
            r#""U8":[1,2]"#,
            r#""U16":[1,2]"#,
            r#""U32":[1,2]"#,
            r#""U64":[1,2]"#,
            r#""I8":[-1,2]"#,
            r#""I16":[-1,2]"#,
            r#""I32":[-1,2]"#,
            r#""I64":[-1,2]"#,
            r#""F32":[1.5,2.5]"#,
            r#""F64":[1.5,2.5]"#,
            r#""String":["first","second"]"#,
            r#""List":[{"U8":[1,2]},{"List":[{"String":["nested"]}]}]"#,
        ] {
            assert!(serialized.contains(value), "missing dynamic value {value}");
        }
    }

    fn read_tree(path: &std::path::Path) -> String {
        let mut output = String::new();
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                output.push_str(&read_tree(&path));
            } else {
                output.push_str(&std::fs::read_to_string(path).unwrap());
            }
        }
        output
    }
}
