#[derive(Debug, PartialEq, Eq)]
pub struct ResourceGroup;

/// IR of marking an entity reference is to be used by entities to qualify as a
/// ResourceGroup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RgRefKind {
    /// The reference is referring to the parent resource group.
    Parent,
}
