//! Multi-level tile caching (L1 In-Memory + L2 Persistent On-Disk).

use std::collections::{HashMap, VecDeque};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use image::RgbaImage;

use crate::coord::TileCoordinate;

/// Maximum number of tiles retained in memory before LRU eviction (~64 MB uncompressed).
pub const DEFAULT_MAX_MEMORY_TILES: usize = 256;

/// L1 in-memory LRU cache + L2 disk cache for map tiles.
pub struct TileCache {
    disk_cache_dir: Option<PathBuf>,
    max_memory_tiles: usize,
    memory_cache: Mutex<MemoryCacheState>,
}

struct MemoryCacheState {
    tiles: HashMap<(String, TileCoordinate), Arc<RgbaImage>>,
    lru_order: VecDeque<(String, TileCoordinate)>,
}

impl TileCache {
    /// Creates a tile cache with default system user cache directory (`~/.cache/rosm/tiles`).
    pub fn new_with_default_dir() -> Self {
        let base_dir = std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
            .map(|p| p.join("rosm").join("tiles"));

        Self::new(base_dir, DEFAULT_MAX_MEMORY_TILES)
    }

    /// Creates a tile cache with a specified disk directory and memory capacity.
    pub fn new(disk_cache_dir: Option<PathBuf>, max_memory_tiles: usize) -> Self {
        Self {
            disk_cache_dir,
            max_memory_tiles: max_memory_tiles.max(1),
            memory_cache: Mutex::new(MemoryCacheState {
                tiles: HashMap::new(),
                lru_order: VecDeque::new(),
            }),
        }
    }

    /// Looks up a tile in L1 memory cache, falling back to L2 disk cache.
    pub fn get(&self, source_id: &str, coord: TileCoordinate) -> Option<Arc<RgbaImage>> {
        // 1. Check L1 memory cache
        {
            let mut mem = self.memory_cache.lock().unwrap();
            let key = (source_id.to_string(), coord);
            if let Some(tile) = mem.tiles.get(&key).cloned() {
                // Promote to front of LRU
                if let Some(pos) = mem.lru_order.iter().position(|k| k == &key) {
                    mem.lru_order.remove(pos);
                }
                mem.lru_order.push_back(key);
                return Some(tile);
            }
        }

        // 2. Check L2 disk cache
        if let Some(ref base_dir) = self.disk_cache_dir {
            let tile_path = self.disk_tile_path(base_dir, source_id, coord);
            if tile_path.exists() {
                if let Ok(mut file) = File::open(&tile_path) {
                    let mut bytes = Vec::new();
                    if file.read_to_end(&mut bytes).is_ok() {
                        if let Ok(dyn_img) = image::load_from_memory(&bytes) {
                            let img_arc = Arc::new(dyn_img.to_rgba8());
                            // Populate L1 cache
                            self.insert_memory(source_id, coord, Arc::clone(&img_arc));
                            return Some(img_arc);
                        }
                    }
                }
            }
        }

        None
    }

    /// Inserts a newly downloaded tile into both memory and disk caches.
    pub fn insert(
        &self,
        source_id: &str,
        coord: TileCoordinate,
        raw_bytes: &[u8],
        image: RgbaImage,
    ) -> Arc<RgbaImage> {
        let img_arc = Arc::new(image);
        self.insert_memory(source_id, coord, Arc::clone(&img_arc));

        // Save raw bytes directly to disk cache
        if let Some(ref base_dir) = self.disk_cache_dir {
            let tile_path = self.disk_tile_path(base_dir, source_id, coord);
            if let Some(parent) = tile_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(mut file) = File::create(&tile_path) {
                let _ = file.write_all(raw_bytes);
            }
        }

        img_arc
    }

    fn insert_memory(&self, source_id: &str, coord: TileCoordinate, img_arc: Arc<RgbaImage>) {
        let mut mem = self.memory_cache.lock().unwrap();
        let key = (source_id.to_string(), coord);

        if let Some(pos) = mem.lru_order.iter().position(|k| k == &key) {
            mem.lru_order.remove(pos);
        }

        mem.tiles.insert(key.clone(), img_arc);
        mem.lru_order.push_back(key);

        // Evict oldest if exceeding capacity
        while mem.lru_order.len() > self.max_memory_tiles {
            if let Some(evicted_key) = mem.lru_order.pop_front() {
                mem.tiles.remove(&evicted_key);
            }
        }
    }

    fn disk_tile_path(&self, base_dir: &Path, source_id: &str, coord: TileCoordinate) -> PathBuf {
        base_dir
            .join(source_id)
            .join(coord.z.to_string())
            .join(coord.x.to_string())
            .join(format!("{}.png", coord.y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_cache_lru() {
        let cache = TileCache::new(None, 2);
        let img1 = RgbaImage::new(10, 10);
        let img2 = RgbaImage::new(10, 10);
        let img3 = RgbaImage::new(10, 10);

        let t1 = TileCoordinate { z: 1, x: 0, y: 0 };
        let t2 = TileCoordinate { z: 1, x: 1, y: 0 };
        let t3 = TileCoordinate { z: 1, x: 0, y: 1 };

        cache.insert("test", t1, &[], img1);
        cache.insert("test", t2, &[], img2);
        // Access t1 so t1 is promoted to most recently used, leaving t2 as oldest
        assert!(cache.get("test", t1).is_some());

        // Insert 3rd tile: should evict oldest (t2 should be evicted)
        cache.insert("test", t3, &[], img3);
        assert!(cache.get("test", t3).is_some());
        assert!(cache.get("test", t1).is_some());
        assert!(cache.get("test", t2).is_none());
    }
}
