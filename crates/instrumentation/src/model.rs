// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Instrumentation models and their contexts.

use crate::{Context, Entity, EntityObserver, ExporterOptions, Uuid, build_info, write_sidecar};

/// Supplies schema-specific observers and metadata to an instrumentation context.
pub trait Model: Sized {
    /// Observer storage for this model.
    type Observers;

    /// Builds one shared observer for every entity in this model.
    ///
    /// `exporter` is `None` for a no-op context.
    ///
    /// # Errors
    ///
    /// Returns an error when an observer or its exporter cannot be constructed.
    #[doc(hidden)]
    fn build_observers(
        context: &Context,
        exporter: Option<&ExporterOptions>,
    ) -> Result<Self::Observers, Box<dyn std::error::Error>>;

    /// Returns metadata describing this instrumentation model.
    fn model_info() -> build_info::ModelInfo;
}

/// Instrumentation context for a generated model.
pub struct ModelContext<M: Model> {
    observers: M::Observers,
    inner: Context,
}

impl<M: Model> ModelContext<M> {
    /// Creates a context and builds every entity's exporter pipeline.
    ///
    /// Passing `None` creates a no-op context that discards events.
    pub fn try_new(exporter: Option<ExporterOptions>) -> Result<Self, Box<dyn std::error::Error>> {
        Self::try_with_id(Uuid::now_v7(), exporter)
    }

    /// Creates a context with the supplied ID.
    pub fn try_with_id(
        id: Uuid,
        exporter: Option<ExporterOptions>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let inner = if exporter.is_some() {
            Context::try_new(id)?
        } else {
            Context::noop(id)
        };
        if let Some(options) = &exporter {
            write_sidecar(options, id, M::model_info());
        }
        let observers = M::build_observers(&inner, exporter.as_ref())?;
        Ok(Self { observers, inner })
    }

    /// Returns the context ID.
    pub fn id(&self) -> Uuid {
        self.inner.id()
    }

    /// Returns the observer associated with entity marker `E`.
    pub fn observer<E>(&self) -> EntityObserver<E>
    where
        E: Entity<Context = Self>,
    {
        EntityObserver::new(E::observer(self))
    }

    /// Returns the model-specific observer storage.
    ///
    /// This preserves its concrete type so [`Entity`] implementations can
    /// select their associated observer.
    #[doc(hidden)]
    pub fn observers(&self) -> &M::Observers {
        &self.observers
    }
}
