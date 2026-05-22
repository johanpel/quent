// // SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// // SPDX-License-Identifier: Apache-2.0

// #![allow(unused)]

// use quent_v2_model::{AnyRg, Entity, entity_ref::EntityRef, scope::RgParentRef};

// use crate::source::attributes;

// #[derive(Entity)]
// #[quent(resource_group(root))]
// pub struct RootUnit;

// #[derive(Entity)]
// #[quent(resource_group(root))]
// pub struct RootUnitBraces {}

// #[derive(Entity)]
// #[quent(resource_group(root))]
// pub struct RootStructPrim {
//     pub a: u8,
// }

// #[derive(Entity)]
// #[quent(resource_group(root))]
// pub struct RootStructMultiAttrib {
//     pub a: u8,
//     pub b: String,
// }

// #[derive(Entity)]
// #[quent(resource_group(root))]
// pub enum RootEnumOneUnit {
//     A,
// }

// #[derive(Entity)]
// #[quent(resource_group(root))]
// pub enum RootEnumMultiUnit {
//     A,
//     B,
// }

// #[derive(Entity)]
// #[quent(resource_group(root))]
// pub enum RootEnumSingleAttribs {
//     A(attributes::OnePrim),
// }

// #[derive(Entity)]
// #[quent(resource_group(root))]
// pub enum RootEnumMultiAttribs {
//     A(attributes::OnePrim),
//     B(attributes::MultiPrim),
// }

// // Things that shouldn't compile because they dont set the parent role field
// // TODO: move to compile fail tests
// // #[derive(Entity)]
// // #[quent(resource_group)]
// // pub struct Unit;

// // #[derive(Entity)]
// // #[quent(resource_group)]
// // pub struct UnitBraces {}

// // #[derive(Entity)]
// // #[quent(resource_group)]
// // pub struct StructPrim {
// //     pub a: u8,
// // }

// // #[derive(Entity)]
// // #[quent(resource_group)]
// // pub struct StructMultiAttrib {
// //     a: u8,
// //     b: String,
// // }

// // #[derive(Entity)]
// // #[quent(resource_group)]
// // pub enum EnumOneUnit {
// //     A,
// // }

// // #[derive(Entity)]
// // #[quent(resource_group)]
// // pub enum EnumMultiUnit {
// //     A,
// //     B,
// // }

// #[derive(Entity)]
// #[quent(resource_group)]
// pub enum EnumSingleAttribs {
//     A {
//         payload: attributes::OnePrim,
//         parent: EntityRef<AnyRg, RgParentRef>,
//     },
// }

// #[derive(Entity)]
// #[quent(resource_group)]
// pub enum EnumMultiAttribs {
//     A {
//         payload: attributes::OnePrim,
//         parent: EntityRef<AnyRg, RgParentRef>,
//     },
//     B(attributes::MultiPrim),
// }
