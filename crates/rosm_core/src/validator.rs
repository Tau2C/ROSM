//! Dataset validation engine for detecting topological and semantic mapping issues (JOSM-style validator).

use crate::dataset::DataSet;
use crate::id::PrimitiveId;

/// Severity level of a validation finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// A specific issue reported by the validator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationIssue {
    pub rule: &'static str,
    pub message: String,
    pub severity: Severity,
    pub primitives: Vec<PrimitiveId>,
}

impl ValidationIssue {
    pub fn new(
        rule: &'static str,
        message: impl Into<String>,
        severity: Severity,
        primitives: Vec<PrimitiveId>,
    ) -> Self {
        Self {
            rule,
            message: message.into(),
            severity,
            primitives,
        }
    }
}

/// Runs all validation rules on the dataset and returns a list of issues.
pub fn validate(dataset: &DataSet) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    validate_dangling_nodes(dataset, &mut issues);
    validate_way_length(dataset, &mut issues);
    validate_duplicate_consecutive_nodes(dataset, &mut issues);
    validate_untagged_isolated_nodes(dataset, &mut issues);
    validate_empty_tags(dataset, &mut issues);
    validate_dangling_relation_members(dataset, &mut issues);

    issues
}

/// Checks for ways referencing node IDs that are missing from the dataset.
fn validate_dangling_nodes(dataset: &DataSet, issues: &mut Vec<ValidationIssue>) {
    for way in dataset.ways() {
        for nid in &way.nodes {
            if !dataset.contains_node(*nid) {
                issues.push(ValidationIssue::new(
                    "DanglingNodeReference",
                    format!("Way {} references missing node {}", way.id, nid),
                    Severity::Error,
                    vec![PrimitiveId::Way(way.id)],
                ));
            }
        }
    }
}

/// Checks for ways with fewer than 2 nodes.
fn validate_way_length(dataset: &DataSet, issues: &mut Vec<ValidationIssue>) {
    for way in dataset.ways() {
        if way.nodes.len() < 2 {
            issues.push(ValidationIssue::new(
                "WayTooShort",
                format!(
                    "Way {} has fewer than 2 nodes ({})",
                    way.id,
                    way.nodes.len()
                ),
                Severity::Error,
                vec![PrimitiveId::Way(way.id)],
            ));
        }
    }
}

/// Checks for consecutive duplicate node IDs in ways.
fn validate_duplicate_consecutive_nodes(dataset: &DataSet, issues: &mut Vec<ValidationIssue>) {
    for way in dataset.ways() {
        for window in way.nodes.windows(2) {
            if window[0] == window[1] {
                issues.push(ValidationIssue::new(
                    "ConsecutiveDuplicateNodes",
                    format!(
                        "Way {} contains consecutive duplicate node {}",
                        way.id, window[0]
                    ),
                    Severity::Warning,
                    vec![PrimitiveId::Way(way.id), PrimitiveId::Node(window[0])],
                ));
            }
        }
    }
}

/// Checks for nodes that have no tags and are not part of any way or relation.
fn validate_untagged_isolated_nodes(dataset: &DataSet, issues: &mut Vec<ValidationIssue>) {
    for node in dataset.nodes() {
        if node.tags.is_empty() {
            let in_ways = !dataset.ways_using_node(node.id).is_empty();
            let in_relations = !dataset
                .relations_using(PrimitiveId::Node(node.id))
                .is_empty();
            if !in_ways && !in_relations {
                issues.push(ValidationIssue::new(
                    "UntaggedIsolatedNode",
                    format!(
                        "Node {} has no tags and is not connected to any way",
                        node.id
                    ),
                    Severity::Warning,
                    vec![PrimitiveId::Node(node.id)],
                ));
            }
        }
    }
}

/// Checks for empty tag keys or values.
fn validate_empty_tags(dataset: &DataSet, issues: &mut Vec<ValidationIssue>) {
    // Check nodes
    for node in dataset.nodes() {
        for (k, v) in node.tags.iter() {
            if k.trim().is_empty() || v.trim().is_empty() {
                issues.push(ValidationIssue::new(
                    "EmptyTag",
                    format!(
                        "Node {} contains an empty tag key or value: '{k}'='{v}'",
                        node.id
                    ),
                    Severity::Warning,
                    vec![PrimitiveId::Node(node.id)],
                ));
            }
        }
    }

    // Check ways
    for way in dataset.ways() {
        for (k, v) in way.tags.iter() {
            if k.trim().is_empty() || v.trim().is_empty() {
                issues.push(ValidationIssue::new(
                    "EmptyTag",
                    format!(
                        "Way {} contains an empty tag key or value: '{k}'='{v}'",
                        way.id
                    ),
                    Severity::Warning,
                    vec![PrimitiveId::Way(way.id)],
                ));
            }
        }
    }
}

/// Checks for relations with members that do not exist in the dataset.
fn validate_dangling_relation_members(dataset: &DataSet, issues: &mut Vec<ValidationIssue>) {
    for rel in dataset.relations() {
        for m in &rel.members {
            let exists = match m.member {
                PrimitiveId::Node(nid) => dataset.contains_node(nid),
                PrimitiveId::Way(wid) => dataset.contains_way(wid),
                PrimitiveId::Relation(rid) => dataset.contains_relation(rid),
            };
            if !exists {
                issues.push(ValidationIssue::new(
                    "DanglingRelationMember",
                    format!("Relation {} references missing member {}", rel.id, m.member),
                    Severity::Warning,
                    vec![PrimitiveId::Relation(rel.id)],
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geo::LatLon;
    use crate::primitive::{Node, Way};
    use crate::{NodeId, WayId};

    #[test]
    fn test_validator_detects_issues() {
        let mut ds = DataSet::new();

        // 1. Untagged isolated node
        let isolated = Node::new(NodeId(1), LatLon::new(0.0, 0.0));
        ds.insert_node(isolated);

        // 2. Way too short & consecutive duplicate nodes
        let n2 = Node::new(NodeId(2), LatLon::new(1.0, 1.0));
        ds.insert_node(n2);
        let way_short = Way::new(WayId(10), vec![NodeId(2)]);
        ds.insert_way(way_short);

        // 3. Dangling node reference
        let way_dangling = Way::new(WayId(11), vec![NodeId(2), NodeId(999)]);
        ds.insert_way(way_dangling);

        let issues = validate(&ds);

        assert!(issues.iter().any(|i| i.rule == "UntaggedIsolatedNode"));
        assert!(issues.iter().any(|i| i.rule == "WayTooShort"));
        assert!(issues.iter().any(|i| i.rule == "DanglingNodeReference"));
    }

    #[test]
    fn test_valid_dataset_has_no_issues() {
        let mut ds = DataSet::new();
        let mut n1 = Node::new(NodeId(1), LatLon::new(0.0, 0.0));
        n1.tags.insert("highway", "bus_stop");
        let n2 = Node::new(NodeId(2), LatLon::new(1.0, 1.0));

        ds.insert_node(n1);
        ds.insert_node(n2);

        let mut way = Way::new(WayId(10), vec![NodeId(1), NodeId(2)]);
        way.tags.insert("highway", "residential");
        ds.insert_way(way);

        let issues = validate(&ds);
        assert!(issues.is_empty(), "expected 0 issues, got: {issues:?}");
    }
}
