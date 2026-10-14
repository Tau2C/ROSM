//! Slippy map raster tile fetching, multi-level caching, and layer management for ROSM.

pub mod cache;
pub mod coord;
pub mod fetcher;
pub mod manager;
pub mod source;

pub use cache::{TileCache, DEFAULT_MAX_MEMORY_TILES};
pub use coord::{TileCoordinate, TILE_SIZE_PIXELS};
pub use fetcher::{FetchError, TileFetchRequest, TileFetchResponse, TileFetcher};
pub use manager::{ResolvedTile, TileManager};
pub use source::{
    builtin_sources, esri_world_imagery, hot_osm, open_railway_map, open_sea_map, open_topo_map,
    osm_carto, ImagerySource, SourceRegistry,
};

/// Converts pixels per degree of longitude into an integer Slippy map zoom level `[0, 19]`.
pub fn zoom_level_from_pixels_per_deg(pixels_per_deg: f64) -> u8 {
    if pixels_per_deg <= 0.0 || !pixels_per_deg.is_finite() {
        return 0;
    }
    // At zoom z, 256 * 2^z pixels cover 360 degrees => 2^z * 256 / 360 = 2^z * 32 / 45
    let z = (pixels_per_deg * 45.0 / 32.0).log2().round() as i32;
    z.clamp(0, 19) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zoom_level_calculation() {
        // At zoom 0: ~0.71 pixels / deg
        assert_eq!(zoom_level_from_pixels_per_deg(0.71), 0);
        // Default Viewport zoom: 50,000 pixels / deg -> ~z16
        assert_eq!(zoom_level_from_pixels_per_deg(50_000.0), 16);
        // Close-up: 500,000 pixels / deg -> ~z19
        assert_eq!(zoom_level_from_pixels_per_deg(500_000.0), 19);
    }
}
