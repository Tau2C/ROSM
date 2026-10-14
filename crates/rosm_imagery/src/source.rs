//! Standard imagery providers (TMS/XYZ) and source definitions.

/// Metadata and URL template for a raster map imagery provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagerySource {
    pub id: &'static str,
    pub name: &'static str,
    pub url_template: &'static str,
    pub min_zoom: u8,
    pub max_zoom: u8,
    pub attribution: &'static str,
}

impl ImagerySource {
    pub const fn new(
        id: &'static str,
        name: &'static str,
        url_template: &'static str,
        min_zoom: u8,
        max_zoom: u8,
        attribution: &'static str,
    ) -> Self {
        Self {
            id,
            name,
            url_template,
            min_zoom,
            max_zoom,
            attribution,
        }
    }
}

/// OpenStreetMap Standard Carto tile layer.
pub const OSM_CARTO: ImagerySource = ImagerySource::new(
    "osm_carto",
    "OpenStreetMap Standard",
    "https://tile.openstreetmap.org/{z}/{x}/{y}.png",
    0,
    19,
    "© OpenStreetMap contributors",
);

/// Esri World Imagery (High-resolution aerial satellite).
pub const ESRI_WORLD_IMAGERY: ImagerySource = ImagerySource::new(
    "esri_world_imagery",
    "Esri World Imagery (Satellite)",
    "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}",
    0,
    19,
    "Source: Esri, Maxar, Earthstar Geographics",
);

/// OpenTopoMap (Topographic map with contours and hillshading).
pub const OPEN_TOPO_MAP: ImagerySource = ImagerySource::new(
    "opentopomap",
    "OpenTopoMap",
    "https://tile.opentopomap.org/{z}/{x}/{y}.png",
    0,
    17,
    "Map data: © OpenStreetMap contributors, SRTM | Map style: © OpenTopoMap (CC-BY-SA)",
);

/// Humanitarian OpenStreetMap Team (HOT) style.
pub const HOT_OSM: ImagerySource = ImagerySource::new(
    "hot_osm",
    "Humanitarian OSM (HOT)",
    "https://tile.openstreetmap.fr/hot/{z}/{x}/{y}.png",
    0,
    19,
    "© OpenStreetMap contributors, Tiles courtesy of Humanitarian OpenStreetMap Team",
);

/// Built-in imagery catalog available in the editor.
pub const BUILTIN_SOURCES: &[ImagerySource] =
    &[OSM_CARTO, ESRI_WORLD_IMAGERY, OPEN_TOPO_MAP, HOT_OSM];
