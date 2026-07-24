// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::fmt::{Display, Formatter};
use std::str::FromStr;

use thiserror::Error;

use crate::Identifier;

/// A nonempty absolute path of identifier segments.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "Vec<Identifier>"))]
pub struct Path(Vec<Identifier>);

#[cfg(feature = "ts")]
impl ts_rs::TS for Path {
    type WithoutGenerics = Self;
    type OptionInnerType = Self;

    fn name(_: &ts_rs::Config) -> String {
        "Path".to_owned()
    }

    fn inline(config: &ts_rs::Config) -> String {
        format!("Array<{}>", Identifier::inline(config))
    }

    fn decl(config: &ts_rs::Config) -> String {
        format!("type Path = {};", Self::inline(config))
    }

    fn output_path() -> Option<std::path::PathBuf> {
        Some("Path.ts".into())
    }
}

/// Reason a path failed validation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PathError {
    /// No segments were provided.
    #[error("path must contain at least one segment")]
    Empty,
    /// A string contained an invalid segment.
    #[error("invalid path segment {index}: {source}")]
    InvalidSegment {
        index: usize,
        #[source]
        source: crate::schema::identifier::IdentifierError,
    },
}

impl Path {
    /// Returns all path segments.
    pub fn segments(&self) -> &[Identifier] {
        &self.0
    }

    /// Returns the segments preceding the path name.
    pub fn namespace(&self) -> &[Identifier] {
        &self.0[..self.0.len() - 1]
    }

    /// Returns the final path segment.
    pub fn name(&self) -> &Identifier {
        self.0.last().expect("Path guarantees at least one segment")
    }

    /// Returns this path with its final segment replaced.
    pub fn with_name(&self, name: Identifier) -> Self {
        let mut segments = self.0.clone();
        *segments
            .last_mut()
            .expect("Path guarantees at least one segment") = name;
        Self(segments)
    }
}

impl From<Identifier> for Path {
    fn from(identifier: Identifier) -> Self {
        Self(vec![identifier])
    }
}

impl TryFrom<Vec<Identifier>> for Path {
    type Error = PathError;

    fn try_from(segments: Vec<Identifier>) -> Result<Self, Self::Error> {
        if segments.is_empty() {
            return Err(PathError::Empty);
        }
        Ok(Self(segments))
    }
}

impl FromStr for Path {
    type Err = PathError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.is_empty() {
            return Err(PathError::Empty);
        }
        value
            .split("::")
            .enumerate()
            .map(|(index, segment)| {
                Identifier::try_new(segment)
                    .map_err(|source| PathError::InvalidSegment { index, source })
            })
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
    }
}

impl Display for Path {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        let mut segments = self.0.iter();
        formatter.write_str(
            segments
                .next()
                .expect("Path guarantees at least one segment"),
        )?;
        for segment in segments {
            write!(formatter, "::{segment}")?;
        }
        Ok(())
    }
}

impl PartialEq<str> for Path {
    fn eq(&self, other: &str) -> bool {
        let mut other_segments = other.split("::");
        self.0
            .iter()
            .all(|segment| other_segments.next().is_some_and(|other| segment == other))
            && other_segments.next().is_none()
    }
}

impl PartialEq<&str> for Path {
    fn eq(&self, other: &&str) -> bool {
        self == *other
    }
}

impl<'a> IntoIterator for &'a Path {
    type Item = &'a Identifier;
    type IntoIter = std::slice::Iter<'a, Identifier>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl IntoIterator for Path {
    type Item = Identifier;
    type IntoIter = std::vec::IntoIter<Identifier>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::ident;

    #[test]
    fn constructs_and_inspects_paths() {
        let path = Path::try_from(vec![ident("Foo"), ident("Query")]).unwrap();

        assert_eq!(path.segments(), &[ident("Foo"), ident("Query")]);
        assert_eq!(path.namespace(), &[ident("Foo")]);
        assert_eq!(path.name(), "Query");
        assert_eq!(path.with_name(ident("Result")).to_string(), "Foo::Result");
        assert_eq!(
            path.into_iter()
                .map(|segment| segment.to_string())
                .collect::<Vec<_>>(),
            ["Foo", "Query"]
        );
    }

    #[test]
    fn parses_and_formats_paths() {
        let path: Path = "Foo::Query".parse().unwrap();
        assert_eq!(path.to_string(), "Foo::Query");
        assert_eq!(Path::from(ident("Query")).to_string(), "Query");
    }

    #[test]
    fn rejects_empty_and_invalid_paths() {
        assert_eq!(Path::try_from(Vec::new()), Err(PathError::Empty));
        assert_eq!("".parse::<Path>(), Err(PathError::Empty));
        assert!(matches!(
            "Foo::".parse::<Path>(),
            Err(PathError::InvalidSegment { index: 1, .. })
        ));
        assert!(matches!(
            "Foo::bad-name".parse::<Path>(),
            Err(PathError::InvalidSegment { index: 1, .. })
        ));
    }
}
