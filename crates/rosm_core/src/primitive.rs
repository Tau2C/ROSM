//! OpenStreetMap core primitives: Node, Way, Relation and associated metadata.

use crate::geo::LatLon;
use crate::id::{NodeId, PrimitiveId, RelationId, WayId};
use crate::tags::Tags;

/// Metadata associated with an OSM primitive.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OsmMetadata {
    pub version: Option<i32>,
    pub changeset: Option<i64>,
    pub timestamp: Option<String>,
    pub user: Option<String>,
    pub uid: Option<i64>,
    pub visible: bool,
}

impl OsmMetadata {
    pub fn new() -> Self {
        Self {
            version: None,
            changeset: None,
            timestamp: None,
            user: None,
            uid: None,
            visible: true,
        }
    }
}

/// An OSM Node represents a single geographical point with latitude, longitude, and tags.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Node {
    pub id: NodeId,
    pub position: LatLon,
    pub tags: Tags,
    pub metadata: OsmMetadata,
}

impl Node {
    pub fn new(id: impl Into<NodeId>, position: LatLon) -> Self {
        Self {
            id: id.into(),
            position,
            tags: Tags::new(),
            metadata: OsmMetadata::new(),
        }
    }

    pub fn with_tags(mut self, tags: Tags) -> Self {
        self.tags = tags;
        self
    }

    pub fn with_metadata(mut self, metadata: OsmMetadata) -> Self {
        self.metadata = metadata;
        self
    }
}

/// An OSM Way is an ordered list of nodes, forming an open linestring or a closed polygon/ring.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Way {
    pub id: WayId,
    pub nodes: Vec<NodeId>,
    pub tags: Tags,
    pub metadata: OsmMetadata,
}

impl Way {
    pub fn new(id: impl Into<WayId>, nodes: Vec<NodeId>) -> Self {
        Self {
            id: id.into(),
            nodes,
            tags: Tags::new(),
            metadata: OsmMetadata::new(),
        }
    }

    pub fn with_tags(mut self, tags: Tags) -> Self {
        self.tags = tags;
        self
    }

    pub fn with_metadata(mut self, metadata: OsmMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Number of node references in this way.
    #[inline]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Checks if this way is closed (at least 3 nodes, and first node equals last node).
    pub fn is_closed(&self) -> bool {
        self.nodes.len() >= 3 && self.nodes.first() == self.nodes.last()
    }

    /// Determines if this way should be treated as an area / polygon based on tags and closure.
    pub fn is_area(&self) -> bool {
        self.is_closed() && self.tags.is_area()
    }

    /// First node ID if non-empty.
    #[inline]
    pub fn first_node(&self) -> Option<NodeId> {
        self.nodes.first().copied()
    }

    /// Last node ID if non-empty.
    #[inline]
    pub fn last_node(&self) -> Option<NodeId> {
        self.nodes.last().copied()
    }

    /// Checks if this way references the given node ID.
    #[inline]
    pub fn contains_node(&self, node_id: NodeId) -> bool {
        self.nodes.contains(&node_id)
    }

    /// Reverses node order of the way.
    pub fn reverse(&mut self) {
        self.nodes.reverse();
    }
}

/// A member of an OSM Relation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RelationMember {
    pub member: PrimitiveId,
    pub role: String,
}

impl RelationMember {
    pub fn new(member: impl Into<PrimitiveId>, role: impl Into<String>) -> Self {
        Self {
            member: member.into(),
            role: role.into(),
        }
    }
}

/// An OSM Relation represents relationships between members (nodes, ways, and other relations).
/// Used for multipolygons, bus routes, turn restrictions, boundaries, etc.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Relation {
    pub id: RelationId,
    pub members: Vec<RelationMember>,
    pub tags: Tags,
    pub metadata: OsmMetadata,
}

impl Relation {
    pub fn new(id: impl Into<RelationId>, members: Vec<RelationMember>) -> Self {
        Self {
            id: id.into(),
            members,
            tags: Tags::new(),
            metadata: OsmMetadata::new(),
        }
    }

    pub fn with_tags(mut self, tags: Tags) -> Self {
        self.tags = tags;
        self
    }

    pub fn with_metadata(mut self, metadata: OsmMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.members.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    pub fn contains_member(&self, id: PrimitiveId) -> bool {
        self.members.iter().any(|m| m.member == id)
    }

    pub fn is_multipolygon(&self) -> bool {
        self.tags.has_value("type", "multipolygon")
    }

    pub fn is_boundary(&self) -> bool {
        self.tags.has_value("type", "boundary")
    }
}

/// Unified enumeration over all OSM primitive types.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum OsmPrimitive {
    Node(Node),
    Way(Way),
    Relation(Relation),
}

impl OsmPrimitive {
    pub fn id(&self) -> PrimitiveId {
        match self {
            OsmPrimitive::Node(n) => n.id.into(),
            OsmPrimitive::Way(w) => w.id.into(),
            OsmPrimitive::Relation(r) => r.id.into(),
        }
    }

    pub fn tags(&self) -> &Tags {
        match self {
            OsmPrimitive::Node(n) => &n.tags,
            OsmPrimitive::Way(w) => &w.tags,
            OsmPrimitive::Relation(r) => &r.tags,
        }
    }

    pub fn tags_mut(&mut self) -> &mut Tags {
        match self {
            OsmPrimitive::Node(n) => &mut n.tags,
            OsmPrimitive::Way(w) => &mut w.tags,
            OsmPrimitive::Relation(r) => &mut r.tags,
        }
    }

    pub fn metadata(&self) -> &OsmMetadata {
        match self {
            OsmPrimitive::Node(n) => &n.metadata,
            OsmPrimitive::Way(w) => &w.metadata,
            OsmPrimitive::Relation(r) => &r.metadata,
        }
    }

    pub fn metadata_mut(&mut self) -> &mut OsmMetadata {
        match self {
            OsmPrimitive::Node(n) => &mut n.metadata,
            OsmPrimitive::Way(w) => &mut w.metadata,
            OsmPrimitive::Relation(r) => &mut r.metadata,
        }
    }
}

impl From<Node> for OsmPrimitive {
    fn from(n: Node) -> Self {
        OsmPrimitive::Node(n)
    }
}

impl From<Way> for OsmPrimitive {
    fn from(w: Way) -> Self {
        OsmPrimitive::Way(w)
    }
}

impl From<Relation> for OsmPrimitive {
    fn from(r: Relation) -> Self {
        OsmPrimitive::Relation(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_creation() {
        let n = Node::new(101, LatLon::new(52.0, 13.0));
        assert_eq!(n.id, NodeId(101));
        assert_eq!(n.position.lat(), 52.0);
        assert!(n.tags.is_empty());
    }

    #[test]
    fn test_way_closure_and_area() {
        let mut way = Way::new(
            WayId(1),
            vec![NodeId(10), NodeId(11), NodeId(12), NodeId(10)],
        );
        assert!(way.is_closed());
        assert!(!way.is_area());

        way.tags.insert("building", "yes");
        assert!(way.is_area());

        way.reverse();
        assert_eq!(
            way.nodes,
            vec![NodeId(10), NodeId(12), NodeId(11), NodeId(10)]
        );
    }

    #[test]
    fn test_relation() {
        let rel = Relation::new(
            RelationId(1),
            vec![
                RelationMember::new(WayId(1), "outer"),
                RelationMember::new(WayId(2), "inner"),
            ],
        );
        assert_eq!(rel.len(), 2);
        assert!(rel.contains_member(PrimitiveId::Way(WayId(1))));
        assert!(!rel.contains_member(PrimitiveId::Node(NodeId(10))));
    }
}
