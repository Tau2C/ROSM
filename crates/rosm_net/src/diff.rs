//! OsmChange (.osc) calculation, XML serialization, and server diffResult remapping.

use std::collections::{HashMap, HashSet};

use rosm_core::id::{NodeId, PrimitiveId, RelationId, WayId};
use rosm_core::primitive::{Node, Relation, Way};
use rosm_core::DataSet;

use crate::error::NetError;

/// Categorized changes between a baseline dataset and current working dataset.
#[derive(Debug, Default, Clone)]
pub struct OsmDiff {
    pub created_nodes: Vec<Node>,
    pub created_ways: Vec<Way>,
    pub created_relations: Vec<Relation>,

    pub modified_nodes: Vec<Node>,
    pub modified_ways: Vec<Way>,
    pub modified_relations: Vec<Relation>,

    pub deleted_nodes: Vec<Node>,
    pub deleted_ways: Vec<Way>,
    pub deleted_relations: Vec<Relation>,
}

impl OsmDiff {
    /// Returns true if there are no creations, modifications, or deletions.
    pub fn is_empty(&self) -> bool {
        self.created_nodes.is_empty()
            && self.created_ways.is_empty()
            && self.created_relations.is_empty()
            && self.modified_nodes.is_empty()
            && self.modified_ways.is_empty()
            && self.modified_relations.is_empty()
            && self.deleted_nodes.is_empty()
            && self.deleted_ways.is_empty()
            && self.deleted_relations.is_empty()
    }

    /// Total count of changed primitives across all change categories.
    pub fn total_changes(&self) -> usize {
        self.created_nodes.len()
            + self.created_ways.len()
            + self.created_relations.len()
            + self.modified_nodes.len()
            + self.modified_ways.len()
            + self.modified_relations.len()
            + self.deleted_nodes.len()
            + self.deleted_ways.len()
            + self.deleted_relations.len()
    }

    /// Computes the difference between a baseline dataset and the current dataset.
    pub fn compute(base: &DataSet, current: &DataSet) -> Self {
        let mut diff = Self::default();

        // 1. Nodes
        let base_node_ids: HashSet<NodeId> = base.nodes().map(|n| n.id).collect();
        let current_node_ids: HashSet<NodeId> = current.nodes().map(|n| n.id).collect();

        for node in current.nodes() {
            if node.id.is_new() {
                // Negative ID -> Newly created
                diff.created_nodes.push(node.clone());
            } else if let Some(base_node) = base.get_node(node.id) {
                // Positive ID existing in base -> Check if modified
                if node.position != base_node.position || node.tags != base_node.tags {
                    diff.modified_nodes.push(node.clone());
                }
            } else {
                // Positive ID not in base -> Created with explicit ID
                diff.created_nodes.push(node.clone());
            }
        }

        for &id in &base_node_ids {
            if !current_node_ids.contains(&id) {
                if let Some(node) = base.get_node(id) {
                    diff.deleted_nodes.push(node.clone());
                }
            }
        }

        // 2. Ways
        let base_way_ids: HashSet<WayId> = base.ways().map(|w| w.id).collect();
        let current_way_ids: HashSet<WayId> = current.ways().map(|w| w.id).collect();

        for way in current.ways() {
            if way.id.is_new() {
                diff.created_ways.push(way.clone());
            } else if let Some(base_way) = base.get_way(way.id) {
                if way.nodes != base_way.nodes || way.tags != base_way.tags {
                    diff.modified_ways.push(way.clone());
                }
            } else {
                diff.created_ways.push(way.clone());
            }
        }

        for &id in &base_way_ids {
            if !current_way_ids.contains(&id) {
                if let Some(way) = base.get_way(id) {
                    diff.deleted_ways.push(way.clone());
                }
            }
        }

        // 3. Relations
        let base_rel_ids: HashSet<RelationId> = base.relations().map(|r| r.id).collect();
        let current_rel_ids: HashSet<RelationId> = current.relations().map(|r| r.id).collect();

        for rel in current.relations() {
            if rel.id.is_new() {
                diff.created_relations.push(rel.clone());
            } else if let Some(base_rel) = base.get_relation(rel.id) {
                if rel.members != base_rel.members || rel.tags != base_rel.tags {
                    diff.modified_relations.push(rel.clone());
                }
            } else {
                diff.created_relations.push(rel.clone());
            }
        }

        for &id in &base_rel_ids {
            if !current_rel_ids.contains(&id) {
                if let Some(rel) = base.get_relation(id) {
                    diff.deleted_relations.push(rel.clone());
                }
            }
        }

        diff
    }

    /// Serializes the diff into an OsmChange (.osc) XML document.
    pub fn to_osm_change_xml(&self, changeset_id: i64) -> String {
        let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        xml.push_str("<osmChange version=\"0.6\" generator=\"ROSM 0.1.0\">\n");

        // 1. <create>: Nodes -> Ways -> Relations
        if !self.created_nodes.is_empty()
            || !self.created_ways.is_empty()
            || !self.created_relations.is_empty()
        {
            xml.push_str("  <create>\n");
            for node in &self.created_nodes {
                Self::write_node_xml(&mut xml, node, changeset_id, false);
            }
            for way in &self.created_ways {
                Self::write_way_xml(&mut xml, way, changeset_id, false);
            }
            for rel in &self.created_relations {
                Self::write_relation_xml(&mut xml, rel, changeset_id, false);
            }
            xml.push_str("  </create>\n");
        }

        // 2. <modify>: Nodes -> Ways -> Relations
        if !self.modified_nodes.is_empty()
            || !self.modified_ways.is_empty()
            || !self.modified_relations.is_empty()
        {
            xml.push_str("  <modify>\n");
            for node in &self.modified_nodes {
                Self::write_node_xml(&mut xml, node, changeset_id, true);
            }
            for way in &self.modified_ways {
                Self::write_way_xml(&mut xml, way, changeset_id, true);
            }
            for rel in &self.modified_relations {
                Self::write_relation_xml(&mut xml, rel, changeset_id, true);
            }
            xml.push_str("  </modify>\n");
        }

        // 3. <delete>: Relations -> Ways -> Nodes (reverse dependency order)
        if !self.deleted_relations.is_empty()
            || !self.deleted_ways.is_empty()
            || !self.deleted_nodes.is_empty()
        {
            xml.push_str("  <delete if-unused=\"true\">\n");
            for rel in &self.deleted_relations {
                Self::write_relation_xml(&mut xml, rel, changeset_id, true);
            }
            for way in &self.deleted_ways {
                Self::write_way_xml(&mut xml, way, changeset_id, true);
            }
            for node in &self.deleted_nodes {
                Self::write_node_xml(&mut xml, node, changeset_id, true);
            }
            xml.push_str("  </delete>\n");
        }

        xml.push_str("</osmChange>\n");
        xml
    }

    fn write_node_xml(xml: &mut String, node: &Node, changeset: i64, include_version: bool) {
        let version_attr = if include_version {
            format!(" version=\"{}\"", node.metadata.version.unwrap_or(1).max(1))
        } else {
            String::new()
        };

        if node.tags.is_empty() {
            xml.push_str(&format!(
                "    <node id=\"{}\" lat=\"{:.7}\" lon=\"{:.7}\" changeset=\"{}\"{}/>\n",
                node.id.0,
                node.position.lat(),
                node.position.lon(),
                changeset,
                version_attr
            ));
        } else {
            xml.push_str(&format!(
                "    <node id=\"{}\" lat=\"{:.7}\" lon=\"{:.7}\" changeset=\"{}\"{}>\n",
                node.id.0,
                node.position.lat(),
                node.position.lon(),
                changeset,
                version_attr
            ));
            for (k, v) in node.tags.iter() {
                xml.push_str(&format!(
                    "      <tag k=\"{}\" v=\"{}\"/>\n",
                    quick_xml::escape::escape(k),
                    quick_xml::escape::escape(v)
                ));
            }
            xml.push_str("    </node>\n");
        }
    }

    fn write_way_xml(xml: &mut String, way: &Way, changeset: i64, include_version: bool) {
        let version_attr = if include_version {
            format!(" version=\"{}\"", way.metadata.version.unwrap_or(1).max(1))
        } else {
            String::new()
        };

        xml.push_str(&format!(
            "    <way id=\"{}\" changeset=\"{}\"{}>\n",
            way.id.0, changeset, version_attr
        ));
        for &nd in &way.nodes {
            xml.push_str(&format!("      <nd ref=\"{}\"/>\n", nd.0));
        }
        for (k, v) in way.tags.iter() {
            xml.push_str(&format!(
                "      <tag k=\"{}\" v=\"{}\"/>\n",
                quick_xml::escape::escape(k),
                quick_xml::escape::escape(v)
            ));
        }
        xml.push_str("    </way>\n");
    }

    fn write_relation_xml(xml: &mut String, rel: &Relation, changeset: i64, include_version: bool) {
        let version_attr = if include_version {
            format!(" version=\"{}\"", rel.metadata.version.unwrap_or(1).max(1))
        } else {
            String::new()
        };

        xml.push_str(&format!(
            "    <relation id=\"{}\" changeset=\"{}\"{}>\n",
            rel.id.0, changeset, version_attr
        ));
        for member in &rel.members {
            let (type_str, ref_id) = match member.member {
                PrimitiveId::Node(nid) => ("node", nid.0),
                PrimitiveId::Way(wid) => ("way", wid.0),
                PrimitiveId::Relation(rid) => ("relation", rid.0),
            };
            xml.push_str(&format!(
                "      <member type=\"{}\" ref=\"{}\" role=\"{}\"/>\n",
                type_str,
                ref_id,
                quick_xml::escape::escape(&member.role)
            ));
        }
        for (k, v) in rel.tags.iter() {
            xml.push_str(&format!(
                "      <tag k=\"{}\" v=\"{}\"/>\n",
                quick_xml::escape::escape(k),
                quick_xml::escape::escape(v)
            ));
        }
        xml.push_str("    </relation>\n");
    }
}

/// Mappings parsed from an OSM API `<diffResult>` response.
#[derive(Debug, Default, Clone)]
pub struct DiffResult {
    pub node_map: HashMap<i64, (i64, i32)>, // old_id -> (new_id, new_version)
    pub way_map: HashMap<i64, (i64, i32)>,
    pub relation_map: HashMap<i64, (i64, i32)>,
}

impl DiffResult {
    /// Parses an XML `<diffResult>` payload returned by the server.
    pub fn parse(xml_content: &str) -> Result<Self, NetError> {
        use quick_xml::events::Event;
        use quick_xml::reader::Reader;

        let mut reader = Reader::from_str(xml_content);
        reader.config_mut().trim_text(true);

        let mut result = Self::default();
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                    let tag_name = e.name().as_ref().to_vec();
                    let mut old_id: Option<i64> = None;
                    let mut new_id: Option<i64> = None;
                    let mut new_version: Option<i32> = None;

                    for attr in e.attributes().flatten() {
                        let val = String::from_utf8_lossy(&attr.value);
                        match attr.key.as_ref() {
                            b"old_id" => old_id = val.parse().ok(),
                            b"new_id" => new_id = val.parse().ok(),
                            b"new_version" => new_version = val.parse().ok(),
                            _ => {}
                        }
                    }

                    if let (Some(old), Some(new), Some(ver)) = (old_id, new_id, new_version) {
                        match tag_name.as_slice() {
                            b"node" => {
                                result.node_map.insert(old, (new, ver));
                            }
                            b"way" => {
                                result.way_map.insert(old, (new, ver));
                            }
                            b"relation" => {
                                result.relation_map.insert(old, (new, ver));
                            }
                            _ => {}
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(NetError::XmlParse(format!("DiffResult parse error: {e}"))),
                _ => {}
            }
            buf.clear();
        }

        Ok(result)
    }

    /// Remaps local negative IDs in a dataset to permanent server IDs and updates versions.
    pub fn apply_to_dataset(&self, dataset: &mut DataSet) {
        // 1. Remap nodes
        let mut nodes_to_remap = Vec::new();
        for node in dataset.nodes() {
            if let Some(&(new_id, new_ver)) = self.node_map.get(&node.id.0) {
                nodes_to_remap.push((node.id, NodeId(new_id), new_ver));
            }
        }
        for (old_id, new_id, new_ver) in nodes_to_remap {
            if let Some(mut node) = dataset.remove_node(old_id) {
                node.id = new_id;
                node.metadata.version = Some(new_ver);
                dataset.insert_node(node);
            }
        }

        // 2. Remap way nodes and way IDs
        let mut ways_to_update = Vec::new();
        for way in dataset.ways() {
            let mut way_clone = way.clone();
            let mut changed = false;

            // Remap referenced nodes
            for nd in &mut way_clone.nodes {
                if let Some(&(new_nid, _)) = self.node_map.get(&nd.0) {
                    *nd = NodeId(new_nid);
                    changed = true;
                }
            }

            // Remap way ID itself
            let new_way_meta = self.way_map.get(&way.id.0).copied();
            if let Some((new_wid, new_ver)) = new_way_meta {
                way_clone.id = WayId(new_wid);
                way_clone.metadata.version = Some(new_ver);
                changed = true;
            }

            if changed {
                ways_to_update.push((way.id, way_clone));
            }
        }
        for (old_wid, new_way) in ways_to_update {
            dataset.remove_way(old_wid);
            dataset.insert_way(new_way);
        }

        // 3. Remap relations
        let mut rels_to_update = Vec::new();
        for rel in dataset.relations() {
            let mut rel_clone = rel.clone();
            let mut changed = false;

            for member in &mut rel_clone.members {
                match member.member {
                    PrimitiveId::Node(nid) => {
                        if let Some(&(new_nid, _)) = self.node_map.get(&nid.0) {
                            member.member = PrimitiveId::Node(NodeId(new_nid));
                            changed = true;
                        }
                    }
                    PrimitiveId::Way(wid) => {
                        if let Some(&(new_wid, _)) = self.way_map.get(&wid.0) {
                            member.member = PrimitiveId::Way(WayId(new_wid));
                            changed = true;
                        }
                    }
                    PrimitiveId::Relation(rid) => {
                        if let Some(&(new_rid, _)) = self.relation_map.get(&rid.0) {
                            member.member = PrimitiveId::Relation(RelationId(new_rid));
                            changed = true;
                        }
                    }
                }
            }

            if let Some(&(new_rid, new_ver)) = self.relation_map.get(&rel.id.0) {
                rel_clone.id = RelationId(new_rid);
                rel_clone.metadata.version = Some(new_ver);
                changed = true;
            }

            if changed {
                rels_to_update.push((rel.id, rel_clone));
            }
        }
        for (old_rid, new_rel) in rels_to_update {
            dataset.remove_relation(old_rid);
            dataset.insert_relation(new_rel);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rosm_core::geo::LatLon;

    #[test]
    fn test_diff_computation_and_osc_generation() {
        let mut base = DataSet::new();
        let mut current = DataSet::new();

        // Node 1: unchanged in both
        let n1 = Node::new(100, LatLon::new(51.5, -0.1));
        base.insert_node(n1.clone());
        current.insert_node(n1);

        // Node 2: modified position and tag
        let n2_base = Node::new(200, LatLon::new(51.6, -0.2));
        base.insert_node(n2_base);
        let mut n2_mod = Node::new(200, LatLon::new(51.61, -0.21));
        n2_mod.tags.insert("amenity", "cafe");
        current.insert_node(n2_mod);

        // Node 3: newly created with negative ID
        let mut n3_new = Node::new(-1, LatLon::new(51.7, -0.3));
        n3_new.tags.insert("name", "New Park");
        current.insert_node(n3_new);

        // Node 4: deleted from base
        let n4_del = Node::new(400, LatLon::new(51.8, -0.4));
        base.insert_node(n4_del);

        let diff = OsmDiff::compute(&base, &current);
        assert_eq!(diff.created_nodes.len(), 1);
        assert_eq!(diff.modified_nodes.len(), 1);
        assert_eq!(diff.deleted_nodes.len(), 1);
        assert_eq!(diff.total_changes(), 3);

        let osc_xml = diff.to_osm_change_xml(9999);
        assert!(osc_xml.contains("<osmChange version=\"0.6\""));
        assert!(osc_xml.contains("<create>"));
        assert!(osc_xml
            .contains("<node id=\"-1\" lat=\"51.7000000\" lon=\"-0.3000000\" changeset=\"9999\">"));
        assert!(osc_xml.contains("<modify>"));
        assert!(osc_xml
            .contains("<node id=\"200\" lat=\"51.6100000\" lon=\"-0.2100000\" changeset=\"9999\""));
        assert!(osc_xml.contains("<delete if-unused=\"true\">"));
        assert!(osc_xml
            .contains("<node id=\"400\" lat=\"51.8000000\" lon=\"-0.4000000\" changeset=\"9999\""));
    }

    #[test]
    fn test_diff_result_parsing_and_remapping() {
        let diff_result_xml = r#"<diffResult generator="OpenStreetMap Server" version="0.6">
            <node old_id="-1" new_id="123456" new_version="1"/>
            <way old_id="-2" new_id="654321" new_version="1"/>
        </diffResult>"#;

        let res = DiffResult::parse(diff_result_xml).expect("Should parse valid diffResult");
        assert_eq!(res.node_map.get(&-1), Some(&(123456, 1)));
        assert_eq!(res.way_map.get(&-2), Some(&(654321, 1)));

        let mut ds = DataSet::new();
        ds.insert_node(Node::new(-1, LatLon::new(51.5, -0.1)));
        ds.insert_way(Way::new(-2, vec![NodeId(-1)]));

        res.apply_to_dataset(&mut ds);

        // Old negative IDs must be gone
        assert!(ds.get_node(NodeId(-1)).is_none());
        assert!(ds.get_way(WayId(-2)).is_none());

        // New server IDs must exist with remapped member references
        let node = ds.get_node(NodeId(123456)).expect("Node must be remapped");
        assert_eq!(node.metadata.version, Some(1));

        let way = ds.get_way(WayId(654321)).expect("Way must be remapped");
        assert_eq!(way.metadata.version, Some(1));
        assert_eq!(way.nodes, vec![NodeId(123456)]);
    }
}
