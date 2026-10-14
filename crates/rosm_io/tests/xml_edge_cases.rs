use rosm_core::geo::LatLon;
use rosm_core::id::{NodeId, PrimitiveId, RelationId, WayId};
use rosm_core::primitive::{Node, Relation, RelationMember, Way};
use rosm_core::DataSet;
use rosm_io::{read_osm_xml, write_osm_xml};
use std::io::Cursor;

#[test]
fn test_unicode_and_special_characters_in_tags() {
    let mut ds = DataSet::new();

    let mut n = Node::new(NodeId(1), LatLon::new(51.7687, 19.4570));
    n.tags.insert("name", "Łódź Fabryczna & Główna <Station>");
    n.tags.insert("name:ja", "ウッチ・ファブリチナ駅");
    n.tags
        .insert("description", "Quotes \"test\" and 'single' & ampersand");
    ds.insert_node(n);

    let mut out = Vec::new();
    write_osm_xml(&ds, &mut out).expect("write failed");

    let read_ds = read_osm_xml(Cursor::new(out)).expect("read failed");
    let read_node = read_ds.get_node(NodeId(1)).expect("node 1 missing");

    assert_eq!(
        read_node.tags.get("name"),
        Some("Łódź Fabryczna & Główna <Station>")
    );
    assert_eq!(
        read_node.tags.get("name:ja"),
        Some("ウッチ・ファブリチナ駅")
    );
    assert_eq!(
        read_node.tags.get("description"),
        Some("Quotes \"test\" and 'single' & ampersand")
    );
}

#[test]
fn test_negative_ids_for_local_edits() {
    let mut ds = DataSet::new();
    let n1 = NodeId(-10);
    let n2 = NodeId(-20);

    ds.insert_node(Node::new(n1, LatLon::new(10.0, 10.0)));
    ds.insert_node(Node::new(n2, LatLon::new(10.1, 10.1)));

    let mut way = Way::new(WayId(-100), vec![n1, n2]);
    way.tags.insert("highway", "path");
    ds.insert_way(way);

    let rel_member = RelationMember::new(PrimitiveId::Way(WayId(-100)), "outer");
    let rel = Relation::new(RelationId(-1000), vec![rel_member]);
    ds.insert_relation(rel);

    let mut out = Vec::new();
    write_osm_xml(&ds, &mut out).expect("write failed");

    let read_ds = read_osm_xml(Cursor::new(out)).expect("read failed");
    assert!(read_ds.contains_node(n1));
    assert!(read_ds.contains_way(WayId(-100)));
    assert!(read_ds.contains_relation(RelationId(-1000)));

    let r = read_ds.get_relation(RelationId(-1000)).unwrap();
    assert_eq!(r.members[0].member, PrimitiveId::Way(WayId(-100)));
}
