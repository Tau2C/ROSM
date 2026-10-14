//! Layer architecture managing stacking order, visibility, sources, and opacity.

/// The functional type of a map rendering layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayerKind {
    /// Background raster tile layer (e.g. OSM Carto, Satellite)
    Background { source_id: String },
    /// Overlay raster tile layer (e.g. OpenRailwayMap, OpenSeaMap)
    Overlay { source_id: String },
    /// Interactive vector OSM data layer (ways, nodes, relations, drawing)
    OsmData,
    /// Geographic lat/lon grid
    Grid,
}

/// A configurable layer in the map rendering pipeline.
#[derive(Debug, Clone, PartialEq)]
pub struct MapLayer {
    pub id: String,
    pub name: String,
    pub kind: LayerKind,
    pub visible: bool,
    pub opacity: f32, // 0.0 ..= 1.0
}

impl MapLayer {
    pub fn new_background(
        id: impl Into<String>,
        name: impl Into<String>,
        source_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind: LayerKind::Background {
                source_id: source_id.into(),
            },
            visible: true,
            opacity: 1.0,
        }
    }

    pub fn new_overlay(
        id: impl Into<String>,
        name: impl Into<String>,
        source_id: impl Into<String>,
        opacity: f32,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind: LayerKind::Overlay {
                source_id: source_id.into(),
            },
            visible: true,
            opacity: opacity.clamp(0.0, 1.0),
        }
    }

    pub fn new_data(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind: LayerKind::OsmData,
            visible: true,
            opacity: 1.0,
        }
    }

    pub fn new_grid(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind: LayerKind::Grid,
            visible: true,
            opacity: 0.8,
        }
    }

    pub fn source_id(&self) -> Option<&str> {
        match &self.kind {
            LayerKind::Background { source_id } | LayerKind::Overlay { source_id } => {
                Some(source_id)
            }
            _ => None,
        }
    }

    pub fn set_source_id(&mut self, new_source_id: impl Into<String>) {
        let sid = new_source_id.into();
        match &mut self.kind {
            LayerKind::Background { source_id } | LayerKind::Overlay { source_id } => {
                *source_id = sid;
            }
            _ => {}
        }
    }

    pub fn type_name(&self) -> &'static str {
        match &self.kind {
            LayerKind::Background { .. } => "Background",
            LayerKind::Overlay { .. } => "Overlay",
            LayerKind::OsmData => "Data",
            LayerKind::Grid => "Grid",
        }
    }
}

/// Ordered stack of layers rendered from bottom (index 0) to top (last index).
#[derive(Debug, Clone)]
pub struct LayerStack {
    layers: Vec<MapLayer>,
}

impl Default for LayerStack {
    fn default() -> Self {
        Self::standard_stack()
    }
}

impl LayerStack {
    /// Creates a standard initial layer stack: Background -> Grid -> OSM Data -> Railway Overlay (hidden by default).
    pub fn standard_stack() -> Self {
        Self {
            layers: vec![
                MapLayer::new_background("layer_bg", "Background: OSM Standard", "osm_carto"),
                MapLayer::new_grid("layer_grid", "Coordinate Grid"),
                MapLayer::new_data("layer_data", "OSM Vector Data"),
                MapLayer {
                    id: "layer_railway".to_string(),
                    name: "OpenRailwayMap Overlay".to_string(),
                    kind: LayerKind::Overlay {
                        source_id: "openrailwaymap".to_string(),
                    },
                    visible: false,
                    opacity: 0.75,
                },
            ],
        }
    }

    pub fn layers(&self) -> &[MapLayer] {
        &self.layers
    }

    #[allow(dead_code)]
    pub fn layers_mut(&mut self) -> &mut [MapLayer] {
        &mut self.layers
    }

    pub fn len(&self) -> usize {
        self.layers.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    pub fn get(&self, idx: usize) -> Option<&MapLayer> {
        self.layers.get(idx)
    }

    #[allow(dead_code)]
    pub fn get_mut(&mut self, idx: usize) -> Option<&mut MapLayer> {
        self.layers.get_mut(idx)
    }

    /// Moves a layer one position higher in rendering order (rendered on top of more layers).
    pub fn move_up(&mut self, idx: usize) -> bool {
        if idx + 1 < self.layers.len() {
            self.layers.swap(idx, idx + 1);
            true
        } else {
            false
        }
    }

    /// Moves a layer one position lower in rendering order (rendered underneath).
    pub fn move_down(&mut self, idx: usize) -> bool {
        if idx > 0 && idx < self.layers.len() {
            self.layers.swap(idx, idx - 1);
            true
        } else {
            false
        }
    }

    /// Toggles visibility of the layer at `idx`.
    pub fn toggle_visibility(&mut self, idx: usize) -> bool {
        if let Some(l) = self.layers.get_mut(idx) {
            l.visible = !l.visible;
            true
        } else {
            false
        }
    }

    /// Sets opacity of the layer at `idx`.
    pub fn set_opacity(&mut self, idx: usize, opacity: f32) -> bool {
        if let Some(l) = self.layers.get_mut(idx) {
            l.opacity = opacity.clamp(0.0, 1.0);
            true
        } else {
            false
        }
    }

    /// Updates the source ID of an imagery/overlay layer at `idx`.
    pub fn set_source_id(&mut self, idx: usize, source_id: &str, source_name: &str) -> bool {
        if let Some(l) = self.layers.get_mut(idx) {
            l.set_source_id(source_id);
            match l.kind {
                LayerKind::Background { .. } => l.name = format!("Background: {source_name}"),
                LayerKind::Overlay { .. } => l.name = source_name.to_string(),
                _ => {}
            }
            true
        } else {
            false
        }
    }

    /// Appends a new layer to the top of the stack.
    pub fn add_layer(&mut self, layer: MapLayer) {
        self.layers.push(layer);
    }

    /// Removes a layer by index. Returns `None` if trying to delete the vector data layer.
    pub fn remove_layer(&mut self, idx: usize) -> Option<MapLayer> {
        if let Some(l) = self.layers.get(idx) {
            if matches!(l.kind, LayerKind::OsmData) {
                return None; // Data layer cannot be deleted
            }
        }
        if idx < self.layers.len() {
            Some(self.layers.remove(idx))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_stack_operations() {
        let mut stack = LayerStack::standard_stack();
        assert_eq!(stack.len(), 4);

        // Move Background up
        assert!(stack.move_up(0));
        assert_eq!(stack.get(1).unwrap().id, "layer_bg");

        // Move Background back down
        assert!(stack.move_down(1));
        assert_eq!(stack.get(0).unwrap().id, "layer_bg");

        // Toggle visibility
        assert!(stack.toggle_visibility(0));
        assert!(!stack.get(0).unwrap().visible);

        // Opacity
        assert!(stack.set_opacity(0, 0.45));
        assert!((stack.get(0).unwrap().opacity - 0.45).abs() < 1e-4);

        // Cannot delete data layer
        let data_idx = stack
            .layers()
            .iter()
            .position(|l| matches!(l.kind, LayerKind::OsmData))
            .unwrap();
        assert!(stack.remove_layer(data_idx).is_none());

        // Can delete overlay layer
        let overlay_idx = stack
            .layers()
            .iter()
            .position(|l| matches!(l.kind, LayerKind::Overlay { .. }))
            .unwrap();
        assert!(stack.remove_layer(overlay_idx).is_some());
        assert_eq!(stack.len(), 3);
    }
}
