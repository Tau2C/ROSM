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
    pub dragged_node_initial_pos: Option<LatLon>,
    pub file_path: Option<PathBuf>,
    pub validation_issues: Vec<ValidationIssue>,
    pub status_message: String,
    pub tile_manager: rosm_imagery::TileManager,
    pub current_mouse_pos: Option<(f32, f32)>,
    pub layer_stack: crate::layers::LayerStack,
    pub osm_client: rosm_net::OsmClient,
    pub overpass_client: rosm_net::OverpassClient,
    pub base_dataset: DataSet,
    pub osm_auth_token: Option<String>,
    pub is_production_server: bool,
    pub pending_created: usize,
    pub pending_modified: usize,
    pub pending_deleted: usize,
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
            dragged_node_initial_pos: None,
            file_path: None,
            validation_issues: Vec::new(),
            status_message: "Ready".to_string(),
            tile_manager: rosm_imagery::TileManager::with_default_cache(),
            current_mouse_pos: None,
            layer_stack: crate::layers::LayerStack::standard_stack(),
            osm_client: rosm_net::OsmClient::new(rosm_net::OsmEnvironment::Dev),
            overpass_client: rosm_net::OverpassClient::default(),
            base_dataset: DataSet::new(),
            osm_auth_token: None,
            is_production_server: false,
            pending_created: 0,
            pending_modified: 0,
            pending_deleted: 0,
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

        self.dataset = ds.clone();
        self.base_dataset = ds;
        self.command_stack = CommandStack::new();
        self.fit_extent();
        self.update_diff_counts();
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

        self.dataset = dataset.clone();
        self.base_dataset = dataset;
        self.command_stack = CommandStack::new();
        self.selected_primitive = None;
        self.drawing_nodes.clear();
        self.file_path = Some(p.to_path_buf());
        self.fit_extent();
        self.run_validator();
        self.update_diff_counts();
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
                        let is_already_selected =
                            self.selected_primitive == Some(PrimitiveId::Node(nid));
                        self.selected_primitive = Some(PrimitiveId::Node(nid));

                        // To move a node it needs to be selected first. Block moving when selecting.
                        if is_already_selected {
                            self.dragged_node = Some(nid);
                            self.dragged_node_initial_pos =
                                self.dataset.get_node(nid).map(|n| n.position);
                        } else {
                            self.dragged_node = None;
                            self.dragged_node_initial_pos = None;
                        }
                    } else if let Some(wid) = self.find_way_near(x, y, 8.0) {
                        self.selected_primitive = Some(PrimitiveId::Way(wid));
                        self.dragged_node = None;
                        self.dragged_node_initial_pos = None;
                    } else {
                        // Clicked empty canvas: start panning or clear selection
                        self.selected_primitive = None;
                        self.dragged_node = None;
                        self.dragged_node_initial_pos = None;
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
        self.current_mouse_pos = Some((x, y));

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
        self.current_mouse_pos = Some((x, y));

        if let Some(dragged_id) = self.dragged_node.take() {
            let initial_pos = self.dragged_node_initial_pos.take();
            let final_pos = self.viewport.screen_to_latlon(x, y);

            let moved = initial_pos
                .map(|p| {
                    (p.lat() - final_pos.lat()).abs() > 1e-7
                        || (p.lon() - final_pos.lon()).abs() > 1e-7
                })
                .unwrap_or(true);

            if moved {
                // Restore node position to initial_pos prior to executing MoveNodeCommand
                // so that MoveNodeCommand accurately records the old position for undo
                if let (Some(init), Some(node)) =
                    (initial_pos, self.dataset.get_node_mut(dragged_id))
                {
                    node.position = init;
                }
                let cmd = Box::new(MoveNodeCommand::new(dragged_id, final_pos));
                let _ = self.command_stack.execute(cmd, &mut self.dataset);
            }
        } else {
            self.dragged_node_initial_pos = None;
        }

        self.last_mouse_pos = None;
    }

    pub fn on_scroll(&mut self, x: f32, y: f32, delta_y: f32) {
        self.current_mouse_pos = Some((x, y));
        let factor = if delta_y > 0.0 { 1.2 } else { 1.0 / 1.2 };
        self.viewport.zoom_at(factor, x, y);
    }

    /// Determines the anchor position for zooming (cursor position if over canvas, otherwise center).
    pub fn current_mouse_anchor(&self) -> (f32, f32) {
        if let Some((mx, my)) = self.current_mouse_pos {
            if mx >= 0.0
                && mx <= self.viewport.width as f32
                && my >= 0.0
                && my <= self.viewport.height as f32
            {
                return (mx, my);
            }
        }
        (
            self.viewport.width as f32 / 2.0,
            self.viewport.height as f32 / 2.0,
        )
    }

    /// Zooms in anchored around the current mouse position.
    pub fn zoom_in(&mut self) {
        let (cx, cy) = self.current_mouse_anchor();
        self.viewport.zoom_at(1.3, cx, cy);
    }

    /// Zooms out anchored around the current mouse position.
    pub fn zoom_out(&mut self) {
        let (cx, cy) = self.current_mouse_anchor();
        self.viewport.zoom_at(1.0 / 1.3, cx, cy);
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

    // --- Layer Operations ---

    pub fn move_layer_up(&mut self, idx: usize) {
        if self.layer_stack.move_up(idx) {
            self.status_message = format!("Moved layer {} up in stack", idx + 1);
        }
    }

    pub fn move_layer_down(&mut self, idx: usize) {
        if self.layer_stack.move_down(idx) {
            self.status_message = format!("Moved layer {} down in stack", idx + 1);
        }
    }

    pub fn toggle_layer_visibility(&mut self, idx: usize) {
        if self.layer_stack.toggle_visibility(idx) {
            if let Some(l) = self.layer_stack.get(idx) {
                self.status_message = format!(
                    "Layer '{}' is now {}",
                    l.name,
                    if l.visible { "visible" } else { "hidden" }
                );
            }
        }
    }

    pub fn set_layer_opacity(&mut self, idx: usize, opacity: f32) {
        if self.layer_stack.set_opacity(idx, opacity) {
            if let Some(l) = self.layer_stack.get(idx) {
                self.status_message =
                    format!("Layer '{}' opacity: {:.0}%", l.name, opacity * 100.0);
            }
        }
    }

    pub fn set_layer_source(&mut self, idx: usize, source_id: &str) {
        if let Some(source) = self.tile_manager.registry().get(source_id).cloned() {
            if self
                .layer_stack
                .set_source_id(idx, &source.id, &source.name)
            {
                self.status_message = format!("Layer source changed to {}", source.name);
            }
        }
    }

    pub fn add_layer_from_source(&mut self, source_id: &str, is_overlay: bool) {
        if let Some(source) = self.tile_manager.registry().get(source_id).cloned() {
            let id = format!("layer_{}_{}", source.id, self.layer_stack.len());
            let layer = if is_overlay {
                crate::layers::MapLayer::new_overlay(
                    id,
                    format!("{} (Overlay)", source.name),
                    &source.id,
                    0.7,
                )
            } else {
                crate::layers::MapLayer::new_background(
                    id,
                    format!("Background: {}", source.name),
                    &source.id,
                )
            };
            self.layer_stack.add_layer(layer);
            self.status_message = format!("Added layer '{}'", source.name);
        }
    }

    pub fn remove_layer(&mut self, idx: usize) {
        if let Some(removed) = self.layer_stack.remove_layer(idx) {
            self.status_message = format!("Removed layer '{}'", removed.name);
        } else {
            self.status_message = "Cannot delete the primary OSM data layer".to_string();
        }
    }

    // --- Dynamic Source Operations ---

    pub fn add_custom_source(
        &mut self,
        id: &str,
        name: &str,
        url_template: &str,
        min_zoom: u8,
        max_zoom: u8,
        is_overlay: bool,
    ) -> bool {
        let source = rosm_imagery::ImagerySource::new(
            id,
            name,
            url_template,
            min_zoom,
            max_zoom,
            "Custom User Source",
            is_overlay,
        );
        let ok = self.tile_manager.registry_mut().add(source);
        if ok {
            self.status_message = format!("Added imagery source '{name}'");
        } else {
            self.status_message = format!("Source ID '{id}' already exists");
        }
        ok
    }

    #[allow(dead_code)]
    pub fn update_custom_source(
        &mut self,
        id: &str,
        name: &str,
        url_template: &str,
        min_zoom: u8,
        max_zoom: u8,
        is_overlay: bool,
    ) -> bool {
        let source = rosm_imagery::ImagerySource::new(
            id,
            name,
            url_template,
            min_zoom,
            max_zoom,
            "Custom User Source",
            is_overlay,
        );
        let ok = self.tile_manager.registry_mut().update(id, source);
        if ok {
            self.status_message = format!("Updated imagery source '{name}'");
        }
        ok
    }

    pub fn remove_custom_source(&mut self, id: &str) -> bool {
        let ok = self.tile_manager.registry_mut().remove(id).is_some();
        if ok {
            self.status_message = format!("Removed imagery source '{id}'");
        }
        ok
    }

    // --- OSM Network & Changeset Operations ---

    /// Recomputes pending change statistics comparing base_dataset to dataset.
    pub fn update_diff_counts(&mut self) {
        let diff = rosm_net::OsmDiff::compute(&self.base_dataset, &self.dataset);
        self.pending_created =
            diff.created_nodes.len() + diff.created_ways.len() + diff.created_relations.len();
        self.pending_modified =
            diff.modified_nodes.len() + diff.modified_ways.len() + diff.modified_relations.len();
        self.pending_deleted =
            diff.deleted_nodes.len() + diff.deleted_ways.len() + diff.deleted_relations.len();
    }

    /// Sets or clears the OAuth Bearer token for OSM API v0.6 authentication.
    pub fn set_auth_token(&mut self, token: &str) {
        let t = token.trim().to_string();
        if t.is_empty() {
            self.osm_auth_token = None;
            self.osm_client = rosm_net::OsmClient::new(self.osm_client.environment().clone());
            self.status_message = "Logged out from OSM API".into();
        } else {
            self.osm_auth_token = Some(t.clone());
            self.osm_client =
                rosm_net::OsmClient::new(self.osm_client.environment().clone()).with_auth(t);
            self.status_message = "Configured OSM API access token".into();
        }
    }

    /// Switches between production OSM server and development sandbox server.
    pub fn set_osm_environment(&mut self, is_production: bool) {
        self.is_production_server = is_production;
        let env = if is_production {
            rosm_net::OsmEnvironment::Production
        } else {
            rosm_net::OsmEnvironment::Dev
        };
        let mut client = rosm_net::OsmClient::new(env);
        if let Some(ref token) = self.osm_auth_token {
            client = client.with_auth(token.clone());
        }
        self.osm_client = client;
        self.status_message = format!(
            "OSM API server set to {}",
            if is_production {
                "Production"
            } else {
                "Dev Sandbox"
            }
        );
    }

    /// Downloads all vector primitives inside the current visible map viewport bounding box.
    pub fn download_current_bbox(&mut self) -> Result<usize, String> {
        let bbox = self.viewport.visible_bbox();
        match self.osm_client.download_bbox(&bbox) {
            Ok(ds) => {
                let node_count = ds.node_count();
                let way_count = ds.way_count();
                self.base_dataset = ds.clone();
                self.dataset = ds;
                self.command_stack = CommandStack::new();
                self.selected_primitive = None;
                self.drawing_nodes.clear();
                self.update_diff_counts();
                self.run_validator();
                self.status_message =
                    format!("Downloaded {node_count} nodes, {way_count} ways from OSM API");
                Ok(node_count + way_count)
            }
            Err(e) => {
                let msg = format!("Download failed: {e}");
                self.status_message = msg.clone();
                Err(msg)
            }
        }
    }

    /// Queries the Overpass API with custom QL and loads the result into the dataset.
    pub fn execute_overpass_query(&mut self, ql: &str) -> Result<usize, String> {
        match self.overpass_client.query(ql) {
            Ok(ds) => {
                let node_count = ds.node_count();
                let way_count = ds.way_count();
                self.base_dataset = ds.clone();
                self.dataset = ds;
                self.command_stack = CommandStack::new();
                self.selected_primitive = None;
                self.drawing_nodes.clear();
                self.fit_extent();
                self.update_diff_counts();
                self.run_validator();
                self.status_message =
                    format!("Overpass query returned {node_count} nodes, {way_count} ways");
                Ok(node_count + way_count)
            }
            Err(e) => {
                let msg = format!("Overpass query failed: {e}");
                self.status_message = msg.clone();
                Err(msg)
            }
        }
    }

    /// Uploads all pending creations, modifications, and deletions as an OSM changeset.
    pub fn upload_changeset(&mut self, comment: &str, source: Option<&str>) -> Result<i64, String> {
        let diff = rosm_net::OsmDiff::compute(&self.base_dataset, &self.dataset);
        if diff.is_empty() {
            let msg = "No pending changes to upload".to_string();
            self.status_message = msg.clone();
            return Err(msg);
        }

        let mut config = rosm_net::ChangesetConfig::new(comment);
        if let Some(src) = source {
            if !src.trim().is_empty() {
                config = config.with_source(src);
            }
        }

        let changeset_id = self
            .osm_client
            .create_changeset(&config)
            .map_err(|e| format!("Failed to create changeset: {e}"))?;

        let osc_xml = diff.to_osm_change_xml(changeset_id);

        let diff_result = match self.osm_client.upload_diff(changeset_id, &osc_xml) {
            Ok(dr) => dr,
            Err(e) => {
                let _ = self.osm_client.close_changeset(changeset_id);
                let msg = format!("Failed to upload diff: {e}");
                self.status_message = msg.clone();
                return Err(msg);
            }
        };

        // Remap negative IDs in local dataset to server IDs
        diff_result.apply_to_dataset(&mut self.dataset);

        // Close the changeset
        let _ = self.osm_client.close_changeset(changeset_id);

        // Update baseline dataset to match
        self.base_dataset = self.dataset.clone();
        self.update_diff_counts();
        self.command_stack.mark_saved();
        self.status_message = format!("Successfully uploaded Changeset #{changeset_id}");
        Ok(changeset_id)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zoom_in_anchors_at_mouse_position() {
        let mut state = AppState::new();
        state.viewport.resize(800, 600);
        let cursor_x = 600.0;
        let cursor_y = 200.0;

        state.on_mouse_move(cursor_x, cursor_y);
        let geo_under_cursor = state.viewport.screen_to_latlon(cursor_x, cursor_y);

        state.zoom_in();

        let (new_screen_x, new_screen_y) = state.viewport.latlon_to_screen(geo_under_cursor);
        assert!((new_screen_x - cursor_x).abs() < 1e-2);
        assert!((new_screen_y - cursor_y).abs() < 1e-2);
    }

    #[test]
    fn test_on_scroll_anchors_at_mouse_position() {
        let mut state = AppState::new();
        state.viewport.resize(800, 600);
        let cursor_x = 250.0;
        let cursor_y = 450.0;

        let geo_under_cursor = state.viewport.screen_to_latlon(cursor_x, cursor_y);
        state.on_scroll(cursor_x, cursor_y, 1.0); // zoom in

        let (new_screen_x, new_screen_y) = state.viewport.latlon_to_screen(geo_under_cursor);
        assert!((new_screen_x - cursor_x).abs() < 1e-2);
        assert!((new_screen_y - cursor_y).abs() < 1e-2);
    }

    #[test]
    fn test_zoom_falls_back_to_center_when_mouse_outside() {
        let mut state = AppState::new();
        state.viewport.resize(800, 600);
        state.current_mouse_pos = None;

        let anchor = state.current_mouse_anchor();
        assert_eq!(anchor, (400.0, 300.0));
    }

    #[test]
    fn test_app_state_layer_operations() {
        let mut state = AppState::new();
        let initial_count = state.layer_stack.len();

        // Add a layer from source
        state.add_layer_from_source("osm_carto", false);
        assert_eq!(state.layer_stack.len(), initial_count + 1);

        // Move layer down
        let new_idx = state.layer_stack.len() - 1;
        state.move_layer_down(new_idx);

        // Toggle layer visibility
        state.toggle_layer_visibility(0);
        assert!(!state.layer_stack.get(0).unwrap().visible);

        // Set opacity
        state.set_layer_opacity(0, 0.42);
        assert!((state.layer_stack.get(0).unwrap().opacity - 0.42).abs() < 1e-4);

        // Set layer source
        state.set_layer_source(0, "esri_world_imagery");
        assert_eq!(
            state.layer_stack.get(0).unwrap().source_id(),
            Some("esri_world_imagery")
        );

        // Remove layer
        state.remove_layer(state.layer_stack.len() - 1);
        assert_eq!(state.layer_stack.len(), initial_count);
    }

    #[test]
    fn test_app_state_custom_sources() {
        let mut state = AppState::new();

        // Add custom source
        assert!(state.add_custom_source(
            "custom_ortho",
            "Custom Orthophoto",
            "https://ortho.example.com/{z}/{x}/{y}.png",
            0,
            20,
            false
        ));
        assert!(state.tile_manager.registry().get("custom_ortho").is_some());

        // Update custom source
        assert!(state.update_custom_source(
            "custom_ortho",
            "Custom Orthophoto v2",
            "https://ortho2.example.com/{z}/{x}/{y}.png",
            1,
            21,
            false
        ));
        assert_eq!(
            state
                .tile_manager
                .registry()
                .get("custom_ortho")
                .unwrap()
                .name,
            "Custom Orthophoto v2"
        );

        // Remove custom source
        assert!(state.remove_custom_source("custom_ortho"));
        assert!(state.tile_manager.registry().get("custom_ortho").is_none());
    }

    #[test]
    fn test_node_cannot_be_moved_on_initial_selection() {
        let mut state = AppState::new();
        state.viewport.resize(800, 600);
        state.viewport.center_lat = 51.5074;
        state.viewport.center_lon = -0.1278;

        // Insert a test node
        let test_nid = state.dataset.allocate_node_id();
        let initial_pos = LatLon::new(51.5074, -0.1278);
        state.dataset.insert_node(Node::new(test_nid, initial_pos));

        let (sx, sy) = state.viewport.latlon_to_screen(initial_pos);

        // 1. First click: node is NOT selected yet
        state.selected_primitive = None;
        state.on_mouse_down(sx, sy, MouseButton::Left);

        // It should now be selected, but dragging MUST be blocked
        assert_eq!(state.selected_primitive, Some(PrimitiveId::Node(test_nid)));
        assert_eq!(state.dragged_node, None);
        assert_eq!(state.dragged_node_initial_pos, None);

        // Attempting to move the mouse during selection must NOT move the node
        state.on_mouse_move(sx + 80.0, sy + 80.0);
        assert_eq!(
            state.dataset.get_node(test_nid).unwrap().position,
            initial_pos
        );

        // Mouse up finishes the selection without any move command
        state.on_mouse_up(sx + 80.0, sy + 80.0, MouseButton::Left);
        assert_eq!(
            state.dataset.get_node(test_nid).unwrap().position,
            initial_pos
        );
        assert!(!state.command_stack.can_undo());
    }

    #[test]
    fn test_already_selected_node_can_be_moved_and_undone() {
        let mut state = AppState::new();
        state.viewport.resize(800, 600);
        state.viewport.center_lat = 51.5074;
        state.viewport.center_lon = -0.1278;

        // Insert a test node
        let test_nid = state.dataset.allocate_node_id();
        let initial_pos = LatLon::new(51.5074, -0.1278);
        state.dataset.insert_node(Node::new(test_nid, initial_pos));

        let (sx, sy) = state.viewport.latlon_to_screen(initial_pos);

        // Pre-select the node
        state.selected_primitive = Some(PrimitiveId::Node(test_nid));

        // 2. Mouse down on already selected node allows dragging
        state.on_mouse_down(sx, sy, MouseButton::Left);
        assert_eq!(state.dragged_node, Some(test_nid));
        assert_eq!(state.dragged_node_initial_pos, Some(initial_pos));

        // Move to new position
        let new_sx = sx + 50.0;
        let new_sy = sy + 50.0;
        let target_geo = state.viewport.screen_to_latlon(new_sx, new_sy);
        state.on_mouse_move(new_sx, new_sy);

        // Mouse up commits the move command
        state.on_mouse_up(new_sx, new_sy, MouseButton::Left);

        let moved_pos = state.dataset.get_node(test_nid).unwrap().position;
        assert!((moved_pos.lat() - target_geo.lat()).abs() < 1e-5);
        assert!((moved_pos.lon() - target_geo.lon()).abs() < 1e-5);
        assert!(state.command_stack.can_undo());

        // Undo successfully restores the original position
        assert!(state.undo().is_ok());
        let reverted_pos = state.dataset.get_node(test_nid).unwrap().position;
        assert_eq!(reverted_pos, initial_pos);
    }

    #[test]
    fn test_diff_counts_on_edits() {
        let mut state = AppState::new();
        state.update_diff_counts();
        assert_eq!(state.pending_created, 0);
        assert_eq!(state.pending_modified, 0);
        assert_eq!(state.pending_deleted, 0);

        // Add a new node (local negative ID)
        let nid = state.dataset.allocate_node_id();
        state
            .dataset
            .insert_node(Node::new(nid, LatLon::new(51.5, -0.1)));
        state.update_diff_counts();
        assert_eq!(state.pending_created, 1);

        // Modify an existing positive node
        let first_node = state.dataset.nodes().find(|n| !n.id.is_new()).map(|n| n.id);
        if let Some(nid) = first_node {
            if let Some(node) = state.dataset.get_node_mut(nid) {
                node.tags.insert("tested", "yes");
            }
            state.update_diff_counts();
            assert_eq!(state.pending_modified, 1);
        }
    }

    #[test]
    fn test_network_environment_and_auth() {
        let mut state = AppState::new();
        assert!(!state.is_production_server);
        assert!(state.osm_auth_token.is_none());

        state.set_auth_token("test_bearer_token_123");
        assert_eq!(
            state.osm_auth_token,
            Some("test_bearer_token_123".to_string())
        );
        assert!(state.osm_client.is_authenticated());

        state.set_osm_environment(true);
        assert!(state.is_production_server);
        assert_eq!(
            state.osm_client.environment().base_url(),
            "https://api.openstreetmap.org/api/0.6/"
        );
        // Auth token should persist across environment switch
        assert!(state.osm_client.is_authenticated());

        state.set_auth_token("");
        assert!(state.osm_auth_token.is_none());
        assert!(!state.osm_client.is_authenticated());
    }

    #[test]
    fn test_upload_changeset_without_changes_fails() {
        let mut state = AppState::new();
        state.set_auth_token("mock_token");
        let result = state.upload_changeset("No changes", None);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "No pending changes to upload");
    }
}
