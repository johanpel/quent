use quent_v2_model::{
    Attributes,
    ir::{
        attributes::{Attributes, Field, ModelAttributes},
        value_type::{ModelValueType, ValueType},
    },
};

mod utils;

// Unit structs
#[test]
fn unit() {
    #[derive(Attributes)]
    struct Unit0;
    assert_eq!(
        Unit0::model_attributes(),
        Attributes {
            name: String::from("Unit0"),
            fields: vec![],
            rust_path: utils::rust_path!("Unit0"),
        }
    );
    assert_eq!(Unit0::model_value_type(), ValueType::attributes("Unit0"));

    #[derive(Attributes)]
    struct Unit1 {}
    assert_eq!(
        Unit1::model_attributes(),
        Attributes {
            name: String::from("Unit1"),
            fields: vec![],
            rust_path: utils::rust_path!("Unit1"),
        }
    );
}

// Single field structs
#[test]
fn single() {
    #[derive(Attributes)]
    struct SinglePrimitive {
        a: u8,
    }
    assert_eq!(
        SinglePrimitive::model_attributes(),
        Attributes {
            name: String::from("SinglePrimitive"),
            fields: vec![Field {
                name: String::from("a"),
                ty: ValueType::U8,
            }],
            rust_path: utils::rust_path!("SinglePrimitive"),
        }
    );

    #[derive(Attributes)]
    struct SingleNested {
        a: SinglePrimitive,
    }
    assert_eq!(
        SingleNested::model_attributes(),
        Attributes {
            name: String::from("SingleNested"),
            fields: vec![Field {
                name: String::from("a"),
                ty: ValueType::Attributes(String::from("SinglePrimitive")),
            }],
            rust_path: utils::rust_path!("SingleNested"),
        }
    );

    #[derive(Attributes)]
    struct SingleList {
        a: Vec<u8>,
    }
    assert_eq!(
        SingleList::model_attributes(),
        Attributes {
            name: String::from("SingleList"),
            fields: vec![Field {
                name: String::from("a"),
                ty: ValueType::List(Box::new(ValueType::U8)),
            }],
            rust_path: utils::rust_path!("SingleList"),
        }
    );

    #[derive(Attributes)]
    struct SingleListNested {
        a: Vec<SinglePrimitive>,
    }
    assert_eq!(
        SingleListNested::model_attributes(),
        Attributes {
            name: String::from("SingleListNested"),
            fields: vec![Field {
                name: String::from("a"),
                ty: ValueType::List(Box::new(ValueType::Attributes(String::from(
                    "SinglePrimitive"
                ),))),
            }],
            rust_path: utils::rust_path!("SingleListNested"),
        }
    );

    #[derive(Attributes)]
    struct SingleListListPrimitive {
        a: Vec<Vec<u8>>,
    }
    assert_eq!(
        SingleListListPrimitive::model_attributes(),
        Attributes {
            name: String::from("SingleListListPrimitive"),
            fields: vec![Field {
                name: String::from("a"),
                ty: ValueType::List(Box::new(ValueType::List(Box::new(ValueType::U8,)))),
            }],
            rust_path: utils::rust_path!("SingleListListPrimitive"),
        }
    );

    #[derive(Attributes)]
    struct SingleListListNested {
        a: Vec<Vec<SinglePrimitive>>,
    }
    assert_eq!(
        SingleListListNested::model_attributes(),
        Attributes {
            name: String::from("SingleListListNested"),
            fields: vec![Field {
                name: String::from("a"),
                ty: ValueType::List(Box::new(ValueType::List(Box::new(ValueType::Attributes(
                    String::from("SinglePrimitive")
                ),)))),
            }],
            rust_path: utils::rust_path!("SingleListListNested"),
        }
    );
}
