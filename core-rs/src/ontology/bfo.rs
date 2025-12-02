//! BFO 2020 Type System - Compile-time ontological alignment
//!
//! Provides type-safe enums bound to BFO 2020 URIs for runtime validation.
//! All ConceptKernel entities should implement `BfoAligned` trait.

use serde::{Deserialize, Serialize};

/// BFO 2020 Entity Types
///
/// Each variant maps to a specific BFO URI for ontological alignment.
///
/// # Examples
///
/// ```
/// use ckp_core::ontology::BfoEntityType;
///
/// let kernel_type = BfoEntityType::MaterialEntity;
/// assert_eq!(
///     kernel_type.uri(),
///     "http://purl.obolibrary.org/obo/BFO_0000040"
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum BfoEntityType {
    /// bfo:0000001 - Entity (Top Level)
    Entity,

    /// bfo:0000002 - Continuant (persists through time)
    Continuant,

    /// bfo:0000003 - Occurrent (unfolds in time)
    Occurrent,

    /// bfo:0000004 - Independent Continuant
    IndependentContinuant,

    /// bfo:0000015 - Process
    Process,

    /// bfo:0000017 - Realizable Entity
    RealizableEntity,

    /// bfo:0000023 - Role
    Role,

    /// bfo:0000034 - Function
    Function,

    /// bfo:0000040 - Material Entity
    MaterialEntity,

    /// bfo:0000008 - Temporal Region
    TemporalRegion,

    /// bfo:0000038 - One-Dimensional Temporal Region (temporal part)
    TemporalPart,
}

impl BfoEntityType {
    /// Get BFO 2020 URI for this entity type
    ///
    /// # Examples
    ///
    /// ```
    /// use ckp_core::ontology::BfoEntityType;
    ///
    /// assert_eq!(
    ///     BfoEntityType::Continuant.uri(),
    ///     "http://purl.obolibrary.org/obo/BFO_0000002"
    /// );
    /// ```
    #[must_use]
    pub const fn uri(&self) -> &'static str {
        match self {
            Self::Entity => "http://purl.obolibrary.org/obo/BFO_0000001",
            Self::Continuant => "http://purl.obolibrary.org/obo/BFO_0000002",
            Self::Occurrent => "http://purl.obolibrary.org/obo/BFO_0000003",
            Self::IndependentContinuant => "http://purl.obolibrary.org/obo/BFO_0000004",
            Self::Process => "http://purl.obolibrary.org/obo/BFO_0000015",
            Self::RealizableEntity => "http://purl.obolibrary.org/obo/BFO_0000017",
            Self::Role => "http://purl.obolibrary.org/obo/BFO_0000023",
            Self::Function => "http://purl.obolibrary.org/obo/BFO_0000034",
            Self::MaterialEntity => "http://purl.obolibrary.org/obo/BFO_0000040",
            Self::TemporalRegion => "http://purl.obolibrary.org/obo/BFO_0000008",
            Self::TemporalPart => "http://purl.obolibrary.org/obo/BFO_0000038",
        }
    }

    /// Get human-readable label
    ///
    /// # Examples
    ///
    /// ```
    /// use ckp_core::ontology::BfoEntityType;
    ///
    /// assert_eq!(BfoEntityType::MaterialEntity.label(), "Material Entity");
    /// ```
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Entity => "Entity",
            Self::Continuant => "Continuant",
            Self::Occurrent => "Occurrent",
            Self::IndependentContinuant => "Independent Continuant",
            Self::Process => "Process",
            Self::RealizableEntity => "Realizable Entity",
            Self::Role => "Role",
            Self::Function => "Function",
            Self::MaterialEntity => "Material Entity",
            Self::TemporalRegion => "Temporal Region",
            Self::TemporalPart => "Temporal Part",
        }
    }

    /// Parse from BFO URI
    ///
    /// # Examples
    ///
    /// ```
    /// use ckp_core::ontology::BfoEntityType;
    ///
    /// let parsed = BfoEntityType::from_uri("http://purl.obolibrary.org/obo/BFO_0000040");
    /// assert_eq!(parsed, Some(BfoEntityType::MaterialEntity));
    /// ```
    #[must_use]
    pub fn from_uri(uri: &str) -> Option<Self> {
        match uri {
            "http://purl.obolibrary.org/obo/BFO_0000001" => Some(Self::Entity),
            "http://purl.obolibrary.org/obo/BFO_0000002" => Some(Self::Continuant),
            "http://purl.obolibrary.org/obo/BFO_0000003" => Some(Self::Occurrent),
            "http://purl.obolibrary.org/obo/BFO_0000004" => Some(Self::IndependentContinuant),
            "http://purl.obolibrary.org/obo/BFO_0000015" => Some(Self::Process),
            "http://purl.obolibrary.org/obo/BFO_0000017" => Some(Self::RealizableEntity),
            "http://purl.obolibrary.org/obo/BFO_0000023" => Some(Self::Role),
            "http://purl.obolibrary.org/obo/BFO_0000034" => Some(Self::Function),
            "http://purl.obolibrary.org/obo/BFO_0000040" => Some(Self::MaterialEntity),
            "http://purl.obolibrary.org/obo/BFO_0000008" => Some(Self::TemporalRegion),
            "http://purl.obolibrary.org/obo/BFO_0000038" => Some(Self::TemporalPart),
            _ => None,
        }
    }
}

/// Trait for entities with BFO classification
pub trait BfoAligned {
    /// Get BFO type of this entity
    fn bfo_type(&self) -> BfoEntityType;

    /// Get BFO URI
    fn bfo_uri(&self) -> &'static str {
        self.bfo_type().uri()
    }

    /// Get human-readable BFO label
    fn bfo_label(&self) -> &'static str {
        self.bfo_type().label()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bfo_uri_mapping() {
        assert_eq!(
            BfoEntityType::MaterialEntity.uri(),
            "http://purl.obolibrary.org/obo/BFO_0000040"
        );
        assert_eq!(
            BfoEntityType::Process.uri(),
            "http://purl.obolibrary.org/obo/BFO_0000015"
        );
    }

    #[test]
    fn test_bfo_roundtrip() {
        let types = [
            BfoEntityType::MaterialEntity,
            BfoEntityType::Process,
            BfoEntityType::Role,
            BfoEntityType::Function,
        ];

        for bfo_type in &types {
            let uri = bfo_type.uri();
            let parsed = BfoEntityType::from_uri(uri);
            assert_eq!(parsed, Some(*bfo_type));
        }
    }

    #[test]
    fn test_bfo_labels() {
        assert_eq!(BfoEntityType::MaterialEntity.label(), "Material Entity");
        assert_eq!(BfoEntityType::Process.label(), "Process");
        assert_eq!(BfoEntityType::Role.label(), "Role");
    }

    #[test]
    fn test_bfo_from_uri_invalid() {
        assert_eq!(BfoEntityType::from_uri("invalid"), None);
        assert_eq!(BfoEntityType::from_uri(""), None);
        assert_eq!(BfoEntityType::from_uri("http://example.com"), None);
    }
}
