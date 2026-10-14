//! In-memory dataset holding OpenStreetMap primitives, relational indices, and selection state.

use std::collections::{HashMap, HashSet};

use crate::geo::{BoundingBox, LatLon};
use crate::id::{NodeId, PrimitiveId, RelationId, WayId};
use crate::primitive::{Node, OsmPrimitive, Relation, Way};

/// The central in-memory dataset of an OSM edit layer.
/// Maintains primitives, back-references, selection state, and bounds.
#[derive(Debug, Clone, Default)]
pub struct DataSet {
    nodes: HashMap<NodeId, Node>,
    ways: HashMap<WayId, Way>,
    relations: HashMap<RelationId, Relation>,

    // Reverse indices for high-performance lookup
    node_to_ways: HashMap<NodeId, HashSet<WayId>>,
    primitive_to_relations: HashMap<PrimitiveId, HashSet<RelationId>>,

    // Current selection
    selection: HashSet<PrimitiveId>,

    // Counter for local negative IDs
    next_node_id: i64,
    next_way_id: i64,
    next_relation_id: i64,
}

impl DataSet {
    /// Creates a new empty `DataSet`.
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            ways: HashMap::new(),
            relations: HashMap::new(),
            node_to_ways: HashMap::new(),
            primitive_to_relations: HashMap::new(),
            selection: HashSet::new(),
            next_node_id: -1,
            next_way_id: -1,
            next_relation_id: -1,
        }
    }

    /// Allocates a new negative `NodeId` for a locally created node.
    pub fn allocate_node_id(&mut self) -> NodeId {
        let id = NodeId(self.next_node_id);
        self.next_node_id -= 1;
        id
    }

    /// Allocates a new negative `WayId` for a locally created way.
    pub fn allocate_way_id(&mut self) -> WayId {
        let id = WayId(self.next_way_id);
        self.next_way_id -= 1;
        id
    }

    /// Allocates a new negative `RelationId` for a locally created relation.
    pub fn allocate_relation_id(&mut self) -> RelationId {
        let id = RelationId(self.next_relation_id);
        self.next_relation_id -= 1;
        id
    }

    /// Total number of primitives in the dataset.
    pub fn primitive_count(&self) -> usize {
        self.nodes.len() + self.ways.len() + self.relations.len()
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn way_count(&self) -> usize {
        self.ways.len()
    }

    pub fn relation_count(&self) -> usize {
        self.relations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.ways.is_empty() && self.relations.is_empty()
    }

    // --- Node CRUD ---

    pub fn get_node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(&id)
    }

    pub fn get_node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(&id)
    }

    pub fn contains_node(&self, id: NodeId) -> bool {
        self.nodes.contains_key(&id)
    }

    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.values()
    }

    /// Inserts a node into the dataset. Updates local ID counter if negative.
    pub fn insert_node(&mut self, node: Node) -> Option<Node> {
        if node.id.0 <= self.next_node_id {
            self.next_node_id = node.id.0 - 1;
        }
        self.nodes.insert(node.id, node)
    }

    /// Removes a node from the dataset and cleans up reverse indices.
    pub fn remove_node(&mut self, id: NodeId) -> Option<Node> {
        self.selection.remove(&PrimitiveId::Node(id));
        self.node_to_ways.remove(&id);
        self.primitive_to_relations.remove(&PrimitiveId::Node(id));
        self.nodes.remove(&id)
    }

    // --- Way CRUD ---

    pub fn get_way(&self, id: WayId) -> Option<&Way> {
        self.ways.get(&id)
    }

    pub fn get_way_mut(&mut self, id: WayId) -> Option<&mut Way> {
        self.ways.get_mut(&id)
    }

    pub fn contains_way(&self, id: WayId) -> bool {
        self.ways.contains_key(&id)
    }

    pub fn ways(&self) -> impl Iterator<Item = &Way> {
        self.ways.values()
    }

    /// Inserts a way into the dataset and registers back-references.
    pub fn insert_way(&mut self, way: Way) -> Option<Way> {
        if way.id.0 <= self.next_way_id {
            self.next_way_id = way.id.0 - 1;
        }

        // Clean up previous references if replacing
        if let Some(old) = self.ways.get(&way.id) {
            for nid in &old.nodes {
                if let Some(set) = self.node_to_ways.get_mut(nid) {
                    set.remove(&way.id);
                }
            }
        }

        // Add new node -> way references
        for nid in &way.nodes {
            self.node_to_ways.entry(*nid).or_default().insert(way.id);
        }

        self.ways.insert(way.id, way)
    }

    /// Removes a way from the dataset and updates reverse indices.
    pub fn remove_way(&mut self, id: WayId) -> Option<Way> {
        self.selection.remove(&PrimitiveId::Way(id));
        self.primitive_to_relations.remove(&PrimitiveId::Way(id));
        let way = self.ways.remove(&id)?;
        for nid in &way.nodes {
            if let Some(set) = self.node_to_ways.get_mut(nid) {
                set.remove(&id);
                if set.is_empty() {
                    self.node_to_ways.remove(nid);
                }
            }
        }
        Some(way)
    }

    // --- Relation CRUD ---

    pub fn get_relation(&self, id: RelationId) -> Option<&Relation> {
        self.relations.get(&id)
    }

    pub fn get_relation_mut(&mut self, id: RelationId) -> Option<&mut Relation> {
        self.relations.get_mut(&id)
    }

    pub fn contains_relation(&self, id: RelationId) -> bool {
        self.relations.contains_key(&id)
    }

    pub fn relations(&self) -> impl Iterator<Item = &Relation> {
        self.relations.values()
    }

    /// Inserts a relation and indexes member relationships.
    pub fn insert_relation(&mut self, relation: Relation) -> Option<Relation> {
        if relation.id.0 <= self.next_relation_id {
            self.next_relation_id = relation.id.0 - 1;
        }

        if let Some(old) = self.relations.get(&relation.id) {
            for m in &old.members {
                if let Some(set) = self.primitive_to_relations.get_mut(&m.member) {
                    set.remove(&relation.id);
                }
            }
        }

        for m in &relation.members {
            self.primitive_to_relations
                .entry(m.member)
                .or_default()
                .insert(relation.id);
        }

        self.relations.insert(relation.id, relation)
    }

    /// Removes a relation and cleans up reverse indices.
    pub fn remove_relation(&mut self, id: RelationId) -> Option<Relation> {
        self.selection.remove(&PrimitiveId::Relation(id));
        let relation = self.relations.remove(&id)?;
        for m in &relation.members {
            if let Some(set) = self.primitive_to_relations.get_mut(&m.member) {
                set.remove(&id);
                if set.is_empty() {
                    self.primitive_to_relations.remove(&m.member);
                }
            }
        }
        Some(relation)
    }

    // --- Polymorphic Primitive Operations ---

    pub fn get_primitive(&self, id: PrimitiveId) -> Option<OsmPrimitive> {
        match id {
            PrimitiveId::Node(nid) => self.get_node(nid).cloned().map(OsmPrimitive::Node),
            PrimitiveId::Way(wid) => self.get_way(wid).cloned().map(OsmPrimitive::Way),
            PrimitiveId::Relation(rid) => {
                self.get_relation(rid).cloned().map(OsmPrimitive::Relation)
            }
        }
    }

    pub fn insert_primitive(&mut self, prim: OsmPrimitive) {
        match prim {
            OsmPrimitive::Node(n) => {
                self.insert_node(n);
            }
            OsmPrimitive::Way(w) => {
                self.insert_way(w);
            }
            OsmPrimitive::Relation(r) => {
                self.insert_relation(r);
            }
        }
    }

    pub fn remove_primitive(&mut self, id: PrimitiveId) -> Option<OsmPrimitive> {
        match id {
            PrimitiveId::Node(nid) => self.remove_node(nid).map(OsmPrimitive::Node),
            PrimitiveId::Way(wid) => self.remove_way(wid).map(OsmPrimitive::Way),
            PrimitiveId::Relation(rid) => self.remove_relation(rid).map(OsmPrimitive::Relation),
        }
    }

    // --- Spatial & Relational Queries ---

    /// Computes the bounding box enclosing all nodes in the dataset.
    pub fn bounds(&self) -> Option<BoundingBox> {
        BoundingBox::from_points(self.nodes.values().map(|n| n.position))
    }

    /// Returns all nodes contained within a given bounding box.
    pub fn nodes_in_bbox(&self, bbox: &BoundingBox) -> Vec<&Node> {
        self.nodes
            .values()
            .filter(|n| bbox.contains_point(n.position))
            .collect()
    }

    /// Returns all ways that have at least one node within the given bounding box.
    pub fn ways_in_bbox(&self, bbox: &BoundingBox) -> Vec<&Way> {
        self.ways
            .values()
            .filter(|w| {
                w.nodes.iter().any(|nid| {
                    self.nodes
                        .get(nid)
                        .map(|n| bbox.contains_point(n.position))
                        .unwrap_or(false)
                })
            })
            .collect()
    }

    /// Finds ways referencing the given node ID.
    pub fn ways_using_node(&self, id: NodeId) -> Vec<&Way> {
        match self.node_to_ways.get(&id) {
            Some(way_ids) => way_ids
                .iter()
                .filter_map(|wid| self.ways.get(wid))
                .collect(),
            None => Vec::new(),
        }
    }

    /// Finds relations referencing the given primitive ID.
    pub fn relations_using(&self, id: PrimitiveId) -> Vec<&Relation> {
        match self.primitive_to_relations.get(&id) {
            Some(rel_ids) => rel_ids
                .iter()
                .filter_map(|rid| self.relations.get(rid))
                .collect(),
            None => Vec::new(),
        }
    }

    /// Resolves the sequence of geographic positions for all nodes in a way.
    /// If any node in the way is missing, returns None for that coordinate or filters it.
    pub fn node_positions_for_way(&self, way_id: WayId) -> Option<Vec<LatLon>> {
        let way = self.get_way(way_id)?;
        let mut positions = Vec::with_capacity(way.nodes.len());
        for nid in &way.nodes {
            if let Some(node) = self.get_node(*nid) {
                positions.push(node.position);
            }
        }
        Some(positions)
    }

    /// Finds the nearest node to a given coordinate within a maximum distance in meters.
    pub fn nearest_node(&self, pos: LatLon, max_dist_meters: f64) -> Option<(NodeId, f64)> {
        let mut best: Option<(NodeId, f64)> = None;

        for node in self.nodes.values() {
            let dist = pos.distance_to(&node.position);
            if dist <= max_dist_meters {
                match best {
                    None => best = Some((node.id, dist)),
                    Some((_, current_min)) if dist < current_min => {
                        best = Some((node.id, dist));
                    }
                    _ => {}
                }
            }
        }

        best
    }

    // --- Selection Management ---

    pub fn selection(&self) -> &HashSet<PrimitiveId> {
        &self.selection
    }

    pub fn is_selected(&self, id: PrimitiveId) -> bool {
        self.selection.contains(&id)
    }

    pub fn select(&mut self, id: PrimitiveId) {
        self.selection.insert(id);
    }

    pub fn deselect(&mut self, id: PrimitiveId) {
        self.selection.remove(&id);
    }

    pub fn toggle_select(&mut self, id: PrimitiveId) {
        if !self.selection.remove(&id) {
            self.selection.insert(id);
        }
    }

    pub fn clear_selection(&mut self) {
        self.selection.clear();
    }

    pub fn set_selection(&mut self, selection: HashSet<PrimitiveId>) {
        self.selection = selection;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dataset_basic_crud_and_indexing() {
        let mut ds = DataSet::new();

        let n1 = Node::new(NodeId(1), LatLon::new(52.0, 13.0));
        let n2 = Node::new(NodeId(2), LatLon::new(52.1, 13.1));
        let n3 = Node::new(NodeId(3), LatLon::new(52.2, 13.2));

        ds.insert_node(n1);
        ds.insert_node(n2);
        ds.insert_node(n3);

        let w1 = Way::new(WayId(10), vec![NodeId(1), NodeId(2)]);
        ds.insert_way(w1);

        assert_eq!(ds.node_count(), 3);
        assert_eq!(ds.way_count(), 1);

        // Reverse index checks
        let ways_using_n1 = ds.ways_using_node(NodeId(1));
        assert_eq!(ways_using_n1.len(), 1);
        assert_eq!(ways_using_n1[0].id, WayId(10));

        let ways_using_n3 = ds.ways_using_node(NodeId(3));
        assert!(ways_using_n3.is_empty());

        // Nearest node search
        let near = ds.nearest_node(LatLon::new(52.0001, 13.0001), 1000.0);
        assert!(near.is_some());
        assert_eq!(near.unwrap().0, NodeId(1));

        // Removing way clears back-references
        ds.remove_way(WayId(10));
        assert!(ds.ways_using_node(NodeId(1)).is_empty());
    }

    #[test]
    fn test_local_id_allocation() {
        let mut ds = DataSet::new();
        let id1 = ds.allocate_node_id();
        let id2 = ds.allocate_node_id();
        assert_eq!(id1, NodeId(-1));
        assert_eq!(id2, NodeId(-2));
        assert!(id1.is_new());

        let wid = ds.allocate_way_id();
        assert_eq!(wid, WayId(-1));
    }

    #[test]
    fn test_selection() {
        let mut ds = DataSet::new();
        let nid = PrimitiveId::Node(NodeId(1));

        ds.select(nid);
        assert!(ds.is_selected(nid));

        ds.toggle_select(nid);
        assert!(!ds.is_selected(nid));

        ds.toggle_select(nid);
        assert!(ds.is_selected(nid));

        ds.clear_selection();
        assert!(ds.selection().is_empty());
    }

    #[test]
    fn test_bbox_queries() {
        let mut ds = DataSet::new();
        ds.insert_node(Node::new(1, LatLon::new(10.0, 10.0)));
        ds.insert_node(Node::new(2, LatLon::new(20.0, 20.0)));
        ds.insert_node(Node::new(3, LatLon::new(50.0, 50.0)));

        let query_box = BoundingBox::new(5.0, 5.0, 25.0, 25.0);
        let results = ds.nodes_in_bbox(&query_box);
        assert_eq!(results.len(), 2);
    }
}
