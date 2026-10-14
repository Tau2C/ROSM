//! Viewport projection and camera navigation for the map canvas.

use rosm_core::geo::{BoundingBox, LatLon};

/// Viewport managing coordinate transformation between screen pixels and WGS84 geographic coordinates.
#[derive(Debug, Clone)]
pub struct Viewport {
    pub center_lat: f64,
    pub center_lon: f64,
    /// Pixels per degree of longitude.
    pub zoom: f64,
    pub width: u32,
    pub height: u32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            center_lat: 51.5074, // London default
            center_lon: -0.1278,
            zoom: 50_000.0,
            width: 800,
            height: 600,
        }
    }
}

impl Viewport {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width: width.max(1),
            height: height.max(1),
            ..Default::default()
        }
    }

    /// Sets viewport canvas dimensions.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width.max(1);
        self.height = height.max(1);
    }

    /// Aspect ratio correction for latitude based on center latitude.
    #[inline]
    fn lat_cos(&self) -> f64 {
        self.center_lat.to_radians().cos().max(0.01)
    }

    /// Converts a geographic coordinate to screen pixel coordinates `(x, y)`.
    pub fn latlon_to_screen(&self, pos: LatLon) -> (f32, f32) {
        let half_w = self.width as f64 / 2.0;
        let half_h = self.height as f64 / 2.0;

        let d_lon = pos.lon() - self.center_lon;
        let d_lat = pos.lat() - self.center_lat;

        let screen_x = half_w + d_lon * self.lat_cos() * self.zoom;
        // Invert Y axis: higher latitude is higher up on screen (lower Y pixel)
        let screen_y = half_h - d_lat * self.zoom;

        (screen_x as f32, screen_y as f32)
    }

    /// Converts screen pixel coordinates `(x, y)` to a geographic coordinate.
    pub fn screen_to_latlon(&self, x: f32, y: f32) -> LatLon {
        let half_w = self.width as f64 / 2.0;
        let half_h = self.height as f64 / 2.0;

        let dx = x as f64 - half_w;
        let dy = y as f64 - half_h;

        let d_lon = dx / (self.lat_cos() * self.zoom);
        let d_lat = -dy / self.zoom;

        let lat = (self.center_lat + d_lat).clamp(-85.0, 85.0);
        let lon = (self.center_lon + d_lon).clamp(-180.0, 180.0);

        LatLon::new(lat, lon)
    }

    /// Pans the camera by a delta in screen pixels.
    pub fn pan(&mut self, dx_pixels: f32, dy_pixels: f32) {
        let d_lon = -(dx_pixels as f64) / (self.lat_cos() * self.zoom);
        let d_lat = (dy_pixels as f64) / self.zoom;

        self.center_lon = (self.center_lon + d_lon).clamp(-180.0, 180.0);
        self.center_lat = (self.center_lat + d_lat).clamp(-85.0, 85.0);
    }

    /// Multiplies current zoom by `factor`, anchoring around the given screen coordinates.
    pub fn zoom_at(&mut self, factor: f64, anchor_x: f32, anchor_y: f32) {
        let anchor_geo = self.screen_to_latlon(anchor_x, anchor_y);

        let new_zoom = (self.zoom * factor).clamp(500.0, 50_000_000.0);
        self.zoom = new_zoom;

        // Reposition camera so the anchor point remains under the cursor
        let (new_screen_x, new_screen_y) = self.latlon_to_screen(anchor_geo);
        let dx = new_screen_x - anchor_x;
        let dy = new_screen_y - anchor_y;
        self.pan(dx, dy);
    }

    /// Adjusts center and zoom to fit the entire bounding box inside the canvas.
    pub fn fit_bounds(&mut self, bounds: &BoundingBox) {
        self.center_lat = (bounds.min_lat + bounds.max_lat) / 2.0;
        self.center_lon = (bounds.min_lon + bounds.max_lon) / 2.0;

        let pad_w = (self.width as f64 * 0.85).max(10.0);
        let pad_h = (self.height as f64 * 0.85).max(10.0);

        let d_lon = (bounds.max_lon - bounds.min_lon).max(0.0001) * self.lat_cos();
        let d_lat = (bounds.max_lat - bounds.min_lat).max(0.0001);

        let zoom_x = pad_w / d_lon;
        let zoom_y = pad_h / d_lat;

        self.zoom = zoom_x.min(zoom_y).clamp(500.0, 50_000_000.0);
    }

    /// Visible geographic bounding box for the current viewport.
    pub fn visible_bbox(&self) -> BoundingBox {
        let top_left = self.screen_to_latlon(0.0, 0.0);
        let bottom_right = self.screen_to_latlon(self.width as f32, self.height as f32);

        BoundingBox::new(
            bottom_right.lat(),
            top_left.lon(),
            top_left.lat(),
            bottom_right.lon(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screen_to_latlon_roundtrip() {
        let vp = Viewport::new(800, 600);
        let screen_pt = (400.0, 300.0); // center
        let geo = vp.screen_to_latlon(screen_pt.0, screen_pt.1);
        assert!((geo.lat() - vp.center_lat).abs() < 1e-6);
        assert!((geo.lon() - vp.center_lon).abs() < 1e-6);

        let (rx, ry) = vp.latlon_to_screen(geo);
        assert!((rx - screen_pt.0).abs() < 1e-3);
        assert!((ry - screen_pt.1).abs() < 1e-3);
    }

    #[test]
    fn test_pan() {
        let mut vp = Viewport::new(800, 600);
        let orig_lat = vp.center_lat;
        vp.pan(0.0, -100.0);
        assert!(
            vp.center_lat < orig_lat,
            "panning down should move center south"
        );
    }
}
