//! Application state managing the active editing session, tools, commands, and dataset.

use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

use rosm_core::command::*;
use rosm_core::geo::LatLon;
use rosm_core::id::{NodeId, PrimitiveId, WayId};
use rosm_core::primitive::{Node, OsmPrimitive, Way};
use rosm_core::validator::{validate, ValidationIssue};
use rosm_core::DataSet;
use rosm_io::{read_osm_xml, write_osm_xml};

use crate::viewport::Viewport;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolMode {
    Select,
    DrawWay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other,
}

pub struct AppState {
    pub dataset: DataSet,
    pub command_stack: CommandStack,
    pub viewport: Viewport,
    pub active_tool: ToolMode,
    pub selected_primitive: Option<PrimitiveId>,
    pub drawing_nodes: Vec<NodeId>,
    pub mouse_geo: Option<LatLon>,
    pub is_panning: bool,
    pub last_mouse_pos: Option<(f32, f32)>,
    pub dragged_node: Option<NodeId>,
    pub file_path: Option<PathBuf>,
    pub validation_issues: Vec<ValidationIssue>,
    pub status_message: String,
    pub tile_manager: rosm_imagery::TileManager,
}

impl Default for AppState {
    fn default() -> Self {
        let mut state = Self {
            dataset: DataSet::new(),
            command_stack: CommandStack::new(),
            viewport: Viewport::new(800, 600),
            active_tool: ToolMode::Select,
            selected_primitive: None,
            drawing_nodes: Vec::new(),
            mouse_geo: None,
            is_panning: false,
            last_mouse_pos: None,
            dragged_node: None,
            file_path: None,
            validation_issues: Vec::new(),
            status_message: "Ready".to_string(),
            tile_manager: rosm_imagery::TileManager::with_default_cache(),
        };

        state.load_sample_map();
        state
    }
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads initial sample data representing a city center (streets, buildings, bus stops).
    pub fn load_sample_map(&mut self) {
        let mut ds = DataSet::new();

        // Central street: Regent Street
        let n1 = Node::new(1, LatLon::new(51.5098, -0.1360));
        let n2 = Node::new(2, LatLon::new(51.5110, -0.1385));
        let n3 = Node::new(3, LatLon::new(51.5125, -0.1410));
        let n4 = Node::new(4, LatLon::new(51.5140, -0.1425));

        ds.insert_node(n1);
        ds.insert_node(n2);
        ds.insert_node(n3);
        ds.insert_node(n4);

        let mut way1 = Way::new(101, vec![NodeId(1), NodeId(2), NodeId(3), NodeId(4)]);
        way1.tags.insert("highway", "primary");
        way1.tags.insert("name", "Regent Street");
        ds.insert_way(way1);

        // Cross street: Oxford Street
        let n5 = Node::new(5, LatLon::new(51.5152, -0.1480));
        let n6 = Node::new(6, LatLon::new(51.5135, -0.1410)); // Connected to Regent St near n3
        let n7 = Node::new(7, LatLon::new(51.5120, -0.1340));

        ds.insert_node(n5);
        ds.insert_node(n6);
        ds.insert_node(n7);

        let mut way2 = Way::new(102, vec![NodeId(5), NodeId(6), NodeId(7)]);
        way2.tags.insert("highway", "primary");
        way2.tags.insert("name", "Oxford Street");
        ds.insert_way(way2);

        // Building
        let b1 = Node::new(10, LatLon::new(51.5105, -0.1345));
        let b2 = Node::new(11, LatLon::new(51.5115, -0.1345));
        let b3 = Node::new(12, LatLon::new(51.5115, -0.1330));
        let b4 = Node::new(13, LatLon::new(51.5105, -0.1330));

        ds.insert_node(b1);
        ds.insert_node(b2);
        ds.insert_node(b3);
        ds.insert_node(b4);

        let mut b_way = Way::new(
            201,
            vec![NodeId(10), NodeId(11), NodeId(12), NodeId(13), NodeId(10)],
        );
        b_way.tags.insert("building", "yes");
        b_way.tags.insert("name", "Piccadilly Plaza");
        ds.insert_way(b_way);

        // Amenity POI
        let mut cafe = Node::new(30, LatLon::new(51.5118, -0.1370));
        cafe.tags.insert("amenity", "cafe");
        cafe.tags.insert("name", "The London Roaster");
        ds.insert_node(cafe);

        self.dataset = ds;
        self.command_stack = CommandStack::new();
        self.fit_extent();
        self.status_message = "Loaded sample map".to_string();
    }

    /// Fits camera viewport to enclose all nodes in the dataset.
    pub fn fit_extent(&mut self) {
        if let Some(bounds) = self.dataset.bounds() {
            self.viewport.fit_bounds(&bounds);
        }
    }

    /// Opens and parses an OSM XML file.
    pub fn open_file(&mut self, path: impl AsRef<Path>) -> Result<(), String> {
        let p = path.as_ref();
        let file = File::open(p).map_err(|e| format!("Cannot open file: {e}"))?;
        let reader = BufReader::new(file);
        let dataset = read_osm_xml(reader).map_err(|e| format!("XML parse error: {e}"))?;

        self.dataset = dataset;
        self.command_stack = CommandStack::new();
        self.selected_primitive = None;
        self.drawing_nodes.clear();
        self.file_path = Some(p.to_path_buf());
        self.fit_extent();
        self.run_validator();
        self.status_message = format!(
            "Loaded {}",
            p.file_name().unwrap_or_default().to_string_lossy()
        );
        Ok(())
    }

    /// Saves dataset to the given or current file path in OSM XML format.
    pub fn save_file(&mut self, path: Option<PathBuf>) -> Result<(), String> {
        let target_path = path
            .or_else(|| self.file_path.clone())
            .ok_or("No file path specified")?;
        let file = File::create(&target_path).map_err(|e| format!("Cannot write file: {e}"))?;
        let writer = BufWriter::new(file);
        write_osm_xml(&self.dataset, writer).map_err(|e| format!("XML write error: {e}"))?;

        self.command_stack.mark_saved();
        self.file_path = Some(target_path.clone());
        self.status_message = format!(
            "Saved {}",
            target_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        );
        Ok(())
    }

    pub fn undo(&mut self) -> Result<String, String> {
        self.command_stack
            .undo(&mut self.dataset)
            .map_err(|e| e.to_string())
    }

    pub fn redo(&mut self) -> Result<String, String> {
        self.command_stack
            .redo(&mut self.dataset)
            .map_err(|e| e.to_string())
    }

    pub fn delete_selected(&mut self) {
        if let Some(prim_id) = self.selected_primitive.take() {
            let cmd = Box::new(DeletePrimitiveCommand::new(prim_id));
            if let Err(e) = self.command_stack.execute(cmd, &mut self.dataset) {
                self.status_message = format!("Delete error: {e}");
            } else {
                self.status_message = format!("Deleted {prim_id}");
            }
        }
    }

    pub fn split_selected_way(&mut self) {
        // Splitting requires selecting a way and a node that belongs to it
        if let Some(PrimitiveId::Node(nid)) = self.selected_primitive {
            let way_id = self.dataset.ways_using_node(nid).first().map(|w| w.id);
            if let Some(wid) = way_id {
                let cmd = Box::new(SplitWayCommand::new(wid, nid));
                if let Err(e) = self.command_stack.execute(cmd, &mut self.dataset) {
                    self.status_message = format!("Split error: {e}");
                } else {
                    self.status_message = format!("Split way {wid} at node {nid}");
                }
            } else {
                self.status_message = "Selected node is not part of any way".into();
            }
        } else {
            self.status_message = "Select a node along a way to split".into();
        }
    }

    pub fn reverse_selected_way(&mut self) {
        if let Some(PrimitiveId::Way(wid)) = self.selected_primitive {
            let cmd = Box::new(ReverseWayCommand::new(wid));
            if let Err(e) = self.command_stack.execute(cmd, &mut self.dataset) {
                self.status_message = format!("Reverse error: {e}");
            } else {
                self.status_message = format!("Reversed way {wid}");
            }
        } else {
            self.status_message = "Select a way to reverse".into();
        }
    }

    pub fn add_or_update_tag(&mut self, key: &str, value: &str) {
        let Some(prim_id) = self.selected_primitive else {
            return;
        };
        let mut tags = match prim_id {
            PrimitiveId::Node(nid) => self.dataset.get_node(nid).map(|n| n.tags.clone()),
            PrimitiveId::Way(wid) => self.dataset.get_way(wid).map(|w| w.tags.clone()),
            PrimitiveId::Relation(rid) => self.dataset.get_relation(rid).map(|r| r.tags.clone()),
        }
        .unwrap_or_default();

        tags.insert(key, value);
        let cmd = Box::new(ChangeTagsCommand::new(prim_id, tags));
        let _ = self.command_stack.execute(cmd, &mut self.dataset);
    }

    pub fn remove_tag(&mut self, key: &str) {
        let Some(prim_id) = self.selected_primitive else {
            return;
        };
        let mut tags = match prim_id {
            PrimitiveId::Node(nid) => self.dataset.get_node(nid).map(|n| n.tags.clone()),
            PrimitiveId::Way(wid) => self.dataset.get_way(wid).map(|w| w.tags.clone()),
            PrimitiveId::Relation(rid) => self.dataset.get_relation(rid).map(|r| r.tags.clone()),
        }
        .unwrap_or_default();

        tags.remove(key);
        let cmd = Box::new(ChangeTagsCommand::new(prim_id, tags));
        let _ = self.command_stack.execute(cmd, &mut self.dataset);
    }

    pub fn run_validator(&mut self) {
        self.validation_issues = validate(&self.dataset);
    }

    // --- Interactive Mouse Handlers ---

    pub fn on_mouse_down(&mut self, x: f32, y: f32, button: MouseButton) {
        self.last_mouse_pos = Some((x, y));

        if button == MouseButton::Right || button == MouseButton::Middle {
            self.is_panning = true;
            return;
        }

        if button == MouseButton::Left {
            let click_geo = self.viewport.screen_to_latlon(x, y);

            match self.active_tool {
                ToolMode::Select => {
                    // Check if clicked an existing node
                    if let Some((nid, _dist)) = self.find_node_near(x, y, 10.0) {
                        self.selected_primitive = Some(PrimitiveId::Node(nid));
                        self.dragged_node = Some(nid);
                    } else if let Some(wid) = self.find_way_near(x, y, 8.0) {
                        self.selected_primitive = Some(PrimitiveId::Way(wid));
                    } else {
                        // Clicked empty canvas: start panning or clear selection
                        self.selected_primitive = None;
                        self.is_panning = true;
                    }
                }
                ToolMode::DrawWay => {
                    // If near an existing node, connect to it
                    let node_id = if let Some((near_id, _)) = self.find_node_near(x, y, 12.0) {
                        near_id
                    } else {
                        // Create a new local node
                        let new_id = self.dataset.allocate_node_id();
                        let node = Node::new(new_id, click_geo);
                        let cmd = Box::new(AddPrimitiveCommand::new(OsmPrimitive::Node(node)));
                        let _ = self.command_stack.execute(cmd, &mut self.dataset);
                        new_id
                    };

                    self.drawing_nodes.push(node_id);
                    self.status_message =
                        format!("Drawing way: {} nodes placed", self.drawing_nodes.len());
                }
            }
        }
    }

    pub fn on_mouse_move(&mut self, x: f32, y: f32) {
        let mouse_geo = self.viewport.screen_to_latlon(x, y);
        self.mouse_geo = Some(mouse_geo);

        if self.is_panning {
            if let Some((last_x, last_y)) = self.last_mouse_pos {
                let dx = x - last_x;
                let dy = y - last_y;
                self.viewport.pan(dx, dy);
            }
        } else if let Some(dragged_id) = self.dragged_node {
            if let Some(node) = self.dataset.get_node_mut(dragged_id) {
                node.position = mouse_geo;
            }
        }

        self.last_mouse_pos = Some((x, y));
    }

    pub fn on_mouse_up(&mut self, x: f32, y: f32, _button: MouseButton) {
        self.is_panning = false;

        if let Some(dragged_id) = self.dragged_node.take() {
            let final_pos = self.viewport.screen_to_latlon(x, y);
            let cmd = Box::new(MoveNodeCommand::new(dragged_id, final_pos));
            let _ = self.command_stack.execute(cmd, &mut self.dataset);
        }

        self.last_mouse_pos = None;
    }

    pub fn on_scroll(&mut self, x: f32, y: f32, delta_y: f32) {
        let factor = if delta_y > 0.0 { 1.2 } else { 1.0 / 1.2 };
        self.viewport.zoom_at(factor, x, y);
    }

    /// Finishes the current way being drawn and records it in command history.
    pub fn finish_drawing(&mut self) {
        if self.drawing_nodes.len() >= 2 {
            let new_way_id = self.dataset.allocate_way_id();
            let mut way = Way::new(new_way_id, self.drawing_nodes.clone());
            way.tags.insert("highway", "residential");

            let cmd = Box::new(AddPrimitiveCommand::new(OsmPrimitive::Way(way)));
            let _ = self.command_stack.execute(cmd, &mut self.dataset);

            self.selected_primitive = Some(PrimitiveId::Way(new_way_id));
            self.status_message = format!("Created way {new_way_id}");
        }
        self.drawing_nodes.clear();
        self.active_tool = ToolMode::Select;
    }

    pub fn cancel_drawing(&mut self) {
        self.drawing_nodes.clear();
        self.active_tool = ToolMode::Select;
        self.status_message = "Canceled drawing".into();
    }

    // --- Spatial hit testing ---

    fn find_node_near(
        &self,
        screen_x: f32,
        screen_y: f32,
        max_pixel_dist: f32,
    ) -> Option<(NodeId, f32)> {
        let mut best: Option<(NodeId, f32)> = None;

        for node in self.dataset.nodes() {
            let (nx, ny) = self.viewport.latlon_to_screen(node.position);
            let dist = ((nx - screen_x).powi(2) + (ny - screen_y).powi(2)).sqrt();
            if dist <= max_pixel_dist {
                match best {
                    None => best = Some((node.id, dist)),
                    Some((_, cur_min)) if dist < cur_min => best = Some((node.id, dist)),
                    _ => {}
                }
            }
        }

        best
    }

    fn find_way_near(&self, screen_x: f32, screen_y: f32, max_pixel_dist: f32) -> Option<WayId> {
        let mut best: Option<(WayId, f32)> = None;

        for way in self.dataset.ways() {
            for pair in way.nodes.windows(2) {
                if let (Some(n1), Some(n2)) = (
                    self.dataset.get_node(pair[0]),
                    self.dataset.get_node(pair[1]),
                ) {
                    let (x1, y1) = self.viewport.latlon_to_screen(n1.position);
                    let (x2, y2) = self.viewport.latlon_to_screen(n2.position);
                    let dist = distance_to_segment(screen_x, screen_y, x1, y1, x2, y2);
                    if dist <= max_pixel_dist {
                        match best {
                            None => best = Some((way.id, dist)),
                            Some((_, cur_min)) if dist < cur_min => best = Some((way.id, dist)),
                            _ => {}
                        }
                    }
                }
            }
        }

        best.map(|(wid, _)| wid)
    }

    pub fn set_imagery_source(&mut self, source_id: &str) {
        match source_id {
            "osm_carto" => {
                self.tile_manager.set_source(Some(rosm_imagery::OSM_CARTO));
                self.tile_manager.set_enabled(true);
                self.status_message = "Imagery: OpenStreetMap Standard".to_string();
            }
            "esri_world_imagery" => {
                self.tile_manager
                    .set_source(Some(rosm_imagery::ESRI_WORLD_IMAGERY));
                self.tile_manager.set_enabled(true);
                self.status_message = "Imagery: Esri World Imagery (Satellite)".to_string();
            }
            "opentopomap" => {
                self.tile_manager
                    .set_source(Some(rosm_imagery::OPEN_TOPO_MAP));
                self.tile_manager.set_enabled(true);
                self.status_message = "Imagery: OpenTopoMap".to_string();
            }
            "hot_osm" => {
                self.tile_manager.set_source(Some(rosm_imagery::HOT_OSM));
                self.tile_manager.set_enabled(true);
                self.status_message = "Imagery: Humanitarian OSM (HOT)".to_string();
            }
            "none" => {
                self.tile_manager.set_source(None);
                self.status_message = "Imagery: Off".to_string();
            }
            _ => {}
        }
    }

    pub fn set_imagery_opacity(&mut self, opacity: f32) {
        self.tile_manager.set_opacity(opacity);
        self.status_message = format!(
            "Imagery opacity: {:.0}%",
            self.tile_manager.opacity() * 100.0
        );
    }

    pub fn toggle_imagery(&mut self) {
        let new_val = !self.tile_manager.is_enabled();
        self.tile_manager.set_enabled(new_val);
        self.status_message = if new_val {
            "Imagery: Enabled".to_string()
        } else {
            "Imagery: Disabled".to_string()
        };
    }
}

fn distance_to_segment(px: f32, py: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len_sq = dx * dx + dy * dy;

    if len_sq == 0.0 {
        return ((px - x1).powi(2) + (py - y1).powi(2)).sqrt();
    }

    let t = (((px - x1) * dx + (py - y1) * dy) / len_sq).clamp(0.0, 1.0);
    let proj_x = x1 + t * dx;
    let proj_y = y1 + t * dy;

    ((px - proj_x).powi(2) + (py - proj_y).powi(2)).sqrt()
}
