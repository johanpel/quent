// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use quent_build_info::ArtifactInfo;
use quent_events::{Event, Model};
use quent_io_types::ImporterProvider;
use serde::de::DeserializeOwned;
use uuid::Uuid;

use super::Format;
use crate::{ContextQuery, EventStore, ImporterOptions, StoreError, StoreResult};

type ImportFn<M> =
    fn(Format, Vec<PathBuf>) -> StoreResult<Box<dyn Iterator<Item = Event<<M as Model>::Event>>>>;

pub struct EventStream<M: Model> {
    entity: &'static str,
    import: ImportFn<M>,
}

impl<M: Model> EventStream<M> {
    pub const fn new(entity: &'static str, import: ImportFn<M>) -> Self {
        Self { entity, import }
    }
}

pub trait FilesystemEventModel: Model {
    fn event_streams() -> &'static [EventStream<Self>];
}

pub fn import_event_files<M, E>(
    format: Format,
    files: Vec<PathBuf>,
) -> StoreResult<Box<dyn Iterator<Item = Event<M::Event>>>>
where
    M: Model,
    E: DeserializeOwned + Into<M::Event> + 'static,
{
    let mut streams = Vec::<Box<dyn Iterator<Item = Event<M::Event>>>>::new();
    for path in files {
        let importer = ImporterOptions::FileSystem(super::importer::Options { format, path })
            .create_importer()?;
        streams.push(Box::new(importer.map(|event: Event<E>| {
            Event::new(event.id, event.timestamp, event.data.into())
        })));
    }
    Ok(Box::new(streams.into_iter().flatten()))
}

pub struct FilesystemEventStore<M> {
    root: PathBuf,
    model: PhantomData<fn() -> M>,
}

impl<M> FilesystemEventStore<M> {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            model: PhantomData,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

impl<M: FilesystemEventModel> EventStore<M> for FilesystemEventStore<M> {
    fn contexts(&self, query: ContextQuery) -> StoreResult<Vec<Uuid>> {
        let expected = M::model_info().name;
        let mut contexts = Vec::new();
        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let Some(id) = entry
                .file_name()
                .to_str()
                .and_then(|name| Uuid::parse_str(name).ok())
            else {
                continue;
            };
            let Ok(artifact) = ArtifactInfo::read_sidecar(&entry.path()) else {
                continue;
            };
            if artifact.model.name != expected {
                continue;
            }
            if let ContextQuery::Emitted { entity } = query
                && !entity_has_data(&entry.path(), entity)?
            {
                continue;
            }
            contexts.push(id);
        }
        contexts.sort_unstable();
        Ok(contexts)
    }

    fn import_events(
        &self,
        context_id: Uuid,
    ) -> StoreResult<Box<dyn Iterator<Item = Event<M::Event>>>> {
        let context = self.root.join(context_id.to_string());
        if !context.is_dir() {
            return Err(StoreError::ContextNotFound(context_id));
        }
        let artifact = ArtifactInfo::read_sidecar(&context)?;
        let expected = M::model_info().name;
        if artifact.model.name != expected {
            return Err(StoreError::ModelMismatch {
                expected,
                actual: artifact.model.name,
            });
        }

        let (format, files) = context_files::<M>(&context)?;
        let mut streams = Vec::new();
        for (descriptor, paths) in M::event_streams().iter().zip(files) {
            if !paths.is_empty() {
                streams.push((descriptor.import)(format, paths)?);
            }
        }
        Ok(Box::new(streams.into_iter().flatten()))
    }
}

fn context_files<M: FilesystemEventModel>(
    context: &Path,
) -> StoreResult<(Format, Vec<Vec<PathBuf>>)> {
    let mut selected: Option<Format> = None;
    let mut has_data = false;
    let mut streams = Vec::with_capacity(M::event_streams().len());
    for descriptor in M::event_streams() {
        let dir = context.join(descriptor.entity);
        let mut files = Vec::new();
        if dir.is_dir() {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                if !entry.file_type()?.is_file() {
                    continue;
                }
                let path = entry.path();
                let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
                    continue;
                };
                let format = Format::try_from(extension)
                    .map_err(|_| StoreError::UnsupportedFormat(extension.to_owned()))?;
                if let Some(first) = selected
                    && first != format
                {
                    return Err(StoreError::MixedFormats {
                        first: first.extension().to_owned(),
                        second: format.extension().to_owned(),
                    });
                }
                selected = Some(format);
                has_data |= path.metadata()?.len() > 0;
                files.push(path);
            }
        }
        files.sort();
        streams.push(files);
    }
    if !has_data {
        return Err(StoreError::EmptyContext);
    }
    Ok((selected.ok_or(StoreError::EmptyContext)?, streams))
}

fn entity_has_data(context: &Path, entity: &str) -> StoreResult<bool> {
    let dir = context.join(entity);
    if !dir.is_dir() {
        return Ok(false);
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry.path().metadata()?.len() > 0
            && entry
                .path()
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|extension| Format::try_from(extension).is_ok())
        {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use quent_build_info::{ArtifactInfo, ModelInfo};
    use quent_events::{Entity, EntityEvent, Event, Model};
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::EventStoreExt;

    struct TestModel;

    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    struct AlphaEvent(u8);

    impl EntityEvent for AlphaEvent {
        const NAME: &'static str = "Alpha";
    }

    struct Alpha;

    impl Entity for Alpha {
        type Event = AlphaEvent;
    }

    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    struct BetaEvent(u8);

    impl EntityEvent for BetaEvent {
        const NAME: &'static str = "Beta";
    }

    #[derive(Debug, PartialEq)]
    enum TestEvent {
        Alpha(AlphaEvent),
        Beta(BetaEvent),
    }

    impl From<AlphaEvent> for TestEvent {
        fn from(event: AlphaEvent) -> Self {
            Self::Alpha(event)
        }
    }

    impl From<BetaEvent> for TestEvent {
        fn from(event: BetaEvent) -> Self {
            Self::Beta(event)
        }
    }

    impl Model for TestModel {
        type Event = TestEvent;

        fn model_info() -> ModelInfo {
            ModelInfo {
                name: "Test".to_owned(),
                ..ModelInfo::unknown()
            }
        }
    }

    impl FilesystemEventModel for TestModel {
        fn event_streams() -> &'static [EventStream<Self>] {
            static STREAMS: &[EventStream<TestModel>] = &[
                EventStream::new(
                    AlphaEvent::NAME,
                    import_event_files::<TestModel, AlphaEvent>,
                ),
                EventStream::new(BetaEvent::NAME, import_event_files::<TestModel, BetaEvent>),
            ];
            STREAMS
        }
    }

    fn context(root: &Path, id: Uuid) -> PathBuf {
        let path = root.join(id.to_string());
        fs::create_dir_all(&path).unwrap();
        ArtifactInfo::new(TestModel::model_info())
            .write_sidecar(&path)
            .unwrap();
        path
    }

    fn write_event<T: Serialize>(path: &Path, event: Event<T>) {
        fs::write(
            path,
            format!("{}\n", serde_json::to_string(&event).unwrap()),
        )
        .unwrap();
    }

    #[test]
    fn imports_all_files_in_schema_and_file_order() {
        let root = tempfile::tempdir().unwrap();
        let id = Uuid::from_u128(2);
        let context = context(root.path(), id);
        fs::create_dir(context.join(AlphaEvent::NAME)).unwrap();
        fs::create_dir(context.join(BetaEvent::NAME)).unwrap();
        write_event(
            &context.join(AlphaEvent::NAME).join("b.ndjson"),
            Event::new(Uuid::from_u128(12), 12, AlphaEvent(2)),
        );
        write_event(
            &context.join(AlphaEvent::NAME).join("a.ndjson"),
            Event::new(Uuid::from_u128(11), 11, AlphaEvent(1)),
        );
        write_event(
            &context.join(BetaEvent::NAME).join("a.ndjson"),
            Event::new(Uuid::from_u128(13), 1, BetaEvent(3)),
        );

        let store = FilesystemEventStore::<TestModel>::new(root.path());
        let events = store.import_events(id).unwrap().collect::<Vec<_>>();

        assert_eq!(
            events
                .into_iter()
                .map(|event| event.data)
                .collect::<Vec<_>>(),
            [
                TestEvent::Alpha(AlphaEvent(1)),
                TestEvent::Alpha(AlphaEvent(2)),
                TestEvent::Beta(BetaEvent(3)),
            ]
        );
    }

    #[test]
    fn queries_matching_contexts_and_nonempty_entity_streams() {
        let root = tempfile::tempdir().unwrap();
        let first = Uuid::from_u128(1);
        let second = Uuid::from_u128(2);
        let first_path = context(root.path(), first);
        let second_path = context(root.path(), second);
        fs::create_dir(first_path.join(AlphaEvent::NAME)).unwrap();
        fs::create_dir(second_path.join(AlphaEvent::NAME)).unwrap();
        fs::write(first_path.join(AlphaEvent::NAME).join("empty.ndjson"), []).unwrap();
        write_event(
            &second_path.join(AlphaEvent::NAME).join("events.ndjson"),
            Event::new(Uuid::from_u128(3), 3, AlphaEvent(3)),
        );

        let store = FilesystemEventStore::<TestModel>::new(root.path());
        assert_eq!(store.context_ids().unwrap(), [first, second]);
        assert_eq!(store.contexts_with_events::<Alpha>().unwrap(), [second]);
    }

    #[test]
    fn rejects_empty_mixed_unsupported_and_mismatched_contexts() {
        let root = tempfile::tempdir().unwrap();
        let store = FilesystemEventStore::<TestModel>::new(root.path());

        let empty = Uuid::from_u128(1);
        let empty_path = context(root.path(), empty);
        fs::create_dir(empty_path.join(AlphaEvent::NAME)).unwrap();
        fs::write(empty_path.join(AlphaEvent::NAME).join("empty.ndjson"), []).unwrap();
        assert!(matches!(
            store.import_events(empty),
            Err(StoreError::EmptyContext)
        ));

        let mixed = Uuid::from_u128(2);
        let mixed_path = context(root.path(), mixed);
        fs::create_dir(mixed_path.join(AlphaEvent::NAME)).unwrap();
        fs::write(mixed_path.join(AlphaEvent::NAME).join("a.ndjson"), b"x").unwrap();
        fs::write(mixed_path.join(AlphaEvent::NAME).join("b.msgpack"), b"x").unwrap();
        assert!(matches!(
            store.import_events(mixed),
            Err(StoreError::MixedFormats { .. })
        ));

        let unsupported = Uuid::from_u128(3);
        let unsupported_path = context(root.path(), unsupported);
        fs::create_dir(unsupported_path.join(AlphaEvent::NAME)).unwrap();
        fs::write(
            unsupported_path.join(AlphaEvent::NAME).join("events.csv"),
            b"x",
        )
        .unwrap();
        assert!(matches!(
            store.import_events(unsupported),
            Err(StoreError::UnsupportedFormat(format)) if format == "csv"
        ));

        let mismatch = Uuid::from_u128(4);
        let mismatch_path = context(root.path(), mismatch);
        let mut info = TestModel::model_info();
        info.name = "Other".to_owned();
        ArtifactInfo::new(info)
            .write_sidecar(&mismatch_path)
            .unwrap();
        assert!(matches!(
            store.import_events(mismatch),
            Err(StoreError::ModelMismatch { actual, .. }) if actual == "Other"
        ));
    }
}
