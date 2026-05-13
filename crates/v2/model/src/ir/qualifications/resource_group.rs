pub struct ResourceGroup;

/// Meaning bestowed upon references used by entities to qualify as a
/// ResourceGroup.
pub enum RgRefKind {
    /// The reference is referring to the parent resource group.
    Parent,
}
