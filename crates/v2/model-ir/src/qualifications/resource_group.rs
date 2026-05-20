#[derive(Debug, PartialEq, Eq)]
pub struct ResourceGroup {
    /// Whether this entity is a root resource group, i.e. has no parent.
    pub is_root: bool,
}

/// IR of marking an entity reference is to be used by entities to qualify as a
/// ResourceGroup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RgRefKind {
    /// The reference is referring to the parent resource group.
    Parent,
}
