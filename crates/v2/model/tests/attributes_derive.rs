use quent_v2_model::{
    Attributes,
    attributes::{ModelAttributes, ModelValueType},
};

#[derive(Attributes)]
struct Checksum {
    algorithm: String,
    value: String,
}

#[test]
fn checksum_attributes_def() {
    let def = Checksum::attributes_def();
    assert_eq!(def.name, "Checksum");
    assert!(def.rust_path.ends_with("::Checksum"));
    assert_eq!(def.fields.len(), 2);
    assert_eq!(def.fields[0].name, "algorithm");
}

#[test]
fn checksum_value_type() {
    match Checksum::value_type() {
        quent_v2_model::ir::attributes::ValueType::Attributes(name) => {
            assert_eq!(name, "Checksum");
        }
        _ => panic!("expected Attributes variant"),
    }
}
