// use quent_v2_model_ir::{
//     attributes::EntityRefTarget,
//     entity::{Entity, ModelEntity},
//     event::{Cardinality, Event, Field},
//     value_type::{ModelEntityRefTarget, ValueType},
// };

// use source::entities::*;

// mod source;
// mod utils;

// #[test]
// fn unit_struct() {
//     assert_eq!(
//         Unit::model_entity(),
//         Entity::new(
//             "Unit",
//             vec![Event::new("Unit", Cardinality::Once, vec![])],
//             vec![],
//             utils::rust_path!("source::entities::Unit"),
//         )
//     );
//     assert_eq!(
//         Unit::model_entity_ref_target(),
//         EntityRefTarget::Specific("Unit".into())
//     );

//     assert_eq!(
//         UnitBraces::model_entity(),
//         Entity::new(
//             "UnitBraces",
//             vec![Event::new("UnitBraces", Cardinality::Once, vec![])],
//             vec![],
//             utils::rust_path!("source::entities::UnitBraces"),
//         )
//     );
//     assert_eq!(
//         UnitBraces::model_entity_ref_target(),
//         EntityRefTarget::Specific("UnitBraces".into())
//     );
// }

// #[test]
// #[allow(unused)]
// fn fields_struct() {
//     assert_eq!(
//         StructPrim::model_entity(),
//         Entity::new(
//             "StructPrim",
//             vec![Event::new(
//                 "StructPrim",
//                 Cardinality::Once,
//                 vec![Field::new(
//                     "payload",
//                     ValueType::Attributes("StructPrim".into()),
//                 )],
//             )],
//             vec![],
//             utils::rust_path!("source::entities::StructPrim"),
//         )
//     );
//     assert_eq!(
//         StructPrim::model_entity_ref_target(),
//         EntityRefTarget::Specific("StructPrim".into())
//     );

//     assert_eq!(
//         StructMultiAttrib::model_entity(),
//         Entity::new(
//             "StructMultiAttrib",
//             vec![Event::new(
//                 "StructMultiAttrib",
//                 Cardinality::Once,
//                 vec![Field::new(
//                     "payload",
//                     ValueType::Attributes("StructMultiAttrib".into()),
//                 )],
//             )],
//             vec![],
//             utils::rust_path!("source::entities::StructMultiAttrib"),
//         )
//     );
//     assert_eq!(
//         StructMultiAttrib::model_entity_ref_target(),
//         EntityRefTarget::Specific("StructMultiAttrib".into())
//     );

//     // TODO: struct with more value types including ref and resource usage
// }

// #[test]
// #[allow(unused)]
// fn enums() {
//     assert_eq!(
//         EnumOneUnit::model_entity(),
//         Entity::new(
//             "EnumOneUnit",
//             vec![Event::new("A", Cardinality::Once, vec![])],
//             vec![],
//             utils::rust_path!("source::entities::EnumOneUnit"),
//         )
//     );
//     assert_eq!(
//         EnumOneUnit::model_entity_ref_target(),
//         EntityRefTarget::Specific("EnumOneUnit".into())
//     );

//     assert_eq!(
//         EnumMultiUnit::model_entity(),
//         Entity::new(
//             "EnumMultiUnit",
//             vec![
//                 Event::new("A", Cardinality::Once, vec![]),
//                 Event::new("B", Cardinality::Once, vec![]),
//             ],
//             vec![],
//             utils::rust_path!("source::entities::EnumMultiUnit"),
//         )
//     );
//     assert_eq!(
//         EnumMultiUnit::model_entity_ref_target(),
//         EntityRefTarget::Specific("EnumMultiUnit".into())
//     );

//     assert_eq!(
//         EnumSingleAttribs::model_entity(),
//         Entity::new(
//             "EnumSingleAttribs",
//             vec![Event::new(
//                 "A",
//                 Cardinality::Once,
//                 vec![Field::new(
//                     "payload",
//                     ValueType::Attributes("OnePrim".into()),
//                 )],
//             )],
//             vec![],
//             utils::rust_path!("source::entities::EnumSingleAttribs"),
//         )
//     );
//     assert_eq!(
//         EnumSingleAttribs::model_entity_ref_target(),
//         EntityRefTarget::Specific("EnumSingleAttribs".into())
//     );

//     assert_eq!(
//         EnumMultiAttribs::model_entity(),
//         Entity::new(
//             "EnumMultiAttribs",
//             vec![
//                 Event::new(
//                     "A",
//                     Cardinality::Once,
//                     vec![Field::new(
//                         "payload",
//                         ValueType::Attributes("OnePrim".into()),
//                     )],
//                 ),
//                 Event::new(
//                     "B",
//                     Cardinality::Once,
//                     vec![Field::new(
//                         "payload",
//                         ValueType::Attributes("MultiPrim".into()),
//                     )],
//                 ),
//             ],
//             vec![],
//             utils::rust_path!("source::entities::EnumMultiAttribs"),
//         )
//     );
//     assert_eq!(
//         EnumMultiAttribs::model_entity_ref_target(),
//         EntityRefTarget::Specific("EnumMultiAttribs".into())
//     );
// }
