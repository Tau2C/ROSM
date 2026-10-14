use rosm_core::command::*;
use rosm_core::geo::LatLon;
use rosm_core::id::{NodeId, PrimitiveId, WayId};
use rosm_core::primitive::{Node, OsmPrimitive, Way};
use rosm_core::tags::Tags;
use rosm_core::validator::validate;
use rosm_core::DataSet;

#[test]
fn test_full_josm_editing_scenario() {
    let mut ds = DataSet::new();
    let mut stack = CommandStack::new();

    // 1. Create a road with 4 nodes: n1 -> n2 -> n3 -> n4
    let mut batch = SequenceCommand::new("Create road Baker Street");
    let nodes = [
        (NodeId(1), LatLon::new(51.5237, -0.1585)),
        (NodeId(2), LatLon::new(51.5240, -0.1580)),
        (NodeId(3), LatLon::new(51.5245, -0.1575)),
        (NodeId(4), LatLon::new(51.5250, -0.1570)),
    ];

    for (id, pos) in nodes {
        batch.push(Box::new(AddPrimitiveCommand::new(OsmPrimitive::Node(
            Node::new(id, pos),
        ))));
    }

    let mut way = Way::new(WayId(101), vec![NodeId(1), NodeId(2), NodeId(3), NodeId(4)]);
    way.tags.insert("highway", "secondary");
    way.tags.insert("name", "Baker Street");
    batch.push(Box::new(AddPrimitiveCommand::new(OsmPrimitive::Way(way))));

    stack.execute(Box::new(batch), &mut ds).unwrap();

    assert_eq!(ds.node_count(), 4);
    assert_eq!(ds.way_count(), 1);
    assert!(
        validate(&ds).is_empty(),
        "clean road should pass validation"
    );

    // 2. Add a building next to the road
    let b_nodes = [
        (NodeId(10), LatLon::new(51.5238, -0.1570)),
        (NodeId(11), LatLon::new(51.5242, -0.1570)),
        (NodeId(12), LatLon::new(51.5242, -0.1565)),
        (NodeId(13), LatLon::new(51.5238, -0.1565)),
    ];
    for (id, pos) in b_nodes {
        ds.insert_node(Node::new(id, pos));
    }
    let mut building = Way::new(
        WayId(202),
        vec![NodeId(10), NodeId(11), NodeId(12), NodeId(13), NodeId(10)],
    );
    building.tags.insert("building", "yes");
    building.tags.insert("addr:housenumber", "221B");
    building.tags.insert("addr:street", "Baker Street");
    ds.insert_way(building);

    assert_eq!(ds.way_count(), 2);
    let b = ds.get_way(WayId(202)).unwrap();
    assert!(b.is_closed());
    assert!(b.is_area());

    // 3. Split road at n2 (an internal node)
    let split = Box::new(SplitWayCommand::new(WayId(101), NodeId(2)));
    stack.execute(split, &mut ds).unwrap();

    assert_eq!(ds.way_count(), 3);
    let orig = ds.get_way(WayId(101)).unwrap();
    assert_eq!(orig.nodes, vec![NodeId(1), NodeId(2)]);
    assert_eq!(orig.tags.get("name"), Some("Baker Street"));

    // Find the new way created by split
    let split_way = ds
        .ways()
        .find(|w| w.id != WayId(101) && w.id != WayId(202))
        .unwrap();
    assert_eq!(split_way.nodes, vec![NodeId(2), NodeId(3), NodeId(4)]);
    assert_eq!(split_way.tags.get("highway"), Some("secondary"));

    // 4. Test validation issues when invalid elements are introduced
    let dangling_way = Way::new(WayId(303), vec![NodeId(1), NodeId(999)]);
    ds.insert_way(dangling_way);
    let issues = validate(&ds);
    assert!(issues.iter().any(|i| i.rule == "DanglingNodeReference"));
    ds.remove_way(WayId(303));

    // 5. Undo the split
    stack.undo(&mut ds).unwrap();
    assert_eq!(ds.way_count(), 2);
    let restored_road = ds.get_way(WayId(101)).unwrap();
    assert_eq!(
        restored_road.nodes,
        vec![NodeId(1), NodeId(2), NodeId(3), NodeId(4)]
    );

    // 6. Test Move Node
    let old_pos = ds.get_node(NodeId(1)).unwrap().position;
    let new_pos = LatLon::new(51.5235, -0.1590);
    stack
        .execute(Box::new(MoveNodeCommand::new(NodeId(1), new_pos)), &mut ds)
        .unwrap();
    assert_eq!(ds.get_node(NodeId(1)).unwrap().position, new_pos);

    // Undo Move Node
    stack.undo(&mut ds).unwrap();
    assert_eq!(ds.get_node(NodeId(1)).unwrap().position, old_pos);

    // 7. Test Tag Changes
    let mut updated_tags = Tags::new();
    updated_tags.insert("highway", "primary");
    updated_tags.insert("name", "Baker Street Boulevard");
    stack
        .execute(
            Box::new(ChangeTagsCommand::new(
                PrimitiveId::Way(WayId(101)),
                updated_tags,
            )),
            &mut ds,
        )
        .unwrap();
    assert_eq!(
        ds.get_way(WayId(101)).unwrap().tags.get("highway"),
        Some("primary")
    );

    stack.undo(&mut ds).unwrap();
    assert_eq!(
        ds.get_way(WayId(101)).unwrap().tags.get("highway"),
        Some("secondary")
    );
}
