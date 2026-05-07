// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

// This file is an example of what a rewrite of the query engine domain types would look like

use crate::v2::{
    entity::{EntityDeclaration, Ref},
    resource_group::ResourceGroupAttributes,
};
use quent_model_macros::{Entity, Fsm, ResourceGroup, RootResourceGroup};

mod engine {
    use super::*;

    pub struct EngineImplementationAttributes {
        pub name: Option<String>,
        pub version: Option<String>,
        pub custom_attributes: quent_attributes::CustomAttributes,
    }

    pub struct Init {
        pub implementation: EngineImplementationAttributes,
        pub instance_name: Option<String>,
    }

    #[derive(Fsm, RootResourceGroup)]
    #[quent(transitions = {
        entry -> Init,
        Init -> exit
    })]
    pub enum Engine {
        Init(Init),
    }
}

mod query_group {
    use super::*;

    #[derive(Entity, ResourceGroup)]
    pub struct QueryGroup {
        pub instance_name: String,
    }
}

mod query {
    use super::*;

    #[derive(Fsm, ResourceGroup)]
    #[quent(transitions = {
        entry -> Init,
        Init -> Planning,
        Planning -> Executing,
        Executing -> exit
    })]
    pub enum Query {
        Init(ResourceGroupAttributes),
        Planning,
        Executing,
    }
}

mod operator {
    use super::*;

    pub struct Declaration {
        pub parent_plan_operators: Vec<Ref<Operator>>,
        pub instance_name: String,
        pub type_name: String,
        pub custom_attributes: quent_attributes::CustomAttributes,
    }

    pub struct Statistics {
        pub custom_attributes: quent_attributes::CustomAttributes,
    }

    #[derive(Entity, ResourceGroup)]
    pub enum Operator {
        Declaration(Declaration, ResourceGroupAttributes),
        Statistics(Statistics),
    }
}

mod port {
    use super::*;

    pub struct Declaration {
        pub operator: Ref<operator::Operator>,
        pub instance_name: String,
    }

    pub struct Statistics {
        pub custom_attributes: quent_attributes::CustomAttributes,
    }

    #[derive(Entity, ResourceGroup)]
    pub enum Port {
        Declaration(Declaration, ResourceGroupAttributes),
        Statistics(Statistics),
    }
}

mod plan {
    use super::*;

    pub struct Edge {
        pub source: Ref<port::Port>,
        pub target: Ref<port::Port>,
    }

    // spec doesn't allow enum attributes, so we have to use mutually exclusive optionals
    pub struct PlanParent {
        pub query: Option<Ref<super::query::Query>>,
        pub plan: Option<Ref<super::plan::Plan>>,
    }

    #[derive(Entity, ResourceGroup)]
    pub struct Plan {
        // parent resource group id is either the worker id for worker-local
        // plan copies/instances, or the query id for plans that are
        // cluster-wide or haven't been lowered to worker-local plans yet.
        pub instance_name: String,
        pub edges: Vec<Edge>,
        pub plan_parent: PlanParent,
    }
}

mod worker {
    use super::*;

    pub struct Init {
        pub instance_name: String,
    }

    #[derive(Fsm, ResourceGroup)]
    #[quent(transitions = {
        entry -> Init,
        Init -> exit
    })]
    pub enum Worker {
        Init(Init),
    }
}

// to be generated, but done here manually so it compiles:

impl EntityDeclaration for engine::Engine {}
impl EntityDeclaration for worker::Worker {}
impl EntityDeclaration for query_group::QueryGroup {}
impl EntityDeclaration for query::Query {}
impl EntityDeclaration for plan::Plan {}
impl EntityDeclaration for operator::Operator {}
impl EntityDeclaration for port::Port {}
