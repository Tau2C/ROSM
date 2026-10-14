//! High-level imagery manager coordinating caches, fetchers, and active tile providers.

use std::collections::HashSet;
use std::sync::Arc;

use image::RgbaImage;

use crate::cache::TileCache;
use crate::coord::TileCoordinate;
use crate::fetcher::TileFetcher;
use crate::source::{osm_carto, ImagerySource, SourceRegistry};

/// Result of looking up a tile, which may return the exact tile or an ancestor tile for fallback upscaling.
#[derive(Clone)]
pub struct ResolvedTile {
    pub image: Arc<RgbaImage>,
    pub actual_coord: TileCoordinate,
    pub is_exact: bool,
}

/// Central manager orchestrating tile loading, multi-level caching, and active layers.
pub struct TileManager {
    cache: TileCache,
    fetcher: TileFetcher,
    pending: HashSet<(String, TileCoordinate)>,
    registry: SourceRegistry,
    active_source: Option<ImagerySource>,
    opacity: f32,
    enabled: bool,
}

impl TileManager {
    /// Creates a new `TileManager` using the provided cache and default registry.
    pub fn new(cache: TileCache) -> Self {
        let registry = SourceRegistry::new_with_builtins();
        let default_source = registry
            .get("osm_carto")
            .cloned()
            .or_else(|| Some(osm_carto()));
        Self {
            cache,
            fetcher: TileFetcher::new(),
            pending: HashSet::new(),
            registry,
            active_source: default_source,
            opacity: 1.0,
            enabled: true,
        }
    }

    /// Creates a new `TileManager` with default system disk cache and memory cache.
    pub fn with_default_cache() -> Self {
        Self::new(TileCache::new_with_default_dir())
    }

    /// Returns a reference to the dynamic source registry.
    pub fn registry(&self) -> &SourceRegistry {
        &self.registry
    }

    /// Returns a mutable reference to the dynamic source registry.
    pub fn registry_mut(&mut self) -> &mut SourceRegistry {
        &mut self.registry
    }

    /// Returns the currently active imagery source.
    pub fn source(&self) -> Option<&ImagerySource> {
        self.active_source.as_ref()
    }

    /// Sets or clears the active imagery source.
    pub fn set_source(&mut self, source: Option<ImagerySource>) {
        self.active_source = source;
    }

    /// Returns the current imagery layer opacity `[0.0, 1.0]`.
    pub fn opacity(&self) -> f32 {
        self.opacity
    }

    /// Sets imagery layer opacity `[0.0, 1.0]`.
    pub fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity.clamp(0.0, 1.0);
    }

    /// Returns whether background imagery is visible.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enables or disables background imagery rendering.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Polls completed background download responses and inserts them into cache.
    ///
    /// Returns the number of new tiles received and cached.
    pub fn update(&mut self) -> usize {
        let mut count = 0;
        while let Some(response) = self.fetcher.try_recv_response() {
            let key = (response.source_id.clone(), response.coord);
            self.pending.remove(&key);

            if let Ok((bytes, img)) = response.result {
                self.cache
                    .insert(&response.source_id, response.coord, &bytes, img);
                count += 1;
            }
        }
        count
    }

    /// Returns a tile if already cached, or queues an async download request.
    pub fn get_or_request_tile(
        &mut self,
        source: &ImagerySource,
        coord: TileCoordinate,
    ) -> Option<Arc<RgbaImage>> {
        if let Some(cached) = self.cache.get(&source.id, coord) {
            return Some(cached);
        }

        let key = (source.id.clone(), coord);
        if !self.pending.contains(&key) && self.fetcher.request_tile(source.clone(), coord) {
            self.pending.insert(key);
        }

        None
    }

    /// Resolves a tile for rendering, returning the exact tile or an ancestor if still loading.
    pub fn get_tile_or_ancestor(
        &mut self,
        source: &ImagerySource,
        coord: TileCoordinate,
    ) -> Option<ResolvedTile> {
        // 1. Try exact tile
        if let Some(img) = self.get_or_request_tile(source, coord) {
            return Some(ResolvedTile {
                image: img,
                actual_coord: coord,
                is_exact: true,
            });
        }

        // 2. Fallback to parent or grandparent if available in cache
        let mut current = coord;
        for _ in 0..3 {
            if let Some(parent) = current.parent() {
                if let Some(img) = self.cache.get(&source.id, parent) {
                    return Some(ResolvedTile {
                        image: img,
                        actual_coord: parent,
                        is_exact: false,
                    });
                }
                current = parent;
            } else {
                break;
            }
        }

        None
    }

    /// Returns whether a tile is currently being downloaded.
    pub fn is_pending(&self, source_id: &str, coord: TileCoordinate) -> bool {
        self.pending.contains(&(source_id.to_string(), coord))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::esri_world_imagery;

    #[test]
    fn test_manager_source_and_opacity() {
        let mut mgr = TileManager::new(TileCache::new(None, 10));
        assert_eq!(mgr.source().unwrap().id, "osm_carto");
        assert_eq!(mgr.opacity(), 1.0);
        assert!(mgr.is_enabled());

        mgr.set_opacity(0.65);
        assert!((mgr.opacity() - 0.65).abs() < 1e-4);

        let esri = esri_world_imagery();
        mgr.set_source(Some(esri));
        assert_eq!(mgr.source().unwrap().id, "esri_world_imagery");

        mgr.set_enabled(false);
        assert!(!mgr.is_enabled());
    }

    #[test]
    fn test_manager_ancestor_fallback() {
        let cache = TileCache::new(None, 10);
        let parent = TileCoordinate {
            z: 10,
            x: 500,
            y: 300,
        };
        let child = TileCoordinate {
            z: 11,
            x: 1000,
            y: 600,
        };
        assert_eq!(child.parent().unwrap(), parent);

        let parent_img = RgbaImage::new(256, 256);
        cache.insert("osm_carto", parent, &[], parent_img);

        let mut mgr = TileManager::new(cache);
        let carto = osm_carto();
        let resolved = mgr.get_tile_or_ancestor(&carto, child);
        assert!(resolved.is_some());
        let tile = resolved.unwrap();
        assert!(!tile.is_exact);
        assert_eq!(tile.actual_coord, parent);
    }
}
