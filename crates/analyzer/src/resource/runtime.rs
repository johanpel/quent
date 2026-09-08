// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Run-time defined Resources and Resource Groups (in analysis)

use quent_time::{OrderKey, OrderedCollector, TimeUnixNanoSec, Timestamp};
use uuid::Uuid;

use crate::{
    AnalyzerError, AnalyzerResult, Entity,
    fsm::{Fsm, Transition},
    resource::{Resource, ResourceCapacities, ResourceGroup},
};

/// Resource state transitions.
pub enum RtResourceTransition {
    Init(TimeUnixNanoSec),
    Operating(TimeUnixNanoSec, ResourceCapacities),
    Resizing(TimeUnixNanoSec),
    Finalizing(TimeUnixNanoSec),
    Exit(TimeUnixNanoSec),
}

impl Timestamp for RtResourceTransition {
    fn timestamp(&self) -> TimeUnixNanoSec {
        *match self {
            RtResourceTransition::Init(ts) => ts,
            RtResourceTransition::Operating(ts, _) => ts,
            RtResourceTransition::Resizing(ts) => ts,
            RtResourceTransition::Finalizing(ts) => ts,
            RtResourceTransition::Exit(ts) => ts,
        }
    }
}

impl OrderKey for RtResourceTransition {
    type Key = TimeUnixNanoSec;

    fn order_key(&self) -> Self::Key {
        self.timestamp()
    }
}

impl Transition for RtResourceTransition {
    fn name(&self) -> &str {
        match self {
            RtResourceTransition::Init(_) => "init",
            RtResourceTransition::Operating(_, _) => "operating",
            RtResourceTransition::Resizing(_) => "resizing",
            RtResourceTransition::Finalizing(_) => "finalizing",
            RtResourceTransition::Exit(_) => "exit",
        }
    }
}

pub struct RtResourceBuilder {
    id: Uuid,
    instance_name: Option<String>,
    type_name: Option<String>,
    parent_group_id: Option<Uuid>,
    transitions: OrderedCollector<RtResourceTransition>,
}

impl RtResourceBuilder {
    pub fn try_new(id: Uuid) -> AnalyzerResult<Self> {
        if id.is_nil() {
            Err(AnalyzerError::InvalidId(id))
        } else {
            Ok(Self {
                id,
                instance_name: None,
                type_name: None,
                parent_group_id: None,
                transitions: Default::default(),
            })
        }
    }
    pub fn set_type_name(&mut self, type_name: impl Into<String>) {
        self.type_name = Some(type_name.into());
    }
    pub fn set_instance_name(&mut self, instance_name: Option<String>) {
        self.instance_name = instance_name;
    }
    pub fn set_parent_group_id(&mut self, parent: Uuid) {
        self.parent_group_id = Some(parent);
    }
    pub fn push(&mut self, transition: RtResourceTransition) {
        self.transitions.push(transition);
    }
    pub fn try_build(self) -> AnalyzerResult<RtResource> {
        let transitions: Vec<RtResourceTransition> = self.transitions.into_inner();

        match transitions.first() {
            None => {
                return Err(AnalyzerError::Validation(format!(
                    "resource {} has no transitions",
                    self.id
                )));
            }
            Some(RtResourceTransition::Init(_)) => {}
            Some(_) => {
                return Err(AnalyzerError::Validation(format!(
                    "first state of resource {} is not init",
                    self.id
                )));
            }
        }

        // TODO(johanpel): validate more transition logic

        Ok(RtResource {
            id: self.id,
            instance_name: self.instance_name.ok_or_else(|| {
                AnalyzerError::IncompleteEntity(format!(
                    "resource {} must have an instance name",
                    self.id
                ))
            })?,
            type_name: self.type_name.ok_or_else(|| {
                AnalyzerError::IncompleteEntity(format!(
                    "resource {} must have a type name",
                    self.id
                ))
            })?,
            parent_group_id: self.parent_group_id.ok_or_else(|| {
                AnalyzerError::IncompleteEntity(format!(
                    "resource {} must have a parent resource group",
                    self.id
                ))
            })?,
            transitions,
        })
    }
}

/// A Resource.
pub struct RtResource {
    /// The ID of this Resource.
    pub id: Uuid,
    /// The name of this Resource.
    pub instance_name: String,
    /// The unique type name of this Resource.
    pub type_name: String,
    /// The id of the parent Resource Group.
    pub parent_group_id: Uuid,
    /// The sequence of state transitions this resource went through.
    pub transitions: Vec<RtResourceTransition>,
}

impl Entity for RtResource {
    fn id(&self) -> Uuid {
        self.id
    }
    fn type_name(&self) -> &str {
        self.type_name.as_str()
    }
    fn instance_name(&self) -> &str {
        self.instance_name.as_str()
    }
}

impl Fsm for RtResource {
    type TransitionType = RtResourceTransition;
    fn len(&self) -> usize {
        self.transitions.len().saturating_sub(1)
    }
    fn transition(&self, index: usize) -> Option<&Self::TransitionType> {
        self.transitions.get(index)
    }
}

impl Resource for RtResource {
    fn parent_group_id(&self) -> Uuid {
        self.parent_group_id
    }
}

/// A Group of [`Resource`]s.
#[derive(Clone, Debug, Default)]
pub struct RtResourceGroup {
    /// The ID of this Resource Group.
    pub id: Uuid,
    /// The name of the type of Resource Group
    pub type_name: String,
    /// The name of the instance of this Resource Group.
    pub instance_name: String,
    /// The parent of this Resource Group.
    ///
    /// If this is None, it is considered the root of the global application's
    /// resource tree.
    pub parent_group_id: Option<Uuid>,
}

impl RtResourceGroup {
    pub fn try_new(
        id: Uuid,
        type_name: String,
        instance_name: String,
        parent_group_id: Option<Uuid>,
    ) -> AnalyzerResult<Self> {
        if id.is_nil() {
            Err(AnalyzerError::InvalidId(id))
        } else {
            Ok(Self {
                id,
                type_name,
                instance_name,
                parent_group_id,
            })
        }
    }
}

impl Entity for RtResourceGroup {
    fn id(&self) -> Uuid {
        self.id
    }
    fn type_name(&self) -> &str {
        self.type_name.as_str()
    }
    fn instance_name(&self) -> &str {
        self.instance_name.as_str()
    }
}

impl ResourceGroup for RtResourceGroup {
    fn parent_group_id(&self) -> Option<Uuid> {
        self.parent_group_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn builder() -> RtResourceBuilder {
        let mut builder = RtResourceBuilder::try_new(Uuid::from_u128(1)).unwrap();
        builder.set_type_name("memory");
        builder.set_instance_name(Some("host memory".to_owned()));
        builder.set_parent_group_id(Uuid::from_u128(2));
        builder
    }

    #[test]
    fn resource_does_not_require_an_exit_transition() {
        let mut builder = builder();
        builder.push(RtResourceTransition::Init(10));
        builder.push(RtResourceTransition::Operating(
            20,
            ResourceCapacities(Vec::new()),
        ));

        let resource = builder.try_build().unwrap();

        assert_eq!(resource.len(), 1);
        assert_eq!(resource.last().unwrap().name(), "init");
        assert_eq!(resource.last_transition().unwrap().name(), "operating");
    }

    #[test]
    fn resource_with_only_init_is_incomplete_but_valid() {
        let mut builder = builder();
        builder.push(RtResourceTransition::Init(10));

        let resource = builder.try_build().unwrap();

        assert!(resource.is_empty());
        assert!(resource.first().is_none());
        assert!(resource.last().is_none());
        assert_eq!(resource.last_transition().unwrap().name(), "init");
    }
}
