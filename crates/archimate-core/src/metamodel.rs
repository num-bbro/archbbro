//! The ArchiMate 3.2 concept catalogue: element types, relationship types and layers.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Layer {
    Strategy,
    Business,
    Application,
    Technology,
    Physical,
    Motivation,
    Implementation,
    Other,
}

impl Layer {
    pub const ALL: [Layer; 8] = [
        Layer::Strategy,
        Layer::Business,
        Layer::Application,
        Layer::Technology,
        Layer::Physical,
        Layer::Motivation,
        Layer::Implementation,
        Layer::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Layer::Strategy => "Strategy",
            Layer::Business => "Business",
            Layer::Application => "Application",
            Layer::Technology => "Technology",
            Layer::Physical => "Physical",
            Layer::Motivation => "Motivation",
            Layer::Implementation => "Implementation & Migration",
            Layer::Other => "Other",
        }
    }

    /// Archi's default fill colour for elements of this layer, as RGB.
    pub fn default_color(self) -> [u8; 3] {
        match self {
            Layer::Strategy => [0xf5, 0xde, 0xaa],
            Layer::Business => [0xff, 0xff, 0xb5],
            Layer::Application => [0xb5, 0xff, 0xff],
            Layer::Technology | Layer::Physical => [0xc9, 0xe7, 0xb7],
            Layer::Motivation => [0xcc, 0xcc, 0xff],
            Layer::Implementation => [0xff, 0xe0, 0xe0],
            Layer::Other => [0xff, 0xff, 0xff],
        }
    }

    /// The top-level folder an element of this layer belongs to.
    pub fn folder(self) -> crate::model::FolderKind {
        use crate::model::FolderKind as F;
        match self {
            Layer::Strategy => F::Strategy,
            Layer::Business => F::Business,
            Layer::Application => F::Application,
            Layer::Technology | Layer::Physical => F::Technology,
            Layer::Motivation => F::Motivation,
            Layer::Implementation => F::ImplementationMigration,
            Layer::Other => F::Other,
        }
    }
}

macro_rules! element_types {
    ($($variant:ident => $layer:ident, $label:literal;)*) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum ElementType { $($variant),* }

        impl ElementType {
            pub const ALL: &'static [ElementType] = &[$(ElementType::$variant),*];

            /// Name used in Archi's `xsi:type="archimate:<name>"` and in relationships.xml.
            pub fn name(self) -> &'static str {
                match self { $(ElementType::$variant => stringify!($variant)),* }
            }

            pub fn label(self) -> &'static str {
                match self { $(ElementType::$variant => $label),* }
            }

            pub fn layer(self) -> Layer {
                match self { $(ElementType::$variant => Layer::$layer),* }
            }

            pub fn from_name(name: &str) -> Option<ElementType> {
                match name { $(stringify!($variant) => Some(ElementType::$variant),)* _ => None }
            }
        }
    };
}

element_types! {
    Resource => Strategy, "Resource";
    Capability => Strategy, "Capability";
    ValueStream => Strategy, "Value Stream";
    CourseOfAction => Strategy, "Course of Action";

    BusinessActor => Business, "Business Actor";
    BusinessRole => Business, "Business Role";
    BusinessCollaboration => Business, "Business Collaboration";
    BusinessInterface => Business, "Business Interface";
    BusinessProcess => Business, "Business Process";
    BusinessFunction => Business, "Business Function";
    BusinessInteraction => Business, "Business Interaction";
    BusinessEvent => Business, "Business Event";
    BusinessService => Business, "Business Service";
    BusinessObject => Business, "Business Object";
    Contract => Business, "Contract";
    Representation => Business, "Representation";
    Product => Business, "Product";

    ApplicationComponent => Application, "Application Component";
    ApplicationCollaboration => Application, "Application Collaboration";
    ApplicationInterface => Application, "Application Interface";
    ApplicationFunction => Application, "Application Function";
    ApplicationInteraction => Application, "Application Interaction";
    ApplicationProcess => Application, "Application Process";
    ApplicationEvent => Application, "Application Event";
    ApplicationService => Application, "Application Service";
    DataObject => Application, "Data Object";

    Node => Technology, "Node";
    Device => Technology, "Device";
    SystemSoftware => Technology, "System Software";
    TechnologyCollaboration => Technology, "Technology Collaboration";
    TechnologyInterface => Technology, "Technology Interface";
    Path => Technology, "Path";
    CommunicationNetwork => Technology, "Communication Network";
    TechnologyFunction => Technology, "Technology Function";
    TechnologyProcess => Technology, "Technology Process";
    TechnologyInteraction => Technology, "Technology Interaction";
    TechnologyEvent => Technology, "Technology Event";
    TechnologyService => Technology, "Technology Service";
    Artifact => Technology, "Artifact";

    Equipment => Physical, "Equipment";
    Facility => Physical, "Facility";
    DistributionNetwork => Physical, "Distribution Network";
    Material => Physical, "Material";

    Stakeholder => Motivation, "Stakeholder";
    Driver => Motivation, "Driver";
    Assessment => Motivation, "Assessment";
    Goal => Motivation, "Goal";
    Outcome => Motivation, "Outcome";
    Principle => Motivation, "Principle";
    Requirement => Motivation, "Requirement";
    Constraint => Motivation, "Constraint";
    Meaning => Motivation, "Meaning";
    Value => Motivation, "Value";

    WorkPackage => Implementation, "Work Package";
    Deliverable => Implementation, "Deliverable";
    ImplementationEvent => Implementation, "Implementation Event";
    Plateau => Implementation, "Plateau";
    Gap => Implementation, "Gap";

    Location => Other, "Location";
    Grouping => Other, "Grouping";
    Junction => Other, "Junction";
}

impl ElementType {
    pub fn default_color(self) -> [u8; 3] {
        match self {
            ElementType::Plateau | ElementType::Gap => [0xe0, 0xff, 0xe0],
            ElementType::Location => [0xfb, 0xb8, 0x75],
            ElementType::Grouping => [0xd2, 0xd7, 0xd7],
            ElementType::Junction => [0x00, 0x00, 0x00],
            _ => self.layer().default_color(),
        }
    }

    pub fn in_layer(layer: Layer) -> impl Iterator<Item = ElementType> {
        ElementType::ALL
            .iter()
            .copied()
            .filter(move |t| t.layer() == layer)
    }
}

impl fmt::Display for ElementType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RelationshipType {
    Composition,
    Aggregation,
    Assignment,
    Realization,
    Serving,
    Access,
    Influence,
    Triggering,
    Flow,
    Specialization,
    Association,
}

impl RelationshipType {
    pub const ALL: [RelationshipType; 11] = [
        RelationshipType::Composition,
        RelationshipType::Aggregation,
        RelationshipType::Assignment,
        RelationshipType::Realization,
        RelationshipType::Serving,
        RelationshipType::Access,
        RelationshipType::Influence,
        RelationshipType::Triggering,
        RelationshipType::Flow,
        RelationshipType::Specialization,
        RelationshipType::Association,
    ];

    /// Name used in Archi's `xsi:type`, e.g. `ServingRelationship`.
    pub fn name(self) -> &'static str {
        match self {
            RelationshipType::Composition => "CompositionRelationship",
            RelationshipType::Aggregation => "AggregationRelationship",
            RelationshipType::Assignment => "AssignmentRelationship",
            RelationshipType::Realization => "RealizationRelationship",
            RelationshipType::Serving => "ServingRelationship",
            RelationshipType::Access => "AccessRelationship",
            RelationshipType::Influence => "InfluenceRelationship",
            RelationshipType::Triggering => "TriggeringRelationship",
            RelationshipType::Flow => "FlowRelationship",
            RelationshipType::Specialization => "SpecializationRelationship",
            RelationshipType::Association => "AssociationRelationship",
        }
    }

    pub fn label(self) -> &'static str {
        self.name().trim_end_matches("Relationship")
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|r| r.name() == name)
    }

    /// The single-letter key Archi's relationships.xml uses for this type.
    pub fn key(self) -> char {
        match self {
            RelationshipType::Access => 'a',
            RelationshipType::Composition => 'c',
            RelationshipType::Flow => 'f',
            RelationshipType::Aggregation => 'g',
            RelationshipType::Assignment => 'i',
            RelationshipType::Influence => 'n',
            RelationshipType::Association => 'o',
            RelationshipType::Realization => 'r',
            RelationshipType::Specialization => 's',
            RelationshipType::Triggering => 't',
            RelationshipType::Serving => 'v',
        }
    }

    pub fn from_key(c: char) -> Option<Self> {
        Self::ALL.iter().copied().find(|r| r.key() == c)
    }
}

impl fmt::Display for RelationshipType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_roundtrip() {
        assert_eq!(ElementType::ALL.len(), 61);
        for t in ElementType::ALL {
            assert_eq!(ElementType::from_name(t.name()), Some(*t));
        }
        for r in RelationshipType::ALL {
            assert_eq!(RelationshipType::from_name(r.name()), Some(r));
            assert_eq!(RelationshipType::from_key(r.key()), Some(r));
        }
    }
}
