//! Identifiers for OpenStreetMap primitives.

use std::fmt;

/// Unique identifier for an OSM Node.
/// Positive IDs (> 0) represent entities on the OSM server.
/// Negative IDs (< 0) represent newly created uncommitted local entities (JOSM convention).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NodeId(pub i64);

impl NodeId {
    /// True if this node was created locally and not yet committed to the server.
    #[inline]
    pub fn is_new(&self) -> bool {
        self.0 < 0
    }

    /// True if this node has a server-assigned ID.
    #[inline]
    pub fn is_server(&self) -> bool {
        self.0 > 0
    }

    #[inline]
    pub fn as_i64(&self) -> i64 {
        self.0
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "n{}", self.0)
    }
}

impl From<i64> for NodeId {
    fn from(id: i64) -> Self {
        Self(id)
    }
}

/// Unique identifier for an OSM Way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WayId(pub i64);

impl WayId {
    #[inline]
    pub fn is_new(&self) -> bool {
        self.0 < 0
    }

    #[inline]
    pub fn is_server(&self) -> bool {
        self.0 > 0
    }

    #[inline]
    pub fn as_i64(&self) -> i64 {
        self.0
    }
}

impl fmt::Display for WayId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "w{}", self.0)
    }
}

impl From<i64> for WayId {
    fn from(id: i64) -> Self {
        Self(id)
    }
}

/// Unique identifier for an OSM Relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RelationId(pub i64);

impl RelationId {
    #[inline]
    pub fn is_new(&self) -> bool {
        self.0 < 0
    }

    #[inline]
    pub fn is_server(&self) -> bool {
        self.0 > 0
    }

    #[inline]
    pub fn as_i64(&self) -> i64 {
        self.0
    }
}

impl fmt::Display for RelationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "r{}", self.0)
    }
}

impl From<i64> for RelationId {
    fn from(id: i64) -> Self {
        Self(id)
    }
}

/// A polymorphic identifier for any OSM primitive (Node, Way, or Relation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PrimitiveId {
    Node(NodeId),
    Way(WayId),
    Relation(RelationId),
}

impl PrimitiveId {
    #[inline]
    pub fn is_new(&self) -> bool {
        match self {
            PrimitiveId::Node(id) => id.is_new(),
            PrimitiveId::Way(id) => id.is_new(),
            PrimitiveId::Relation(id) => id.is_new(),
        }
    }

    #[inline]
    pub fn raw_id(&self) -> i64 {
        match self {
            PrimitiveId::Node(id) => id.0,
            PrimitiveId::Way(id) => id.0,
            PrimitiveId::Relation(id) => id.0,
        }
    }

    #[inline]
    pub fn type_name(&self) -> &'static str {
        match self {
            PrimitiveId::Node(_) => "node",
            PrimitiveId::Way(_) => "way",
            PrimitiveId::Relation(_) => "relation",
        }
    }
}

impl fmt::Display for PrimitiveId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrimitiveId::Node(id) => write!(f, "{id}"),
            PrimitiveId::Way(id) => write!(f, "{id}"),
            PrimitiveId::Relation(id) => write!(f, "{id}"),
        }
    }
}

impl From<NodeId> for PrimitiveId {
    fn from(id: NodeId) -> Self {
        PrimitiveId::Node(id)
    }
}

impl From<WayId> for PrimitiveId {
    fn from(id: WayId) -> Self {
        PrimitiveId::Way(id)
    }
}

impl From<RelationId> for PrimitiveId {
    fn from(id: RelationId) -> Self {
        PrimitiveId::Relation(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_id_types() {
        let node_id = NodeId(-5);
        assert!(node_id.is_new());
        assert!(!node_id.is_server());
        assert_eq!(node_id.to_string(), "n-5");

        let way_id = WayId(123456);
        assert!(!way_id.is_new());
        assert!(way_id.is_server());
        assert_eq!(way_id.to_string(), "w123456");

        let rel_id = RelationId(789);
        assert_eq!(rel_id.to_string(), "r789");

        let prim: PrimitiveId = way_id.into();
        assert_eq!(prim.type_name(), "way");
        assert_eq!(prim.raw_id(), 123456);
        assert_eq!(prim.to_string(), "w123456");
    }
}
