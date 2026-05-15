impl ModelEntityRefTarget for AnyRg {
    fn model_entity_ref_target() -> EntityRefTarget {
        EntityRefTarget::AnyQualified(QualificationKind::ResourceGroup)
    }
}

impl ModelEntityRefKind for RgParentRef {
    fn model_entity_ref_kind() -> EntityRefKind {
        EntityRefKind::Qualification(QualificationRefKind::ResourceGroup(RgRefKind::Parent))
    }
}
