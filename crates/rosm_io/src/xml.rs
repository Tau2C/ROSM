//! Reader and writer for OpenStreetMap XML format (.osm / OSM API v0.6).

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, Event};
use quick_xml::{Reader, Writer};
use std::io::{BufRead, Write};

use rosm_core::geo::LatLon;
use rosm_core::id::{NodeId, PrimitiveId, RelationId, WayId};
use rosm_core::primitive::{Node, OsmMetadata, Relation, RelationMember, Way};
use rosm_core::DataSet;

use crate::error::IoError;

/// Reads an OSM XML stream into a `DataSet`.
pub fn read_osm_xml<R: BufRead>(reader: R) -> Result<DataSet, IoError> {
    let mut xml = Reader::from_reader(reader);
    xml.config_mut().trim_text(true);

    let mut dataset = DataSet::new();
    let mut buf = Vec::new();

    enum CurrentState {
        None,
        Node(Node),
        Way(Way),
        Relation(Relation),
    }

    let mut state = CurrentState::None;

    loop {
        buf.clear();
        match xml.read_event_into(&mut buf)? {
            Event::Eof => break,
            Event::Start(ref e) => match e.name().as_ref() {
                b"node" => {
                    let node = parse_node_attrs(e)?;
                    state = CurrentState::Node(node);
                }
                b"way" => {
                    let way = parse_way_attrs(e)?;
                    state = CurrentState::Way(way);
                }
                b"relation" => {
                    let rel = parse_relation_attrs(e)?;
                    state = CurrentState::Relation(rel);
                }
                _ => {}
            },
            Event::Empty(ref e) => match e.name().as_ref() {
                b"node" => {
                    let node = parse_node_attrs(e)?;
                    dataset.insert_node(node);
                }
                b"way" => {
                    let way = parse_way_attrs(e)?;
                    dataset.insert_way(way);
                }
                b"relation" => {
                    let rel = parse_relation_attrs(e)?;
                    dataset.insert_relation(rel);
                }
                b"tag" => {
                    let (k, v) = parse_tag_attrs(e)?;
                    match state {
                        CurrentState::Node(ref mut n) => {
                            n.tags.insert(k, v);
                        }
                        CurrentState::Way(ref mut w) => {
                            w.tags.insert(k, v);
                        }
                        CurrentState::Relation(ref mut r) => {
                            r.tags.insert(k, v);
                        }
                        CurrentState::None => {}
                    }
                }
                b"nd" => {
                    if let CurrentState::Way(ref mut w) = state {
                        let ref_id = parse_nd_ref(e)?;
                        w.nodes.push(ref_id);
                    }
                }
                b"member" => {
                    if let CurrentState::Relation(ref mut r) = state {
                        let member = parse_member_attrs(e)?;
                        r.members.push(member);
                    }
                }
                _ => {}
            },
            Event::End(ref e) => match e.name().as_ref() {
                b"node" => {
                    if let CurrentState::Node(node) =
                        std::mem::replace(&mut state, CurrentState::None)
                    {
                        dataset.insert_node(node);
                    }
                }
                b"way" => {
                    if let CurrentState::Way(way) =
                        std::mem::replace(&mut state, CurrentState::None)
                    {
                        dataset.insert_way(way);
                    }
                }
                b"relation" => {
                    if let CurrentState::Relation(rel) =
                        std::mem::replace(&mut state, CurrentState::None)
                    {
                        dataset.insert_relation(rel);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    Ok(dataset)
}

fn parse_node_attrs(e: &BytesStart) -> Result<Node, IoError> {
    let mut id = None;
    let mut lat = None;
    let mut lon = None;
    let mut meta = OsmMetadata::new();

    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"id" => {
                let s = std::str::from_utf8(&attr.value)
                    .map_err(|e| IoError::ParseNumber("id", e.to_string()))?;
                id = Some(
                    s.parse::<i64>()
                        .map_err(|e| IoError::ParseNumber("id", e.to_string()))?,
                );
            }
            b"lat" => {
                let s = std::str::from_utf8(&attr.value)
                    .map_err(|e| IoError::ParseNumber("lat", e.to_string()))?;
                lat = Some(
                    s.parse::<f64>()
                        .map_err(|e| IoError::ParseNumber("lat", e.to_string()))?,
                );
            }
            b"lon" => {
                let s = std::str::from_utf8(&attr.value)
                    .map_err(|e| IoError::ParseNumber("lon", e.to_string()))?;
                lon = Some(
                    s.parse::<f64>()
                        .map_err(|e| IoError::ParseNumber("lon", e.to_string()))?,
                );
            }
            b"version" => {
                if let Ok(s) = std::str::from_utf8(&attr.value) {
                    meta.version = s.parse::<i32>().ok();
                }
            }
            b"changeset" => {
                if let Ok(s) = std::str::from_utf8(&attr.value) {
                    meta.changeset = s.parse::<i64>().ok();
                }
            }
            b"timestamp" => {
                if let Ok(s) = std::str::from_utf8(&attr.value) {
                    meta.timestamp = Some(s.to_string());
                }
            }
            b"user" => {
                if let Ok(s) = std::str::from_utf8(&attr.value) {
                    meta.user = Some(s.to_string());
                }
            }
            b"uid" => {
                if let Ok(s) = std::str::from_utf8(&attr.value) {
                    meta.uid = s.parse::<i64>().ok();
                }
            }
            b"visible" => {
                if let Ok(s) = std::str::from_utf8(&attr.value) {
                    meta.visible = s != "false";
                }
            }
            _ => {}
        }
    }

    let id = id.ok_or(IoError::MissingAttribute("id", "node"))?;
    let lat = lat.ok_or(IoError::MissingAttribute("lat", "node"))?;
    let lon = lon.ok_or(IoError::MissingAttribute("lon", "node"))?;
    let position = LatLon::try_new(lat, lon)
        .ok_or_else(|| IoError::InvalidCoordinate(format!("lat={lat}, lon={lon}")))?;

    Ok(Node::new(NodeId(id), position).with_metadata(meta))
}

fn parse_way_attrs(e: &BytesStart) -> Result<Way, IoError> {
    let mut id = None;
    let mut meta = OsmMetadata::new();

    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"id" => {
                let s = std::str::from_utf8(&attr.value)
                    .map_err(|e| IoError::ParseNumber("id", e.to_string()))?;
                id = Some(
                    s.parse::<i64>()
                        .map_err(|e| IoError::ParseNumber("id", e.to_string()))?,
                );
            }
            b"version" => {
                if let Ok(s) = std::str::from_utf8(&attr.value) {
                    meta.version = s.parse::<i32>().ok();
                }
            }
            b"changeset" => {
                if let Ok(s) = std::str::from_utf8(&attr.value) {
                    meta.changeset = s.parse::<i64>().ok();
                }
            }
            b"timestamp" => {
                if let Ok(s) = std::str::from_utf8(&attr.value) {
                    meta.timestamp = Some(s.to_string());
                }
            }
            b"user" => {
                if let Ok(s) = std::str::from_utf8(&attr.value) {
                    meta.user = Some(s.to_string());
                }
            }
            b"uid" => {
                if let Ok(s) = std::str::from_utf8(&attr.value) {
                    meta.uid = s.parse::<i64>().ok();
                }
            }
            b"visible" => {
                if let Ok(s) = std::str::from_utf8(&attr.value) {
                    meta.visible = s != "false";
                }
            }
            _ => {}
        }
    }

    let id = id.ok_or(IoError::MissingAttribute("id", "way"))?;
    Ok(Way::new(WayId(id), Vec::new()).with_metadata(meta))
}

fn parse_relation_attrs(e: &BytesStart) -> Result<Relation, IoError> {
    let mut id = None;
    let mut meta = OsmMetadata::new();

    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"id" => {
                let s = std::str::from_utf8(&attr.value)
                    .map_err(|e| IoError::ParseNumber("id", e.to_string()))?;
                id = Some(
                    s.parse::<i64>()
                        .map_err(|e| IoError::ParseNumber("id", e.to_string()))?,
                );
            }
            b"version" => {
                if let Ok(s) = std::str::from_utf8(&attr.value) {
                    meta.version = s.parse::<i32>().ok();
                }
            }
            b"changeset" => {
                if let Ok(s) = std::str::from_utf8(&attr.value) {
                    meta.changeset = s.parse::<i64>().ok();
                }
            }
            b"timestamp" => {
                if let Ok(s) = std::str::from_utf8(&attr.value) {
                    meta.timestamp = Some(s.to_string());
                }
            }
            b"user" => {
                if let Ok(s) = std::str::from_utf8(&attr.value) {
                    meta.user = Some(s.to_string());
                }
            }
            b"uid" => {
                if let Ok(s) = std::str::from_utf8(&attr.value) {
                    meta.uid = s.parse::<i64>().ok();
                }
            }
            b"visible" => {
                if let Ok(s) = std::str::from_utf8(&attr.value) {
                    meta.visible = s != "false";
                }
            }
            _ => {}
        }
    }

    let id = id.ok_or(IoError::MissingAttribute("id", "relation"))?;
    Ok(Relation::new(RelationId(id), Vec::new()).with_metadata(meta))
}

fn parse_tag_attrs(e: &BytesStart) -> Result<(String, String), IoError> {
    let mut k = None;
    let mut v = None;

    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"k" => {
                if let Ok(s) = std::str::from_utf8(&attr.value) {
                    k = Some(s.to_string());
                }
            }
            b"v" => {
                if let Ok(s) = std::str::from_utf8(&attr.value) {
                    v = Some(s.to_string());
                }
            }
            _ => {}
        }
    }

    let key = k.ok_or(IoError::MissingAttribute("k", "tag"))?;
    let val = v.ok_or(IoError::MissingAttribute("v", "tag"))?;
    Ok((key, val))
}

fn parse_nd_ref(e: &BytesStart) -> Result<NodeId, IoError> {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == b"ref" {
            let s = std::str::from_utf8(&attr.value)
                .map_err(|e| IoError::ParseNumber("ref", e.to_string()))?;
            let id = s
                .parse::<i64>()
                .map_err(|e| IoError::ParseNumber("ref", e.to_string()))?;
            return Ok(NodeId(id));
        }
    }
    Err(IoError::MissingAttribute("ref", "nd"))
}

fn parse_member_attrs(e: &BytesStart) -> Result<RelationMember, IoError> {
    let mut member_type = None;
    let mut member_ref = None;
    let mut role = String::new();

    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"type" => {
                if let Ok(s) = std::str::from_utf8(&attr.value) {
                    member_type = Some(s.to_string());
                }
            }
            b"ref" => {
                let s = std::str::from_utf8(&attr.value)
                    .map_err(|e| IoError::ParseNumber("ref", e.to_string()))?;
                member_ref = Some(
                    s.parse::<i64>()
                        .map_err(|e| IoError::ParseNumber("ref", e.to_string()))?,
                );
            }
            b"role" => {
                if let Ok(s) = std::str::from_utf8(&attr.value) {
                    role = s.to_string();
                }
            }
            _ => {}
        }
    }

    let m_type = member_type.ok_or(IoError::MissingAttribute("type", "member"))?;
    let m_ref = member_ref.ok_or(IoError::MissingAttribute("ref", "member"))?;

    let prim_id = match m_type.as_str() {
        "node" => PrimitiveId::Node(NodeId(m_ref)),
        "way" => PrimitiveId::Way(WayId(m_ref)),
        "relation" => PrimitiveId::Relation(RelationId(m_ref)),
        other => return Err(IoError::UnknownType(other.to_string())),
    };

    Ok(RelationMember::new(prim_id, role))
}

/// Writes a `DataSet` into an OSM XML format.
pub fn write_osm_xml<W: Write>(dataset: &DataSet, writer: W) -> Result<(), IoError> {
    let mut xml = Writer::new_with_indent(writer, b' ', 2);

    xml.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

    let mut osm_start = BytesStart::new("osm");
    osm_start.push_attribute(("version", "0.6"));
    osm_start.push_attribute(("generator", "ROSM"));
    xml.write_event(Event::Start(osm_start))?;

    // Optional bounds
    if let Some(bounds) = dataset.bounds() {
        let mut b_start = BytesStart::new("bounds");
        b_start.push_attribute(("minlat", bounds.min_lat.to_string().as_str()));
        b_start.push_attribute(("minlon", bounds.min_lon.to_string().as_str()));
        b_start.push_attribute(("maxlat", bounds.max_lat.to_string().as_str()));
        b_start.push_attribute(("maxlon", bounds.max_lon.to_string().as_str()));
        xml.write_event(Event::Empty(b_start))?;
    }

    // Write nodes (sorted by ID for reproducibility)
    let mut nodes: Vec<_> = dataset.nodes().collect();
    nodes.sort_by_key(|n| n.id.0);

    for node in nodes {
        let mut n_start = BytesStart::new("node");
        n_start.push_attribute(("id", node.id.0.to_string().as_str()));
        n_start.push_attribute(("lat", node.position.lat().to_string().as_str()));
        n_start.push_attribute(("lon", node.position.lon().to_string().as_str()));

        if let Some(v) = node.metadata.version {
            n_start.push_attribute(("version", v.to_string().as_str()));
        }
        if let Some(cs) = node.metadata.changeset {
            n_start.push_attribute(("changeset", cs.to_string().as_str()));
        }
        if let Some(ref ts) = node.metadata.timestamp {
            n_start.push_attribute(("timestamp", ts.as_str()));
        }
        if let Some(ref u) = node.metadata.user {
            n_start.push_attribute(("user", u.as_str()));
        }
        if let Some(uid) = node.metadata.uid {
            n_start.push_attribute(("uid", uid.to_string().as_str()));
        }

        if node.tags.is_empty() {
            xml.write_event(Event::Empty(n_start))?;
        } else {
            xml.write_event(Event::Start(n_start))?;
            for (k, v) in node.tags.iter() {
                let mut tag = BytesStart::new("tag");
                tag.push_attribute(("k", k));
                tag.push_attribute(("v", v));
                xml.write_event(Event::Empty(tag))?;
            }
            xml.write_event(Event::End(BytesEnd::new("node")))?;
        }
    }

    // Write ways (sorted by ID)
    let mut ways: Vec<_> = dataset.ways().collect();
    ways.sort_by_key(|w| w.id.0);

    for way in ways {
        let mut w_start = BytesStart::new("way");
        w_start.push_attribute(("id", way.id.0.to_string().as_str()));
        if let Some(v) = way.metadata.version {
            w_start.push_attribute(("version", v.to_string().as_str()));
        }

        if way.nodes.is_empty() && way.tags.is_empty() {
            xml.write_event(Event::Empty(w_start))?;
        } else {
            xml.write_event(Event::Start(w_start))?;
            for nid in &way.nodes {
                let mut nd = BytesStart::new("nd");
                nd.push_attribute(("ref", nid.0.to_string().as_str()));
                xml.write_event(Event::Empty(nd))?;
            }
            for (k, v) in way.tags.iter() {
                let mut tag = BytesStart::new("tag");
                tag.push_attribute(("k", k));
                tag.push_attribute(("v", v));
                xml.write_event(Event::Empty(tag))?;
            }
            xml.write_event(Event::End(BytesEnd::new("way")))?;
        }
    }

    // Write relations (sorted by ID)
    let mut relations: Vec<_> = dataset.relations().collect();
    relations.sort_by_key(|r| r.id.0);

    for rel in relations {
        let mut r_start = BytesStart::new("relation");
        r_start.push_attribute(("id", rel.id.0.to_string().as_str()));
        if let Some(v) = rel.metadata.version {
            r_start.push_attribute(("version", v.to_string().as_str()));
        }

        if rel.members.is_empty() && rel.tags.is_empty() {
            xml.write_event(Event::Empty(r_start))?;
        } else {
            xml.write_event(Event::Start(r_start))?;
            for m in &rel.members {
                let mut member = BytesStart::new("member");
                member.push_attribute(("type", m.member.type_name()));
                member.push_attribute(("ref", m.member.raw_id().to_string().as_str()));
                member.push_attribute(("role", m.role.as_str()));
                xml.write_event(Event::Empty(member))?;
            }
            for (k, v) in rel.tags.iter() {
                let mut tag = BytesStart::new("tag");
                tag.push_attribute(("k", k));
                tag.push_attribute(("v", v));
                xml.write_event(Event::Empty(tag))?;
            }
            xml.write_event(Event::End(BytesEnd::new("relation")))?;
        }
    }

    xml.write_event(Event::End(BytesEnd::new("osm")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const SAMPLE_OSM_XML: &str = r#"<?xml version='1.0' encoding='UTF-8'?>
<osm version="0.6" generator="test">
  <bounds minlat="50.0" minlon="10.0" maxlat="51.0" maxlon="11.0"/>
  <node id="1" lat="50.123" lon="10.456" version="2" user="alice" uid="42">
    <tag k="name" v="Sample Node"/>
    <tag k="amenity" v="cafe"/>
  </node>
  <node id="2" lat="50.124" lon="10.457" version="1"/>
  <way id="100" version="1">
    <nd ref="1"/>
    <nd ref="2"/>
    <tag k="highway" v="residential"/>
  </way>
  <relation id="1000" version="1">
    <member type="way" ref="100" role="outer"/>
    <tag k="type" v="multipolygon"/>
  </relation>
</osm>"#;

    #[test]
    fn test_read_osm_xml() {
        let cursor = Cursor::new(SAMPLE_OSM_XML);
        let dataset = read_osm_xml(cursor).expect("failed to parse OSM XML");

        assert_eq!(dataset.node_count(), 2);
        assert_eq!(dataset.way_count(), 1);
        assert_eq!(dataset.relation_count(), 1);

        let n1 = dataset.get_node(NodeId(1)).unwrap();
        assert_eq!(n1.position.lat(), 50.123);
        assert_eq!(n1.position.lon(), 10.456);
        assert_eq!(n1.tags.get("name"), Some("Sample Node"));
        assert_eq!(n1.tags.get("amenity"), Some("cafe"));
        assert_eq!(n1.metadata.user.as_deref(), Some("alice"));
        assert_eq!(n1.metadata.uid, Some(42));

        let w100 = dataset.get_way(WayId(100)).unwrap();
        assert_eq!(w100.nodes, vec![NodeId(1), NodeId(2)]);
        assert_eq!(w100.tags.get("highway"), Some("residential"));

        let r1000 = dataset.get_relation(RelationId(1000)).unwrap();
        assert_eq!(r1000.members.len(), 1);
        assert_eq!(r1000.members[0].member, PrimitiveId::Way(WayId(100)));
        assert_eq!(r1000.members[0].role, "outer");
    }

    #[test]
    fn test_roundtrip_osm_xml() {
        let cursor = Cursor::new(SAMPLE_OSM_XML);
        let dataset = read_osm_xml(cursor).unwrap();

        let mut output = Vec::new();
        write_osm_xml(&dataset, &mut output).unwrap();

        // Parse back the written XML
        let dataset2 = read_osm_xml(Cursor::new(output)).unwrap();

        assert_eq!(dataset2.node_count(), dataset.node_count());
        assert_eq!(dataset2.way_count(), dataset.way_count());
        assert_eq!(dataset2.relation_count(), dataset.relation_count());

        let n1 = dataset2.get_node(NodeId(1)).unwrap();
        assert_eq!(n1.tags.get("name"), Some("Sample Node"));
    }
}
