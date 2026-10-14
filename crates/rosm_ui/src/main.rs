//! ROSM GUI application entry point connecting the Slint interface to the ROSM core.

use std::cell::RefCell;
use std::env;
use std::path::PathBuf;
use std::rc::Rc;

use rosm_core::id::PrimitiveId;
use rosm_core::validator::Severity;
use slint::{Image, ModelRc, VecModel};

mod app_state;
mod layers;
mod renderer;
mod viewport;

use app_state::{AppState, MouseButton, ToolMode};
use renderer::render_map;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = MainWindow::new()?;
    let ui_weak = ui.as_weak();

    let state = Rc::new(RefCell::new(AppState::new()));

    // Check command line arguments for an optional OSM file to load
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        let path = PathBuf::from(&args[1]);
        if path.exists() {
            let _ = state.borrow_mut().open_file(&path);
        }
    }

    // Refresh UI with initial dataset
    update_ui(&ui, &mut state.borrow_mut());

    // --- Pointer / Canvas events ---

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_canvas_resized(move |w, h| {
            if w > 10.0 && h > 10.0 {
                let mut s = state.borrow_mut();
                s.viewport.resize(w as u32, h as u32);
                if let Some(ui) = ui_weak.upgrade() {
                    update_ui(&ui, &mut s);
                }
            }
        });
    }

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_map_pointer_down(move |x, y, w, h, btn| {
            let button = match btn {
                0 => MouseButton::Left,
                1 => MouseButton::Right,
                2 => MouseButton::Middle,
                _ => MouseButton::Other,
            };
            let mut s = state.borrow_mut();
            if w > 10.0
                && h > 10.0
                && (s.viewport.width != w as u32 || s.viewport.height != h as u32)
            {
                s.viewport.resize(w as u32, h as u32);
            }
            s.on_mouse_down(x, y, button);
            if let Some(ui) = ui_weak.upgrade() {
                update_ui(&ui, &mut s);
            }
        });
    }

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_map_pointer_up(move |x, y, w, h, btn| {
            let button = match btn {
                0 => MouseButton::Left,
                1 => MouseButton::Right,
                2 => MouseButton::Middle,
                _ => MouseButton::Other,
            };
            let mut s = state.borrow_mut();
            if w > 10.0
                && h > 10.0
                && (s.viewport.width != w as u32 || s.viewport.height != h as u32)
            {
                s.viewport.resize(w as u32, h as u32);
            }
            s.on_mouse_up(x, y, button);
            if let Some(ui) = ui_weak.upgrade() {
                update_ui(&ui, &mut s);
            }
        });
    }

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_map_pointer_move(move |x, y, w, h| {
            let mut s = state.borrow_mut();
            if w > 10.0
                && h > 10.0
                && (s.viewport.width != w as u32 || s.viewport.height != h as u32)
            {
                s.viewport.resize(w as u32, h as u32);
            }
            s.on_mouse_move(x, y);
            if let Some(ui) = ui_weak.upgrade() {
                if s.is_panning || s.dragged_node.is_some() || !s.drawing_nodes.is_empty() {
                    update_ui(&ui, &mut s);
                } else if let Some(geo) = s.mouse_geo {
                    ui.set_cursor_coords(
                        format!("Lat: {:.6}°, Lon: {:.6}°", geo.lat(), geo.lon()).into(),
                    );
                }
            }
        });
    }

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_map_scroll(move |x, y, w, h, delta_y| {
            let mut s = state.borrow_mut();
            if w > 10.0
                && h > 10.0
                && (s.viewport.width != w as u32 || s.viewport.height != h as u32)
            {
                s.viewport.resize(w as u32, h as u32);
            }
            s.on_scroll(x, y, delta_y);
            if let Some(ui) = ui_weak.upgrade() {
                update_ui(&ui, &mut s);
            }
        });
    }

    // --- Action callbacks ---

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_action_zoom_in(move || {
            let mut s = state.borrow_mut();
            s.zoom_in();
            if let Some(ui) = ui_weak.upgrade() {
                update_ui(&ui, &mut s);
            }
        });
    }

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_action_zoom_out(move || {
            let mut s = state.borrow_mut();
            s.zoom_out();
            if let Some(ui) = ui_weak.upgrade() {
                update_ui(&ui, &mut s);
            }
        });
    }

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_action_fit_extent(move || {
            let mut s = state.borrow_mut();
            s.fit_extent();
            if let Some(ui) = ui_weak.upgrade() {
                update_ui(&ui, &mut s);
            }
        });
    }

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_action_undo(move || {
            let mut s = state.borrow_mut();
            match s.undo() {
                Ok(desc) => s.status_message = format!("Undo: {desc}"),
                Err(e) => s.status_message = e,
            }
            if let Some(ui) = ui_weak.upgrade() {
                update_ui(&ui, &mut s);
            }
        });
    }

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_action_redo(move || {
            let mut s = state.borrow_mut();
            match s.redo() {
                Ok(desc) => s.status_message = format!("Redo: {desc}"),
                Err(e) => s.status_message = e,
            }
            if let Some(ui) = ui_weak.upgrade() {
                update_ui(&ui, &mut s);
            }
        });
    }

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_set_tool_mode(move |mode| {
            let mut s = state.borrow_mut();
            s.active_tool = match mode {
                1 => ToolMode::DrawWay,
                _ => ToolMode::Select,
            };
            if let Some(ui) = ui_weak.upgrade() {
                update_ui(&ui, &mut s);
            }
        });
    }

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_finish_drawing(move || {
            let mut s = state.borrow_mut();
            s.finish_drawing();
            if let Some(ui) = ui_weak.upgrade() {
                update_ui(&ui, &mut s);
            }
        });
    }

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_cancel_drawing(move || {
            let mut s = state.borrow_mut();
            s.cancel_drawing();
            if let Some(ui) = ui_weak.upgrade() {
                update_ui(&ui, &mut s);
            }
        });
    }

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_action_delete(move || {
            let mut s = state.borrow_mut();
            s.delete_selected();
            if let Some(ui) = ui_weak.upgrade() {
                update_ui(&ui, &mut s);
            }
        });
    }

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_action_split_way(move || {
            let mut s = state.borrow_mut();
            s.split_selected_way();
            if let Some(ui) = ui_weak.upgrade() {
                update_ui(&ui, &mut s);
            }
        });
    }

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_action_reverse_way(move || {
            let mut s = state.borrow_mut();
            s.reverse_selected_way();
            if let Some(ui) = ui_weak.upgrade() {
                update_ui(&ui, &mut s);
            }
        });
    }

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_action_validate(move || {
            let mut s = state.borrow_mut();
            s.run_validator();
            s.status_message = format!("Validation found {} issues", s.validation_issues.len());
            if let Some(ui) = ui_weak.upgrade() {
                update_ui(&ui, &mut s);
            }
        });
    }

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_action_open(move || {
            let mut s = state.borrow_mut();
            let sample_path = PathBuf::from("data/sample.osm");
            if sample_path.exists() {
                let _ = s.open_file(&sample_path);
            } else {
                s.load_sample_map();
            }
            if let Some(ui) = ui_weak.upgrade() {
                update_ui(&ui, &mut s);
            }
        });
    }

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_action_save(move || {
            let mut s = state.borrow_mut();
            let default_save = PathBuf::from("output.osm");
            if let Err(e) = s.save_file(Some(default_save)) {
                s.status_message = format!("Save failed: {e}");
            }
            if let Some(ui) = ui_weak.upgrade() {
                update_ui(&ui, &mut s);
            }
        });
    }

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_add_tag(move |key, value| {
            let mut s = state.borrow_mut();
            s.add_or_update_tag(key.as_str(), value.as_str());
            if let Some(ui) = ui_weak.upgrade() {
                update_ui(&ui, &mut s);
            }
        });
    }

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_remove_tag(move |key| {
            let mut s = state.borrow_mut();
            s.remove_tag(key.as_str());
            if let Some(ui) = ui_weak.upgrade() {
                update_ui(&ui, &mut s);
            }
        });
    }

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_select_issue(move |idx| {
            let mut s = state.borrow_mut();
            if let Some(issue) = s.validation_issues.get(idx as usize) {
                if let Some(&first_prim) = issue.primitives.first() {
                    s.selected_primitive = Some(first_prim);
                    // Center on the primitive if it's a node
                    if let PrimitiveId::Node(nid) = first_prim {
                        let center_opt = s
                            .dataset
                            .get_node(nid)
                            .map(|n| (n.position.lat(), n.position.lon()));
                        if let Some((lat, lon)) = center_opt {
                            s.viewport.center_lat = lat;
                            s.viewport.center_lon = lon;
                        }
                    }
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                update_ui(&ui, &mut s);
            }
        });
    }

    // --- Layer Stack Callbacks ---

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_move_layer_up(move |idx| {
            let mut s = state.borrow_mut();
            s.move_layer_up(idx as usize);
            if let Some(ui) = ui_weak.upgrade() {
                update_ui(&ui, &mut s);
            }
        });
    }

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_move_layer_down(move |idx| {
            let mut s = state.borrow_mut();
            s.move_layer_down(idx as usize);
            if let Some(ui) = ui_weak.upgrade() {
                update_ui(&ui, &mut s);
            }
        });
    }

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_toggle_layer_visibility(move |idx| {
            let mut s = state.borrow_mut();
            s.toggle_layer_visibility(idx as usize);
            if let Some(ui) = ui_weak.upgrade() {
                update_ui(&ui, &mut s);
            }
        });
    }

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_set_layer_opacity(move |idx, opacity| {
            let mut s = state.borrow_mut();
            s.set_layer_opacity(idx as usize, opacity);
            if let Some(ui) = ui_weak.upgrade() {
                update_ui(&ui, &mut s);
            }
        });
    }

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_set_layer_source(move |idx, src_id| {
            let mut s = state.borrow_mut();
            s.set_layer_source(idx as usize, src_id.as_str());
            if let Some(ui) = ui_weak.upgrade() {
                update_ui(&ui, &mut s);
            }
        });
    }

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_add_layer_from_source(move |src_id, is_overlay| {
            let mut s = state.borrow_mut();
            s.add_layer_from_source(src_id.as_str(), is_overlay);
            if let Some(ui) = ui_weak.upgrade() {
                update_ui(&ui, &mut s);
            }
        });
    }

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_remove_layer(move |idx| {
            let mut s = state.borrow_mut();
            s.remove_layer(idx as usize);
            if let Some(ui) = ui_weak.upgrade() {
                update_ui(&ui, &mut s);
            }
        });
    }

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_add_custom_source(move |id, name, url, min_z, max_z, is_overlay| {
            let mut s = state.borrow_mut();
            s.add_custom_source(
                id.as_str(),
                name.as_str(),
                url.as_str(),
                min_z as u8,
                max_z as u8,
                is_overlay,
            );
            if let Some(ui) = ui_weak.upgrade() {
                update_ui(&ui, &mut s);
            }
        });
    }

    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        ui.on_remove_custom_source(move |id| {
            let mut s = state.borrow_mut();
            s.remove_custom_source(id.as_str());
            if let Some(ui) = ui_weak.upgrade() {
                update_ui(&ui, &mut s);
            }
        });
    }

    // Synchronize viewport with window layout on first show
    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        slint::Timer::single_shot(std::time::Duration::from_millis(50), move || {
            if let Some(ui) = ui_weak.upgrade() {
                let actual_w = ui.get_canvas_actual_width();
                let actual_h = ui.get_canvas_actual_height();
                let mut s = state.borrow_mut();
                if actual_w > 10.0 && actual_h > 10.0 {
                    s.viewport.resize(actual_w as u32, actual_h as u32);
                    s.fit_extent();
                }
                update_ui(&ui, &mut s);
            }
        });
    }

    // Periodic timer to poll background downloaded tiles
    let tile_timer = slint::Timer::default();
    {
        let state = Rc::clone(&state);
        let ui_weak = ui_weak.clone();
        tile_timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(60),
            move || {
                let mut s = state.borrow_mut();
                let count = s.tile_manager.update();
                if count > 0 {
                    if let Some(ui) = ui_weak.upgrade() {
                        update_ui(&ui, &mut s);
                    }
                }
            },
        );
    }
    let _keep_timer = tile_timer;

    ui.run()
}

/// Updates Slint UI properties from the current AppState.
fn update_ui(ui: &MainWindow, state: &mut AppState) {
    let mut vp = state.viewport.clone();
    let actual_w = ui.get_canvas_actual_width();
    let actual_h = ui.get_canvas_actual_height();
    if actual_w > 10.0 && actual_h > 10.0 {
        vp.resize(actual_w as u32, actual_h as u32);
    }

    // 1. Render Map Canvas
    let buffer = render_map(
        &state.dataset,
        &vp,
        state.selected_primitive,
        &state.drawing_nodes,
        state.mouse_geo,
        Some(&mut state.tile_manager),
        Some(&state.layer_stack),
    );
    ui.set_map_image(Image::from_rgba8(buffer));

    // 2. Layer Stack Properties
    let layer_entries: Vec<LayerEntry> = state
        .layer_stack
        .layers()
        .iter()
        .enumerate()
        .map(|(idx, l)| LayerEntry {
            idx: idx as i32,
            id: l.id.clone().into(),
            name: l.name.clone().into(),
            type_name: l.type_name().into(),
            visible: l.visible,
            opacity: l.opacity,
            source_id: l.source_id().unwrap_or("").into(),
            can_delete: !matches!(l.kind, crate::layers::LayerKind::OsmData),
        })
        .collect();
    ui.set_layer_count(layer_entries.len() as i32);
    ui.set_layers_list(ModelRc::new(VecModel::from(layer_entries)));

    // 3. Toolbar & Status
    ui.set_active_tool(match state.active_tool {
        ToolMode::Select => 0,
        ToolMode::DrawWay => 1,
    });
    ui.set_is_drawing(!state.drawing_nodes.is_empty());
    ui.set_can_undo(state.command_stack.can_undo());
    ui.set_can_redo(state.command_stack.can_redo());
    ui.set_is_dirty(state.command_stack.is_dirty());

    ui.set_stats_text(
        format!(
            "Nodes: {} | Ways: {} | Relations: {}",
            state.dataset.node_count(),
            state.dataset.way_count(),
            state.dataset.relation_count()
        )
        .into(),
    );

    if let Some(geo) = state.mouse_geo {
        ui.set_cursor_coords(format!("Lat: {:.6}°, Lon: {:.6}°", geo.lat(), geo.lon()).into());
    }

    ui.set_status_message(state.status_message.clone().into());

    // 3. Selection Details & Tags
    ui.set_has_selection(state.selected_primitive.is_some());
    if let Some(prim_id) = state.selected_primitive {
        match prim_id {
            PrimitiveId::Node(nid) => {
                ui.set_selection_title(format!("Node {nid}").into());
                if let Some(node) = state.dataset.get_node(nid) {
                    ui.set_selection_details(
                        format!(
                            "Lat: {:.6}, Lon: {:.6}",
                            node.position.lat(),
                            node.position.lon()
                        )
                        .into(),
                    );
                    let tags: Vec<TagEntry> = node
                        .tags
                        .iter()
                        .map(|(k, v)| TagEntry {
                            key: k.into(),
                            value: v.into(),
                        })
                        .collect();
                    ui.set_selection_tags(ModelRc::new(VecModel::from(tags)));
                }
            }
            PrimitiveId::Way(wid) => {
                ui.set_selection_title(format!("Way {wid}").into());
                if let Some(way) = state.dataset.get_way(wid) {
                    ui.set_selection_details(
                        format!(
                            "{} nodes | {}",
                            way.nodes.len(),
                            if way.is_closed() {
                                "Closed ring"
                            } else {
                                "Open linestring"
                            }
                        )
                        .into(),
                    );
                    let tags: Vec<TagEntry> = way
                        .tags
                        .iter()
                        .map(|(k, v)| TagEntry {
                            key: k.into(),
                            value: v.into(),
                        })
                        .collect();
                    ui.set_selection_tags(ModelRc::new(VecModel::from(tags)));
                }
            }
            PrimitiveId::Relation(rid) => {
                ui.set_selection_title(format!("Relation {rid}").into());
                if let Some(rel) = state.dataset.get_relation(rid) {
                    ui.set_selection_details(format!("{} members", rel.members.len()).into());
                    let tags: Vec<TagEntry> = rel
                        .tags
                        .iter()
                        .map(|(k, v)| TagEntry {
                            key: k.into(),
                            value: v.into(),
                        })
                        .collect();
                    ui.set_selection_tags(ModelRc::new(VecModel::from(tags)));
                }
            }
        }
    } else {
        ui.set_selection_title("No Selection".into());
        ui.set_selection_details("Click a node or way on the map to inspect".into());
        ui.set_selection_tags(ModelRc::new(VecModel::default()));
    }

    // 4. Validation Issues
    ui.set_issue_count(state.validation_issues.len() as i32);
    let issue_models: Vec<IssueEntry> = state
        .validation_issues
        .iter()
        .map(|i| IssueEntry {
            rule: i.rule.into(),
            message: i.message.clone().into(),
            severity: match i.severity {
                Severity::Error => "Error".into(),
                Severity::Warning => "Warning".into(),
                Severity::Info => "Info".into(),
            },
        })
        .collect();
    ui.set_validation_issues(ModelRc::new(VecModel::from(issue_models)));
}
