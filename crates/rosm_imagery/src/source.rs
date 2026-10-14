//! Dynamic imagery providers (TMS/XYZ) and runtime source registry.

/// Metadata and URL template for a raster map imagery provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagerySource {
    pub id: String,
    pub name: String,
    pub url_template: String,
    pub min_zoom: u8,
    pub max_zoom: u8,
    pub attribution: String,
    pub is_overlay: bool,
}

impl ImagerySource {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        url_template: impl Into<String>,
        min_zoom: u8,
        max_zoom: u8,
        attribution: impl Into<String>,
        is_overlay: bool,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            url_template: url_template.into(),
            min_zoom,
            max_zoom,
            attribution: attribution.into(),
            is_overlay,
        }
    }
}

/// OpenStreetMap Standard Carto tile layer.
pub fn osm_carto() -> ImagerySource {
    ImagerySource::new(
        "osm_carto",
        "OpenStreetMap Standard",
        "https://tile.openstreetmap.org/{z}/{x}/{y}.png",
        0,
        19,
        "© OpenStreetMap contributors",
        false,
    )
}

/// Esri World Imagery (High-resolution aerial satellite).
pub fn esri_world_imagery() -> ImagerySource {
    ImagerySource::new(
        "esri_world_imagery",
        "Esri World Imagery (Satellite)",
        "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}",
        0,
        19,
        "Source: Esri, Maxar, Earthstar Geographics",
        false,
    )
}

/// OpenTopoMap (Topographic map with contours and hillshading).
pub fn open_topo_map() -> ImagerySource {
    ImagerySource::new(
        "opentopomap",
        "OpenTopoMap",
        "https://tile.opentopomap.org/{z}/{x}/{y}.png",
        0,
        17,
        "Map data: © OpenStreetMap contributors, SRTM | Map style: © OpenTopoMap (CC-BY-SA)",
        false,
    )
}

/// Humanitarian OpenStreetMap Team (HOT) style.
pub fn hot_osm() -> ImagerySource {
    ImagerySource::new(
        "hot_osm",
        "Humanitarian OSM (HOT)",
        "https://tile.openstreetmap.fr/hot/{z}/{x}/{y}.png",
        0,
        19,
        "© OpenStreetMap contributors, Tiles courtesy of Humanitarian OpenStreetMap Team",
        false,
    )
}

/// OpenRailwayMap infrastructure overlay.
pub fn open_railway_map() -> ImagerySource {
    ImagerySource::new(
        "openrailwaymap",
        "OpenRailwayMap (Overlay)",
        "https://tiles.openrailwaymap.org/standard/{z}/{x}/{y}.png",
        2,
        19,
        "Map data: © OpenStreetMap contributors, OpenRailwayMap (CC-BY-SA)",
        true,
    )
}

/// OpenSeaMap nautical marks overlay.
pub fn open_sea_map() -> ImagerySource {
    ImagerySource::new(
        "openseamap",
        "OpenSeaMap (Overlay)",
        "https://tiles.openseamap.org/seamark/{z}/{x}/{y}.png",
        0,
        18,
        "Map data: © OpenStreetMap contributors, OpenSeaMap",
        true,
    )
}

/// Returns the initial set of built-in imagery providers.
pub fn builtin_sources() -> Vec<ImagerySource> {
    vec![
        osm_carto(),
        esri_world_imagery(),
        open_topo_map(),
        hot_osm(),
        open_railway_map(),
        open_sea_map(),
    ]
}

/// Dynamic registry of imagery sources supporting runtime addition, editing, and removal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRegistry {
    sources: Vec<ImagerySource>,
}

impl Default for SourceRegistry {
    fn default() -> Self {
        Self::new_with_builtins()
    }
}

impl SourceRegistry {
    /// Creates an empty source registry.
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
        }
    }

    /// Creates a source registry populated with all standard built-in providers.
    pub fn new_with_builtins() -> Self {
        Self {
            sources: builtin_sources(),
        }
    }

    /// Returns all registered sources.
    pub fn sources(&self) -> &[ImagerySource] {
        &self.sources
    }

    /// Finds a source by ID.
    pub fn get(&self, id: &str) -> Option<&ImagerySource> {
        self.sources.iter().find(|s| s.id == id)
    }

    /// Finds a mutable reference to a source by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut ImagerySource> {
        self.sources.iter_mut().find(|s| s.id == id)
    }

    /// Adds a new imagery source. Returns `false` if a source with the same ID already exists.
    pub fn add(&mut self, source: ImagerySource) -> bool {
        if self.sources.iter().any(|s| s.id == source.id) {
            return false;
        }
        self.sources.push(source);
        true
    }

    /// Updates an existing imagery source by ID.
    pub fn update(&mut self, id: &str, updated: ImagerySource) -> bool {
        if let Some(pos) = self.sources.iter().position(|s| s.id == id) {
            self.sources[pos] = updated;
            true
        } else {
            false
        }
    }

    /// Removes an imagery source by ID.
    pub fn remove(&mut self, id: &str) -> Option<ImagerySource> {
        if let Some(pos) = self.sources.iter().position(|s| s.id == id) {
            Some(self.sources.remove(pos))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_registry_crud() {
        let mut reg = SourceRegistry::new();
        assert!(reg.sources().is_empty());

        let custom = ImagerySource::new(
            "custom_ortho",
            "Local Orthophoto",
            "https://ortho.local/{z}/{x}/{y}.png",
            10,
            20,
            "Local Survey",
            false,
        );

        assert!(reg.add(custom.clone()));
        // Duplicate ID should fail
        assert!(!reg.add(custom));
        assert_eq!(reg.sources().len(), 1);

        // Edit/update
        let mut updated = reg.get("custom_ortho").unwrap().clone();
        updated.name = "Local High-Res Ortho".to_string();
        assert!(reg.update("custom_ortho", updated));
        assert_eq!(
            reg.get("custom_ortho").unwrap().name,
            "Local High-Res Ortho"
        );

        // Remove
        let removed = reg.remove("custom_ortho");
        assert!(removed.is_some());
        assert!(reg.get("custom_ortho").is_none());
    }
}
