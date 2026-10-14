//! Command pattern implementation for reversible map editing operations (Undo / Redo).

use std::fmt;
use thiserror::Error;

use crate::dataset::DataSet;
use crate::geo::LatLon;
use crate::id::{NodeId, PrimitiveId, WayId};
use crate::primitive::{OsmPrimitive, Way};
use crate::tags::Tags;

/// Errors that can occur when executing or undoing commands.
#[derive(Debug, Error, PartialEq)]
pub enum CommandError {
    #[error("Primitive not found: {0}")]
    PrimitiveNotFound(PrimitiveId),

    #[error("Node not found: {0}")]
    NodeNotFound(NodeId),

    #[error("Way not found: {0}")]
    WayNotFound(WayId),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Nothing to undo")]
    NothingToUndo,

    #[error("Nothing to redo")]
    NothingToRedo,
}

/// A reversible editing operation on a `DataSet`.
pub trait Command: fmt::Debug {
    /// Executes the command, applying changes to the dataset.
    fn execute(&mut self, dataset: &mut DataSet) -> Result<(), CommandError>;

    /// Undoes the command, reverting changes made to the dataset.
    fn undo(&mut self, dataset: &mut DataSet) -> Result<(), CommandError>;

    /// Human-readable description of the operation (e.g. for menu items: "Undo Move Node").
    fn description(&self) -> &str;
}

// --- Add Primitive Command ---

#[derive(Debug)]
pub struct AddPrimitiveCommand {
    primitive: OsmPrimitive,
    description: String,
}

impl AddPrimitiveCommand {
    pub fn new(primitive: OsmPrimitive) -> Self {
        let desc = format!("Add {}", primitive.id());
        Self {
            primitive,
            description: desc,
        }
    }
}

impl Command for AddPrimitiveCommand {
    fn execute(&mut self, dataset: &mut DataSet) -> Result<(), CommandError> {
        dataset.insert_primitive(self.primitive.clone());
        Ok(())
    }

    fn undo(&mut self, dataset: &mut DataSet) -> Result<(), CommandError> {
        dataset.remove_primitive(self.primitive.id());
        Ok(())
    }

    fn description(&self) -> &str {
        &self.description
    }
}

// --- Delete Primitive Command ---

#[derive(Debug)]
pub struct DeletePrimitiveCommand {
    id: PrimitiveId,
    deleted_primitive: Option<OsmPrimitive>,
    description: String,
}

impl DeletePrimitiveCommand {
    pub fn new(id: PrimitiveId) -> Self {
        Self {
            id,
            deleted_primitive: None,
            description: format!("Delete {id}"),
        }
    }
}

impl Command for DeletePrimitiveCommand {
    fn execute(&mut self, dataset: &mut DataSet) -> Result<(), CommandError> {
        let prim = dataset
            .remove_primitive(self.id)
            .ok_or(CommandError::PrimitiveNotFound(self.id))?;
        self.deleted_primitive = Some(prim);
        Ok(())
    }

    fn undo(&mut self, dataset: &mut DataSet) -> Result<(), CommandError> {
        let prim = self
            .deleted_primitive
            .take()
            .ok_or_else(|| CommandError::InvalidOperation("no deleted primitive stored".into()))?;
        dataset.insert_primitive(prim);
        Ok(())
    }

    fn description(&self) -> &str {
        &self.description
    }
}

// --- Move Node Command ---

#[derive(Debug)]
pub struct MoveNodeCommand {
    node_id: NodeId,
    old_position: Option<LatLon>,
    new_position: LatLon,
}

impl MoveNodeCommand {
    pub fn new(node_id: NodeId, new_position: LatLon) -> Self {
        Self {
            node_id,
            old_position: None,
            new_position,
        }
    }
}

impl Command for MoveNodeCommand {
    fn execute(&mut self, dataset: &mut DataSet) -> Result<(), CommandError> {
        let node = dataset
            .get_node_mut(self.node_id)
            .ok_or(CommandError::NodeNotFound(self.node_id))?;
        if self.old_position.is_none() {
            self.old_position = Some(node.position);
        }
        node.position = self.new_position;
        Ok(())
    }

    fn undo(&mut self, dataset: &mut DataSet) -> Result<(), CommandError> {
        let old_pos = self
            .old_position
            .ok_or_else(|| CommandError::InvalidOperation("no old position recorded".into()))?;
        let node = dataset
            .get_node_mut(self.node_id)
            .ok_or(CommandError::NodeNotFound(self.node_id))?;
        node.position = old_pos;
        Ok(())
    }

    fn description(&self) -> &str {
        "Move Node"
    }
}

// --- Change Tags Command ---

#[derive(Debug)]
pub struct ChangeTagsCommand {
    target: PrimitiveId,
    old_tags: Option<Tags>,
    new_tags: Tags,
}

impl ChangeTagsCommand {
    pub fn new(target: PrimitiveId, new_tags: Tags) -> Self {
        Self {
            target,
            old_tags: None,
            new_tags,
        }
    }
}

impl Command for ChangeTagsCommand {
    fn execute(&mut self, dataset: &mut DataSet) -> Result<(), CommandError> {
        match self.target {
            PrimitiveId::Node(id) => {
                let node = dataset
                    .get_node_mut(id)
                    .ok_or(CommandError::NodeNotFound(id))?;
                if self.old_tags.is_none() {
                    self.old_tags = Some(node.tags.clone());
                }
                node.tags = self.new_tags.clone();
            }
            PrimitiveId::Way(id) => {
                let way = dataset
                    .get_way_mut(id)
                    .ok_or(CommandError::WayNotFound(id))?;
                if self.old_tags.is_none() {
                    self.old_tags = Some(way.tags.clone());
                }
                way.tags = self.new_tags.clone();
            }
            PrimitiveId::Relation(id) => {
                let rel = dataset
                    .get_relation_mut(id)
                    .ok_or(CommandError::PrimitiveNotFound(self.target))?;
                if self.old_tags.is_none() {
                    self.old_tags = Some(rel.tags.clone());
                }
                rel.tags = self.new_tags.clone();
            }
        }
        Ok(())
    }

    fn undo(&mut self, dataset: &mut DataSet) -> Result<(), CommandError> {
        let old = self
            .old_tags
            .as_ref()
            .ok_or_else(|| CommandError::InvalidOperation("no previous tags".into()))?;
        match self.target {
            PrimitiveId::Node(id) => {
                let node = dataset
                    .get_node_mut(id)
                    .ok_or(CommandError::NodeNotFound(id))?;
                node.tags = old.clone();
            }
            PrimitiveId::Way(id) => {
                let way = dataset
                    .get_way_mut(id)
                    .ok_or(CommandError::WayNotFound(id))?;
                way.tags = old.clone();
            }
            PrimitiveId::Relation(id) => {
                let rel = dataset
                    .get_relation_mut(id)
                    .ok_or(CommandError::PrimitiveNotFound(self.target))?;
                rel.tags = old.clone();
            }
        }
        Ok(())
    }

    fn description(&self) -> &str {
        "Change Tags"
    }
}

// --- Reverse Way Command ---

#[derive(Debug)]
pub struct ReverseWayCommand {
    way_id: WayId,
}

impl ReverseWayCommand {
    pub fn new(way_id: WayId) -> Self {
        Self { way_id }
    }
}

impl Command for ReverseWayCommand {
    fn execute(&mut self, dataset: &mut DataSet) -> Result<(), CommandError> {
        let way = dataset
            .get_way_mut(self.way_id)
            .ok_or(CommandError::WayNotFound(self.way_id))?;
        way.reverse();
        Ok(())
    }

    fn undo(&mut self, dataset: &mut DataSet) -> Result<(), CommandError> {
        let way = dataset
            .get_way_mut(self.way_id)
            .ok_or(CommandError::WayNotFound(self.way_id))?;
        way.reverse();
        Ok(())
    }

    fn description(&self) -> &str {
        "Reverse Way Direction"
    }
}

// --- Split Way Command ---

#[derive(Debug)]
pub struct SplitWayCommand {
    original_way_id: WayId,
    split_node_id: NodeId,
    split_index: Option<usize>,
    created_way_id: Option<WayId>,
}

impl SplitWayCommand {
    pub fn new(original_way_id: WayId, split_node_id: NodeId) -> Self {
        Self {
            original_way_id,
            split_node_id,
            split_index: None,
            created_way_id: None,
        }
    }
}

impl Command for SplitWayCommand {
    fn execute(&mut self, dataset: &mut DataSet) -> Result<(), CommandError> {
        let way = dataset
            .get_way(self.original_way_id)
            .ok_or(CommandError::WayNotFound(self.original_way_id))?;

        // Find split index (must not be an endpoint)
        let idx = way
            .nodes
            .iter()
            .enumerate()
            .find(|(i, &nid)| nid == self.split_node_id && *i > 0 && *i < way.nodes.len() - 1)
            .map(|(i, _)| i)
            .ok_or_else(|| {
                CommandError::InvalidOperation(
                    "Split node must be an internal node of the way".into(),
                )
            })?;

        self.split_index = Some(idx);

        let second_half_nodes = way.nodes[idx..].to_vec();
        let first_half_nodes = way.nodes[..=idx].to_vec();
        let tags = way.tags.clone();

        let new_way_id = dataset.allocate_way_id();
        self.created_way_id = Some(new_way_id);

        let new_way = Way::new(new_way_id, second_half_nodes).with_tags(tags);

        // Update original way nodes
        let orig = dataset
            .get_way_mut(self.original_way_id)
            .ok_or(CommandError::WayNotFound(self.original_way_id))?;
        orig.nodes = first_half_nodes;

        // Insert new way
        dataset.insert_way(new_way);

        Ok(())
    }

    fn undo(&mut self, dataset: &mut DataSet) -> Result<(), CommandError> {
        let created_id = self
            .created_way_id
            .take()
            .ok_or_else(|| CommandError::InvalidOperation("no created way to undo".into()))?;

        let created_way = dataset
            .remove_way(created_id)
            .ok_or(CommandError::WayNotFound(created_id))?;

        let orig = dataset
            .get_way_mut(self.original_way_id)
            .ok_or(CommandError::WayNotFound(self.original_way_id))?;

        // Rejoin: orig nodes + created_way nodes starting after the shared split node
        orig.nodes.extend_from_slice(&created_way.nodes[1..]);

        Ok(())
    }

    fn description(&self) -> &str {
        "Split Way"
    }
}

// --- Modify Way Nodes Command ---

#[derive(Debug)]
pub struct ModifyWayNodesCommand {
    way_id: WayId,
    old_nodes: Option<Vec<NodeId>>,
    new_nodes: Vec<NodeId>,
}

impl ModifyWayNodesCommand {
    pub fn new(way_id: WayId, new_nodes: Vec<NodeId>) -> Self {
        Self {
            way_id,
            old_nodes: None,
            new_nodes,
        }
    }
}

impl Command for ModifyWayNodesCommand {
    fn execute(&mut self, dataset: &mut DataSet) -> Result<(), CommandError> {
        let way = dataset
            .get_way(self.way_id)
            .ok_or(CommandError::WayNotFound(self.way_id))?;
        if self.old_nodes.is_none() {
            self.old_nodes = Some(way.nodes.clone());
        }
        let mut updated = way.clone();
        updated.nodes = self.new_nodes.clone();
        dataset.insert_way(updated);
        Ok(())
    }

    fn undo(&mut self, dataset: &mut DataSet) -> Result<(), CommandError> {
        let old = self
            .old_nodes
            .as_ref()
            .ok_or_else(|| CommandError::InvalidOperation("no previous nodes".into()))?;
        let way = dataset
            .get_way(self.way_id)
            .ok_or(CommandError::WayNotFound(self.way_id))?;
        let mut reverted = way.clone();
        reverted.nodes = old.clone();
        dataset.insert_way(reverted);
        Ok(())
    }

    fn description(&self) -> &str {
        "Modify Way Nodes"
    }
}

// --- Combine Ways Command ---

#[derive(Debug)]
pub struct CombineWaysCommand {
    kept_way_id: WayId,
    removed_way_id: WayId,
    old_kept_nodes: Option<Vec<NodeId>>,
    removed_way: Option<Way>,
}

impl CombineWaysCommand {
    pub fn new(kept_way_id: WayId, removed_way_id: WayId) -> Self {
        Self {
            kept_way_id,
            removed_way_id,
            old_kept_nodes: None,
            removed_way: None,
        }
    }
}

impl Command for CombineWaysCommand {
    fn execute(&mut self, dataset: &mut DataSet) -> Result<(), CommandError> {
        let way1 = dataset
            .get_way(self.kept_way_id)
            .ok_or(CommandError::WayNotFound(self.kept_way_id))?
            .clone();
        let way2 = dataset
            .get_way(self.removed_way_id)
            .ok_or(CommandError::WayNotFound(self.removed_way_id))?
            .clone();

        let w1_first = way1.first_node().ok_or_else(|| CommandError::InvalidOperation("way1 is empty".into()))?;
        let w1_last = way1.last_node().ok_or_else(|| CommandError::InvalidOperation("way1 is empty".into()))?;
        let w2_first = way2.first_node().ok_or_else(|| CommandError::InvalidOperation("way2 is empty".into()))?;
        let w2_last = way2.last_node().ok_or_else(|| CommandError::InvalidOperation("way2 is empty".into()))?;

        let mut combined_nodes: Vec<NodeId>;

        if w1_last == w2_first {
            // [w1_0..shared] + [shared..w2_n]
            combined_nodes = way1.nodes.clone();
            combined_nodes.extend_from_slice(&way2.nodes[1..]);
        } else if w1_last == w2_last {
            // [w1_0..shared] + [shared..w2_0 reversed]
            combined_nodes = way1.nodes.clone();
            let mut rev = way2.nodes.clone();
            rev.reverse();
            combined_nodes.extend_from_slice(&rev[1..]);
        } else if w1_first == w2_last {
            // [w2_0..shared] + [shared..w1_n]
            combined_nodes = way2.nodes.clone();
            combined_nodes.extend_from_slice(&way1.nodes[1..]);
        } else if w1_first == w2_first {
            // [w2_n..shared reversed] + [shared..w1_n]
            let mut rev = way2.nodes.clone();
            rev.reverse();
            combined_nodes = rev;
            combined_nodes.extend_from_slice(&way1.nodes[1..]);
        } else {
            return Err(CommandError::InvalidOperation(
                "Ways do not share an endpoint".into(),
            ));
        }

        self.old_kept_nodes = Some(way1.nodes.clone());

        // Remove way2
        let removed = dataset
            .remove_way(self.removed_way_id)
            .ok_or(CommandError::WayNotFound(self.removed_way_id))?;
        self.removed_way = Some(removed);

        // Update way1 with combined nodes
        let mut updated_way1 = way1;
        updated_way1.nodes = combined_nodes;
        dataset.insert_way(updated_way1);

        Ok(())
    }

    fn undo(&mut self, dataset: &mut DataSet) -> Result<(), CommandError> {
        let old_nodes = self
            .old_kept_nodes
            .take()
            .ok_or_else(|| CommandError::InvalidOperation("no previous nodes stored".into()))?;
        let removed_way = self
            .removed_way
            .take()
            .ok_or_else(|| CommandError::InvalidOperation("no removed way stored".into()))?;

        // Restore way1 nodes
        let way1 = dataset
            .get_way(self.kept_way_id)
            .ok_or(CommandError::WayNotFound(self.kept_way_id))?;
        let mut restored_way1 = way1.clone();
        restored_way1.nodes = old_nodes;
        dataset.insert_way(restored_way1);

        // Re-insert way2
        dataset.insert_way(removed_way);

        Ok(())
    }

    fn description(&self) -> &str {
        "Combine Ways"
    }
}

// --- Merge Nodes Command ---

#[derive(Debug)]
pub struct MergeNodesCommand {
    target_node_id: NodeId,
    absorbed_node_id: NodeId,
    absorbed_node: Option<crate::primitive::Node>,
    old_target_tags: Option<Tags>,
    affected_ways: Vec<(WayId, Vec<NodeId>)>,
}

impl MergeNodesCommand {
    pub fn new(target_node_id: NodeId, absorbed_node_id: NodeId) -> Self {
        Self {
            target_node_id,
            absorbed_node_id,
            absorbed_node: None,
            old_target_tags: None,
            affected_ways: Vec::new(),
        }
    }
}

impl Command for MergeNodesCommand {
    fn execute(&mut self, dataset: &mut DataSet) -> Result<(), CommandError> {
        if self.target_node_id == self.absorbed_node_id {
            return Err(CommandError::InvalidOperation("Cannot merge node with itself".into()));
        }

        let absorbed = dataset
            .get_node(self.absorbed_node_id)
            .ok_or(CommandError::NodeNotFound(self.absorbed_node_id))?
            .clone();

        let target = dataset
            .get_node(self.target_node_id)
            .ok_or(CommandError::NodeNotFound(self.target_node_id))?
            .clone();

        self.old_target_tags = Some(target.tags.clone());

        // Merge tags: tags in absorbed that are absent in target are copied
        let mut new_tags = target.tags.clone();
        for (k, v) in absorbed.tags.iter() {
            if !new_tags.contains_key(k) {
                new_tags.insert(k, v);
            }
        }
        let mut updated_target = target;
        updated_target.tags = new_tags;
        dataset.insert_node(updated_target);

        // Find all ways referencing absorbed_node
        let referencing_ways = dataset.ways_using_node(self.absorbed_node_id);
        self.affected_ways.clear();

        let ways_to_update: Vec<(WayId, Vec<NodeId>)> = referencing_ways
            .iter()
            .map(|w| (w.id, w.nodes.clone()))
            .collect();

        for (wid, orig_nodes) in ways_to_update {
            self.affected_ways.push((wid, orig_nodes.clone()));
            let new_nodes: Vec<NodeId> = orig_nodes
                .into_iter()
                .map(|nid| if nid == self.absorbed_node_id { self.target_node_id } else { nid })
                .collect();
            let mut way = dataset.get_way(wid).unwrap().clone();
            way.nodes = new_nodes;
            dataset.insert_way(way);
        }

        // Remove absorbed node
        let deleted = dataset
            .remove_node(self.absorbed_node_id)
            .ok_or(CommandError::NodeNotFound(self.absorbed_node_id))?;
        self.absorbed_node = Some(deleted);

        Ok(())
    }

    fn undo(&mut self, dataset: &mut DataSet) -> Result<(), CommandError> {
        let absorbed = self
            .absorbed_node
            .take()
            .ok_or_else(|| CommandError::InvalidOperation("no absorbed node to restore".into()))?;
        let old_target_tags = self
            .old_target_tags
            .take()
            .ok_or_else(|| CommandError::InvalidOperation("no target tags to restore".into()))?;

        // Re-insert absorbed node
        dataset.insert_node(absorbed);

        // Restore target node tags
        let target = dataset
            .get_node(self.target_node_id)
            .ok_or(CommandError::NodeNotFound(self.target_node_id))?;
        let mut restored_target = target.clone();
        restored_target.tags = old_target_tags;
        dataset.insert_node(restored_target);

        // Restore affected ways
        for (wid, orig_nodes) in self.affected_ways.drain(..) {
            if let Some(way) = dataset.get_way(wid) {
                let mut restored_way = way.clone();
                restored_way.nodes = orig_nodes;
                dataset.insert_way(restored_way);
            }
        }

        Ok(())
    }

    fn description(&self) -> &str {
        "Merge Nodes"
    }
}

// --- Sequence Command (Composite) ---

#[derive(Debug, Default)]
pub struct SequenceCommand {
    commands: Vec<Box<dyn Command>>,
    description: String,
}

impl SequenceCommand {
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            commands: Vec::new(),
            description: description.into(),
        }
    }

    pub fn push(&mut self, cmd: Box<dyn Command>) {
        self.commands.push(cmd);
    }
}

impl Command for SequenceCommand {
    fn execute(&mut self, dataset: &mut DataSet) -> Result<(), CommandError> {
        for (i, cmd) in self.commands.iter_mut().enumerate() {
            if let Err(e) = cmd.execute(dataset) {
                // Roll back already executed commands in this sequence
                for rollback_cmd in self.commands[..i].iter_mut().rev() {
                    let _ = rollback_cmd.undo(dataset);
                }
                return Err(e);
            }
        }
        Ok(())
    }

    fn undo(&mut self, dataset: &mut DataSet) -> Result<(), CommandError> {
        for cmd in self.commands.iter_mut().rev() {
            cmd.undo(dataset)?;
        }
        Ok(())
    }

    fn description(&self) -> &str {
        &self.description
    }
}

// --- Undo / Redo Command Stack ---

#[derive(Default)]
pub struct CommandStack {
    undo_stack: Vec<Box<dyn Command>>,
    redo_stack: Vec<Box<dyn Command>>,
    save_index: Option<usize>,
}

impl fmt::Debug for CommandStack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommandStack")
            .field("undo_count", &self.undo_stack.len())
            .field("redo_count", &self.redo_stack.len())
            .field("save_index", &self.save_index)
            .finish()
    }
}

impl CommandStack {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            save_index: Some(0),
        }
    }

    /// Executes a command on the dataset and pushes it onto the undo stack.
    /// Clears the redo stack.
    pub fn execute(
        &mut self,
        mut cmd: Box<dyn Command>,
        dataset: &mut DataSet,
    ) -> Result<(), CommandError> {
        cmd.execute(dataset)?;
        self.undo_stack.push(cmd);
        self.redo_stack.clear();
        Ok(())
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo_description(&self) -> Option<&str> {
        self.undo_stack.last().map(|cmd| cmd.description())
    }

    pub fn redo_description(&self) -> Option<&str> {
        self.redo_stack.last().map(|cmd| cmd.description())
    }

    /// Undoes the last command and moves it to the redo stack.
    pub fn undo(&mut self, dataset: &mut DataSet) -> Result<String, CommandError> {
        let mut cmd = self.undo_stack.pop().ok_or(CommandError::NothingToUndo)?;
        cmd.undo(dataset)?;
        let desc = cmd.description().to_string();
        self.redo_stack.push(cmd);
        Ok(desc)
    }

    /// Redoes the most recently undone command.
    pub fn redo(&mut self, dataset: &mut DataSet) -> Result<String, CommandError> {
        let mut cmd = self.redo_stack.pop().ok_or(CommandError::NothingToRedo)?;
        cmd.execute(dataset)?;
        let desc = cmd.description().to_string();
        self.undo_stack.push(cmd);
        Ok(desc)
    }

    /// Marks the current state as saved / clean.
    pub fn mark_saved(&mut self) {
        self.save_index = Some(self.undo_stack.len());
    }

    /// Returns `true` if there are unsaved changes.
    pub fn is_dirty(&self) -> bool {
        self.save_index != Some(self.undo_stack.len())
    }
}

#[cfg(test)]
mod tests {
    use crate::Node;

    use super::*;

    #[test]
    fn test_add_and_undo_primitive() {
        let mut ds = DataSet::new();
        let mut stack = CommandStack::new();
        assert!(!stack.is_dirty());

        let node = Node::new(NodeId(1), LatLon::new(50.0, 10.0));
        let cmd = Box::new(AddPrimitiveCommand::new(OsmPrimitive::Node(node)));
        stack.execute(cmd, &mut ds).unwrap();

        assert_eq!(ds.node_count(), 1);
        assert!(stack.is_dirty());

        // Undo
        stack.undo(&mut ds).unwrap();
        assert_eq!(ds.node_count(), 0);
        assert!(!stack.is_dirty());

        // Redo
        stack.redo(&mut ds).unwrap();
        assert_eq!(ds.node_count(), 1);
        assert!(stack.is_dirty());
    }

    #[test]
    fn test_move_node_undo_redo() {
        let mut ds = DataSet::new();
        let mut stack = CommandStack::new();

        ds.insert_node(Node::new(NodeId(1), LatLon::new(10.0, 10.0)));
        stack.mark_saved();

        let move_cmd = Box::new(MoveNodeCommand::new(NodeId(1), LatLon::new(20.0, 20.0)));
        stack.execute(move_cmd, &mut ds).unwrap();

        assert_eq!(
            ds.get_node(NodeId(1)).unwrap().position,
            LatLon::new(20.0, 20.0)
        );
        assert!(stack.is_dirty());

        stack.undo(&mut ds).unwrap();
        assert_eq!(
            ds.get_node(NodeId(1)).unwrap().position,
            LatLon::new(10.0, 10.0)
        );
        assert!(!stack.is_dirty());
    }

    #[test]
    fn test_split_way_command() {
        let mut ds = DataSet::new();
        let mut stack = CommandStack::new();

        let n1 = NodeId(1);
        let n2 = NodeId(2);
        let n3 = NodeId(3);
        ds.insert_node(Node::new(n1, LatLon::new(0.0, 0.0)));
        ds.insert_node(Node::new(n2, LatLon::new(1.0, 1.0)));
        ds.insert_node(Node::new(n3, LatLon::new(2.0, 2.0)));

        let mut way = Way::new(WayId(100), vec![n1, n2, n3]);
        way.tags.insert("highway", "residential");
        ds.insert_way(way);

        // Split way at n2
        let split_cmd = Box::new(SplitWayCommand::new(WayId(100), n2));
        stack.execute(split_cmd, &mut ds).unwrap();

        assert_eq!(ds.way_count(), 2);
        let orig = ds.get_way(WayId(100)).unwrap();
        assert_eq!(orig.nodes, vec![n1, n2]);

        let other = ds.ways().find(|w| w.id != WayId(100)).unwrap();
        assert_eq!(other.nodes, vec![n2, n3]);
        assert_eq!(other.tags.get("highway"), Some("residential"));

        // Undo split
        stack.undo(&mut ds).unwrap();
        assert_eq!(ds.way_count(), 1);
        let restored = ds.get_way(WayId(100)).unwrap();
        assert_eq!(restored.nodes, vec![n1, n2, n3]);
    }

    #[test]
    fn test_sequence_command() {
        let mut ds = DataSet::new();
        let mut seq = SequenceCommand::new("Add two nodes");
        seq.push(Box::new(AddPrimitiveCommand::new(OsmPrimitive::Node(
            Node::new(1, LatLon::new(0.0, 0.0)),
        ))));
        seq.push(Box::new(AddPrimitiveCommand::new(OsmPrimitive::Node(
            Node::new(2, LatLon::new(1.0, 1.0)),
        ))));

        let mut stack = CommandStack::new();
        stack.execute(Box::new(seq), &mut ds).unwrap();
        assert_eq!(ds.node_count(), 2);

        stack.undo(&mut ds).unwrap();
        assert_eq!(ds.node_count(), 0);
    }

    #[test]
    fn test_combine_ways_command() {
        let mut ds = DataSet::new();
        let mut stack = CommandStack::new();

        let n1 = NodeId(1);
        let n2 = NodeId(2);
        let n3 = NodeId(3);
        ds.insert_node(Node::new(n1, LatLon::new(0.0, 0.0)));
        ds.insert_node(Node::new(n2, LatLon::new(1.0, 1.0)));
        ds.insert_node(Node::new(n3, LatLon::new(2.0, 2.0)));

        ds.insert_way(Way::new(WayId(10), vec![n1, n2]));
        ds.insert_way(Way::new(WayId(20), vec![n2, n3]));

        let combine_cmd = Box::new(CombineWaysCommand::new(WayId(10), WayId(20)));
        stack.execute(combine_cmd, &mut ds).unwrap();

        assert_eq!(ds.way_count(), 1);
        let w = ds.get_way(WayId(10)).unwrap();
        assert_eq!(w.nodes, vec![n1, n2, n3]);

        // Undo combine
        stack.undo(&mut ds).unwrap();
        assert_eq!(ds.way_count(), 2);
        assert_eq!(ds.get_way(WayId(10)).unwrap().nodes, vec![n1, n2]);
        assert_eq!(ds.get_way(WayId(20)).unwrap().nodes, vec![n2, n3]);
    }

    #[test]
    fn test_merge_nodes_command() {
        let mut ds = DataSet::new();
        let mut stack = CommandStack::new();

        let n1 = NodeId(1);
        let n2 = NodeId(2);
        let n3 = NodeId(3);
        ds.insert_node(Node::new(n1, LatLon::new(0.0, 0.0)));

        let mut node2 = Node::new(n2, LatLon::new(0.00001, 0.00001));
        node2.tags.insert("amenity", "bench");
        ds.insert_node(node2);

        ds.insert_node(Node::new(n3, LatLon::new(1.0, 1.0)));

        // Way references n2
        ds.insert_way(Way::new(WayId(100), vec![n2, n3]));

        // Merge n2 into n1
        let merge_cmd = Box::new(MergeNodesCommand::new(n1, n2));
        stack.execute(merge_cmd, &mut ds).unwrap();

        assert_eq!(ds.node_count(), 2);
        assert!(!ds.contains_node(n2));
        let merged_n1 = ds.get_node(n1).unwrap();
        assert_eq!(merged_n1.tags.get("amenity"), Some("bench"));

        // Way now references n1 instead of n2
        let way = ds.get_way(WayId(100)).unwrap();
        assert_eq!(way.nodes, vec![n1, n3]);

        // Undo merge
        stack.undo(&mut ds).unwrap();
        assert_eq!(ds.node_count(), 3);
        assert!(ds.contains_node(n2));
        let restored_way = ds.get_way(WayId(100)).unwrap();
        assert_eq!(restored_way.nodes, vec![n2, n3]);
    }
}
