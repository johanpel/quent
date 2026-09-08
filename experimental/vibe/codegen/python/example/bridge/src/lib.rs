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

context = quent_demo.Context()
cluster_observer = context.cluster_observer()
cluster = cluster_observer.create(context.id)
cluster.declaration(instance_name="cluster")
assert cluster.declaration_emitted()
worker = context.worker_observer().create()
worker.declaration(
    instance_name="worker",
    cluster=cluster,
    details=UserDict({
        "version": "1.0",
        "custom": UserDict({"threads": 8}),
    }),
)
queue = context.queue_observer().create()
queue.declaration(instance_name="queue", worker=worker)
thread = context.thread_observer().create()
thread.idle(seq=0, worker=worker)
thread.active(seq=1)
task = context.task_observer().create()
task.queued(
    seq=0,
    instance_name="task",
    index=1,
    worker=worker,
    use_queue=UserDict({
        "target": queue,
        "data": UserDict({"entries": 1}),
    }),
)
task.computing(seq=1, use_thread={"target": thread, "data": {}}, use_memory=None)
task.exit(seq=2)
context.close()
detached_cluster = cluster_observer.create()
detached_cluster.declaration(instance_name="detached")
"#,
                None,
                Some(&locals),
            )
            .unwrap();
        });
    }
}
