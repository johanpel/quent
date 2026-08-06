// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Filesystem-backed event storage.

use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use quent_build_info::ArtifactInfo;
use quent_events::{EntityEvent, Event, Model as EventModel, ModelEvents};
use quent_io::ImporterProvider;
use quent_io::filesystem::{Format, importer};
use serde::de::DeserializeOwned;
use uuid::Uuid;

use super::{
    EntityEventLoader, EntityEventStore, EventIterator, ModelEventLoader, ModelEventStore,
    StoredEntity,
};

/// Result returned by filesystem event stores.
pub type Result<T> = std::result::Result<T, Error>;

/// An error encountered while loading filesystem events.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("context `{0}` was not found")]
    ContextNotFound(Uuid),
    #[error("context model `{actual}` does not match expected model `{expected}`")]
    ModelMismatch { expected: String, actual: String },
    #[error("context contains an unsupported event format `{0}`")]
    UnsupportedFormat(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Importer(#[from] quent_io::ImporterError),
}

/// Associates a generated model with its filesystem entity-event streams.
#[doc(hidden)]
pub trait Model: ModelEvents {
    /// Returns the streams generated from the model schema.
    fn event_streams() -> &'static [EventStream<Self>]
    where
        Self: Sized;
}

type ImportFn<M> =
    fn(Vec<EventFile>) -> Result<EventIterator<<M as ModelEvents>::UmbrellaEvent, Error>>;

/// Describes one entity-event stream in a generated analysis model.
pub struct EventStream<M: ModelEvents> {
    entity: &'static str,
    import: ImportFn<M>,
}

impl<M: ModelEvents> EventStream<M> {
    /// Creates a generated entity-event stream descriptor.
    #[doc(hidden)]
    pub const fn new(entity: &'static str, import: ImportFn<M>) -> Self {
        Self { entity, import }
    }
}

/// Identifies an event file and the importer required to decode it.
#[doc(hidden)]
pub struct EventFile {
    format: Format,
    path: PathBuf,
}

/// Imports files containing entity events and converts them to the model umbrella type.
#[doc(hidden)]
pub fn import_event_files<M, E>(
    files: Vec<EventFile>,
) -> Result<EventIterator<M::UmbrellaEvent, Error>>
where
    M: ModelEvents,
    E: DeserializeOwned + Into<M::UmbrellaEvent> + 'static,
    M::UmbrellaEvent: 'static,
{
    let streams = import_files::<E>(files)?
        .map(|stream| {
            Box::new(stream.map(|event| {
                event
                    .map(|event| Event::new(event.id, event.timestamp, event.data.into()))
                    .map_err(Error::from)
            })) as EventIterator<M::UmbrellaEvent, Error>
        })
        .collect::<Vec<_>>();
    Ok(Box::new(streams.into_iter().flatten()))
}

/// Loads model events from filesystem exporter output.
pub struct Store<M> {
    root: PathBuf,
    model: PhantomData<fn() -> M>,
}

impl<M> Store<M> {
    /// Creates a store rooted at an exporter output directory.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            model: PhantomData,
        }
    }

    /// Returns the exporter output directory.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

impl<M> EntityEventStore<M> for Store<M> {
    type Error = Error;
}

impl<M, E> EntityEventLoader<E> for Store<M>
where
    M: EventModel,
    E: StoredEntity<M>,
    E::Event: DeserializeOwned + 'static,
{
    type Error = Error;

    fn load_entity_events(&self, context_id: Uuid) -> Result<EventIterator<E::Event, Error>> {
        let context = self.context(context_id)?;
        let streams = import_files::<E::Event>(event_files(&context, E::Event::NAME)?)?;
        Ok(Box::new(
            streams.flatten().map(|event| event.map_err(Error::from)),
        ))
    }
}

impl<M: Model> ModelEventStore<M> for Store<M> {}

impl<M> ModelEventLoader<M> for Store<M>
where
    M: EventModel + Model + 'static,
{
    type Error = Error;

    fn load_model_events(
        &self,
        context_id: Uuid,
    ) -> Result<EventIterator<M::UmbrellaEvent, Error>> {
        let context = self.context(context_id)?;
        let mut streams = Vec::new();
        for descriptor in M::event_streams() {
            let files = event_files(&context, descriptor.entity)?;
            streams.push((descriptor.import)(files)?);
        }
        Ok(Box::new(streams.into_iter().flatten()))
    }
}

impl<M> Store<M>
where
    M: EventModel,
{
    fn context(&self, context_id: Uuid) -> Result<PathBuf> {
        let context = self.root.join(context_id.to_string());
        if !context.is_dir() {
            return Err(Error::ContextNotFound(context_id));
        }
        let artifact = ArtifactInfo::read_sidecar(&context)?;
        if artifact.model.name != M::NAME {
            return Err(Error::ModelMismatch {
                expected: M::NAME.to_owned(),
                actual: artifact.model.name,
            });
        }
        Ok(context)
    }
}

fn import_files<T>(
    files: Vec<EventFile>,
) -> Result<impl Iterator<Item = Box<dyn quent_io::Importer<T>>>>
where
    T: DeserializeOwned + 'static,
{
    files
        .into_iter()
        .map(|file| {
            let importer = importer::Options {
                format: file.format,
                path: file.path,
            }
            .create_importer()?;
            Ok(importer)
        })
        .collect::<Result<Vec<_>>>()
        .map(Vec::into_iter)
}

fn event_files(context: &Path, entity: &str) -> Result<Vec<EventFile>> {
    let directory = context.join(entity);
    if !directory.is_dir() {
        return Ok(Vec::new());
    }

    let mut paths = std::fs::read_dir(directory)?
        .filter_map(|entry| match entry {
            Ok(entry) => match entry.file_type() {
                Ok(file_type) if file_type.is_file() => Some(Ok(entry.path())),
                Ok(_) => None,
                Err(error) => Some(Err(Error::Io(error))),
            },
            Err(error) => Some(Err(Error::Io(error))),
        })
        .collect::<Result<Vec<_>>>()?;
    paths.sort();

    paths
        .into_iter()
        .map(|path| {
            let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
                return Err(Error::UnsupportedFormat(String::new()));
            };
            let format = Format::try_from(extension)
                .map_err(|_| Error::UnsupportedFormat(extension.to_owned()))?;
            Ok(EventFile { format, path })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use quent_build_info::{BuildInfo, ModelSource};
    use quent_events::{Entity, EntityEvent, Event, Model as EventModel, ModelEvents};
    use quent_instrumentation::{ContextExporter, ContextInner};
    use quent_io::{ExporterOptions, FileSystemExporterOptions, FileSystemFormat};
    use serde::{Deserialize, Serialize};

    use super::*;

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

    impl StoredEntity<TestModel> for Alpha {}

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

    impl EventModel for TestModel {
        const NAME: &'static str = "Test";
    }

    impl ModelSource for TestModel {
        fn package() -> &'static str {
            "quent-store"
        }

        fn source() -> BuildInfo {
            BuildInfo::unknown()
        }
    }

    struct OtherModel;

    impl EventModel for OtherModel {
        const NAME: &'static str = "Other";
    }

    impl ModelSource for OtherModel {
        fn package() -> &'static str {
            "quent-store"
        }

        fn source() -> BuildInfo {
            BuildInfo::unknown()
        }
    }

    impl ModelEvents for TestModel {
        type UmbrellaEvent = TestEvent;
    }

    impl Model for TestModel {
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

    fn context<M>(root: &Path, id: Uuid) -> (ContextInner, ExporterOptions)
    where
        M: EventModel + ModelSource,
    {
        let context = ContextInner::try_new(id).unwrap();
        let options = ExporterOptions::FileSystem(FileSystemExporterOptions::new(
            FileSystemFormat::Ndjson,
            root.to_path_buf(),
        ));
        options.prepare_context(id, M::model_info());
        (context, options)
    }

    fn export_events(root: &Path, id: Uuid) {
        let (context, options) = context::<TestModel>(root, id);
        let alpha = context
            .block_on(context.observer::<AlphaEvent>(&options))
            .unwrap();
        let beta = context
            .block_on(context.observer::<BetaEvent>(&options))
            .unwrap();

        alpha.send(Event::new(Uuid::from_u128(11), 11, AlphaEvent(1)));
        beta.send(Event::new(Uuid::from_u128(12), 1, BetaEvent(2)));
    }

    #[test]
    fn loads_all_model_events_without_relying_on_order() {
        let root = tempfile::tempdir().unwrap();
        let id = Uuid::from_u128(2);
        export_events(root.path(), id);

        let store = Store::<TestModel>::new(root.path());
        let events = store
            .events(id)
            .unwrap()
            .map(|event| event.map(|event| event.data))
            .collect::<Result<Vec<_>>>()
            .unwrap();

        assert_eq!(events.len(), 2);
        assert!(events.contains(&TestEvent::Alpha(AlphaEvent(1))));
        assert!(events.contains(&TestEvent::Beta(BetaEvent(2))));
    }

    #[test]
    fn loads_one_entity_type_as_concrete_events() {
        let root = tempfile::tempdir().unwrap();
        let id = Uuid::from_u128(2);
        export_events(root.path(), id);

        let store = Store::<TestModel>::new(root.path());
        let events = store
            .entity_events::<Alpha>(id)
            .unwrap()
            .collect::<Result<Vec<_>>>()
            .unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, AlphaEvent(1));
    }

    #[test]
    fn validates_context_and_supported_formats() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::<TestModel>::new(root.path());

        let missing = Uuid::from_u128(1);
        assert!(matches!(
            store.events(missing),
            Err(Error::ContextNotFound(id)) if id == missing
        ));

        let unsupported = Uuid::from_u128(2);
        export_events(root.path(), unsupported);
        let unsupported_path = root.path().join(unsupported.to_string());
        fs::write(
            unsupported_path.join(AlphaEvent::NAME).join("events.csv"),
            b"event",
        )
        .unwrap();
        assert!(matches!(
            store.events(unsupported),
            Err(Error::UnsupportedFormat(format)) if format == "csv"
        ));

        let mismatch = Uuid::from_u128(3);
        context::<OtherModel>(root.path(), mismatch);
        assert!(matches!(
            store.events(mismatch),
            Err(Error::ModelMismatch { actual, .. }) if actual == "Other"
        ));
    }

    #[test]
    fn returns_event_files_in_path_order() {
        let root = tempfile::tempdir().unwrap();
        let entity = root.path().join(AlphaEvent::NAME);
        fs::create_dir(&entity).unwrap();
        for name in ["charlie.ndjson", "alpha.ndjson", "bravo.ndjson"] {
            fs::write(entity.join(name), b"").unwrap();
        }

        let paths = event_files(root.path(), AlphaEvent::NAME)
            .unwrap()
            .into_iter()
            .map(|file| file.path.file_name().unwrap().to_owned())
            .collect::<Vec<_>>();

        assert_eq!(paths, ["alpha.ndjson", "bravo.ndjson", "charlie.ndjson"]);
    }

    #[test]
    fn reports_import_failures_during_iteration() {
        let root = tempfile::tempdir().unwrap();
        let id = Uuid::from_u128(2);
        let context_path = root.path().join(id.to_string());
        context::<TestModel>(root.path(), id);
        let entity = context_path.join(AlphaEvent::NAME);
        fs::create_dir(&entity).unwrap();
        fs::write(entity.join("events.ndjson"), b"not json\n").unwrap();

        let store = Store::<TestModel>::new(root.path());
        let mut events = store.entity_events::<Alpha>(id).unwrap();

        assert!(matches!(events.next(), Some(Err(Error::Importer(_)))));
        assert!(events.next().is_none());
    }
}
