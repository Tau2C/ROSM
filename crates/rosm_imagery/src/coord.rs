//! Slippy map tile coordinate math and bounding box calculations (EPSG:3857 / Web Mercator).

use std::f64::consts::PI;
use std::fmt;

use rosm_core::geo::{BoundingBox, LatLon};

/// Standard size of a Web Mercator slippy map tile in pixels.
pub const TILE_SIZE_PIXELS: u32 = 256;

/// A tile coordinate in the standard Slippy map / OpenStreetMap grid at zoom level `z`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TileCoordinate {
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

impl fmt::Display for TileCoordinate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}/{}", self.z, self.x, self.y)
    }
}

impl TileCoordinate {
    /// Creates a new `TileCoordinate`. Clamps `x` and `y` to `[0, 2^z - 1]`.
    pub fn new(z: u8, x: u32, y: u32) -> Self {
        let max = (1u32 << z).saturating_sub(1);
        Self {
            z,
            x: x.min(max),
            y: y.min(max),
        }
    }

    /// Converts a geographic coordinate (`LatLon`) to a tile coordinate at zoom level `z`.
    pub fn from_latlon(pos: LatLon, zoom: u8) -> Self {
        let (tx, ty) = pos.to_tile_coords(zoom);
        let max_idx = (1u32 << zoom).saturating_sub(1);

        let x = (tx.floor() as i64).clamp(0, max_idx as i64) as u32;
        let y = (ty.floor() as i64).clamp(0, max_idx as i64) as u32;

        Self { z: zoom, x, y }
    }

    /// Computes the exact geographic bounding box (`BoundingBox`) covered by this tile.
    pub fn bounds(&self) -> BoundingBox {
        let n = 2.0_f64.powi(self.z as i32);

        let min_lon = (self.x as f64 / n) * 360.0 - 180.0;
        let max_lon = ((self.x + 1) as f64 / n) * 360.0 - 180.0;

        let max_lat = tile_y_to_lat(self.y as f64, n);
        let min_lat = tile_y_to_lat((self.y + 1) as f64, n);

        BoundingBox::new(min_lat, min_lon, max_lat, max_lon)
    }

    /// Finds all tile coordinates intersecting a given bounding box at zoom level `zoom`.
    pub fn tiles_in_bbox(bbox: &BoundingBox, zoom: u8) -> Vec<TileCoordinate> {
        let nw_coord = LatLon::new(bbox.max_lat, bbox.min_lon);
        let se_coord = LatLon::new(bbox.min_lat, bbox.max_lon);

        let top_left = Self::from_latlon(nw_coord, zoom);
        let bottom_right = Self::from_latlon(se_coord, zoom);

        let mut tiles = Vec::new();
        let min_x = top_left.x.min(bottom_right.x);
        let max_x = top_left.x.max(bottom_right.x);
        let min_y = top_left.y.min(bottom_right.y);
        let max_y = top_left.y.max(bottom_right.y);

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                tiles.push(TileCoordinate { z: zoom, x, y });
            }
        }

        tiles
    }

    /// Returns the parent tile at zoom `z - 1` if `z > 0`.
    pub fn parent(&self) -> Option<TileCoordinate> {
        if self.z == 0 {
            None
        } else {
            Some(TileCoordinate {
                z: self.z - 1,
                x: self.x / 2,
                y: self.y / 2,
            })
        }
    }

    /// Returns the 4 child tiles at zoom `z + 1`.
    pub fn children(&self) -> [TileCoordinate; 4] {
        let z = self.z + 1;
        let x = self.x * 2;
        let y = self.y * 2;
        [
            TileCoordinate { z, x, y },
            TileCoordinate { z, x: x + 1, y },
            TileCoordinate { z, x, y: y + 1 },
            TileCoordinate {
                z,
                x: x + 1,
                y: y + 1,
            },
        ]
    }

    /// Formats this tile coordinate into a URL template containing `{z}`, `{x}`, and `{y}`.
    pub fn format_url(&self, template: &str) -> String {
        template
            .replace("{z}", &self.z.to_string())
            .replace("{x}", &self.x.to_string())
            .replace("{y}", &self.y.to_string())
    }
}

fn tile_y_to_lat(y: f64, n: f64) -> f64 {
    let sinh = (PI * (1.0 - 2.0 * y / n)).sinh();
    sinh.atan().to_degrees()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile_from_latlon_zoom0() {
        let coord = LatLon::new(0.0, 0.0);
        let tile = TileCoordinate::from_latlon(coord, 0);
        assert_eq!(tile, TileCoordinate { z: 0, x: 0, y: 0 });

        let bounds = tile.bounds();
        assert!((bounds.min_lon - (-180.0)).abs() < 1e-4);
        assert!((bounds.max_lon - 180.0).abs() < 1e-4);
    }

    #[test]
    fn test_london_tile_coordinates() {
        // Trafalgar Square, London (~51.5080, -0.1281)
        let london = LatLon::new(51.5080, -0.1281);
        let tile = TileCoordinate::from_latlon(london, 15);

        // Standard known tile for central London at z15
        assert_eq!(tile.z, 15);
        assert_eq!(tile.x, 16372);
        assert_eq!(tile.y, 10896);

        let bounds = tile.bounds();
        assert!(bounds.contains_point(london));
    }

    #[test]
    fn test_parent_and_children() {
        let tile = TileCoordinate {
            z: 15,
            x: 16372,
            y: 10895,
        };
        let parent = tile.parent().unwrap();
        assert_eq!(
            parent,
            TileCoordinate {
                z: 14,
                x: 8186,
                y: 5447
            }
        );

        let children = parent.children();
        assert!(children.contains(&tile));
    }

    #[test]
    fn test_format_url() {
        let tile = TileCoordinate {
            z: 10,
            x: 500,
            y: 300,
        };
        let url = tile.format_url("https://tile.openstreetmap.org/{z}/{x}/{y}.png");
        assert_eq!(url, "https://tile.openstreetmap.org/10/500/300.png");
    }

    #[test]
    fn test_tiles_in_bbox() {
        let bbox = BoundingBox::new(51.50, -0.14, 51.52, -0.12);
        let tiles = TileCoordinate::tiles_in_bbox(&bbox, 14);
        assert!(!tiles.is_empty());
        for t in &tiles {
            assert!(t.bounds().intersects(&bbox));
        }
    }
}
