//! Geographic primitives, coordinates, bounding boxes, and projections.

use std::f64::consts::PI;

/// Earth's radius in meters (WGS84 mean radius).
pub const EARTH_RADIUS_METERS: f64 = 6_371_008.8;

/// A point representing a latitude and longitude on Earth (WGS84).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LatLon {
    lat: f64,
    lon: f64,
}

impl LatLon {
    /// Creates a new `LatLon` coordinate.
    ///
    /// # Panics
    /// Panics in debug mode if `lat` is not in `[-90.0, 90.0]` or `lon` is not in `[-180.0, 180.0]`.
    pub fn new(lat: f64, lon: f64) -> Self {
        Self::try_new(lat, lon).expect("invalid latitude or longitude range")
    }

    /// Tries to create a new `LatLon` coordinate, returning `None` if out of range.
    pub fn try_new(lat: f64, lon: f64) -> Option<Self> {
        if !lat.is_finite() || !lon.is_finite() {
            return None;
        }
        if (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon) {
            Some(Self { lat, lon })
        } else {
            None
        }
    }

    /// Latitude in degrees `[-90.0, 90.0]`.
    #[inline]
    pub fn lat(&self) -> f64 {
        self.lat
    }

    /// Longitude in degrees `[-180.0, 180.0]`.
    #[inline]
    pub fn lon(&self) -> f64 {
        self.lon
    }

    /// Calculates the great-circle distance between two coordinates in meters using the Haversine formula.
    pub fn distance_to(&self, other: &Self) -> f64 {
        let lat1_rad = self.lat.to_radians();
        let lat2_rad = other.lat.to_radians();
        let d_lat = (other.lat - self.lat).to_radians();
        let d_lon = (other.lon - self.lon).to_radians();

        let a = (d_lat / 2.0).sin().powi(2)
            + lat1_rad.cos() * lat2_rad.cos() * (d_lon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

        EARTH_RADIUS_METERS * c
    }

    /// Calculates initial compass bearing in degrees [0, 360) from this point to `other`.
    pub fn bearing_to(&self, other: &Self) -> f64 {
        let lat1 = self.lat.to_radians();
        let lat2 = other.lat.to_radians();
        let d_lon = (other.lon - self.lon).to_radians();

        let y = d_lon.sin() * lat2.cos();
        let x = lat1.cos() * lat2.sin() - lat1.sin() * lat2.cos() * d_lon.cos();

        let rad = y.atan2(x);
        (rad.to_degrees() + 360.0) % 360.0
    }

    /// Converts this coordinate to EPSG:3857 (Spherical / Web Mercator) coordinates in meters.
    pub fn to_mercator_meters(&self) -> (f64, f64) {
        let clamped_lat = self.lat.clamp(-85.05112878, 85.05112878);
        let x = self.lon * (PI * EARTH_RADIUS_METERS / 180.0);
        let y = (PI / 4.0 + clamped_lat.to_radians() / 2.0).tan().ln() * EARTH_RADIUS_METERS;
        (x, y)
    }

    /// Converts EPSG:3857 (Spherical Mercator) meters to a `LatLon`.
    pub fn from_mercator_meters(x: f64, y: f64) -> Self {
        let lon = (x / (PI * EARTH_RADIUS_METERS)) * 180.0;
        let lat = ((2.0 * (y / EARTH_RADIUS_METERS).exp().atan() - PI / 2.0).to_degrees())
            .clamp(-85.05112878, 85.05112878);
        Self::new(lat, lon)
    }

    /// Converts coordinate to slippy map tile coordinates `(x, y)` at a given zoom level.
    pub fn to_tile_coords(&self, zoom: u8) -> (f64, f64) {
        let n = 2.0_f64.powi(zoom as i32);
        let clamped_lat = self.lat.clamp(-85.05112878, 85.05112878);
        let x = ((self.lon + 180.0) / 360.0) * n;
        let lat_rad = clamped_lat.to_radians();
        let y = (1.0 - (lat_rad.tan() + (1.0 / lat_rad.cos())).ln() / PI) / 2.0 * n;
        (x, y)
    }
}

/// An axis-aligned geographic bounding box defined by min/max latitude and longitude.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BoundingBox {
    pub min_lat: f64,
    pub min_lon: f64,
    pub max_lat: f64,
    pub max_lon: f64,
}

impl BoundingBox {
    /// Creates a new bounding box.
    pub fn new(min_lat: f64, min_lon: f64, max_lat: f64, max_lon: f64) -> Self {
        assert!(min_lat <= max_lat, "min_lat must be <= max_lat");
        assert!(min_lon <= max_lon, "min_lon must be <= max_lon");
        Self {
            min_lat,
            min_lon,
            max_lat,
            max_lon,
        }
    }

    /// Creates a single-point bounding box.
    pub fn from_point(point: LatLon) -> Self {
        Self {
            min_lat: point.lat(),
            min_lon: point.lon(),
            max_lat: point.lat(),
            max_lon: point.lon(),
        }
    }

    /// Creates a bounding box enclosing an iterator of coordinates, or `None` if empty.
    pub fn from_points<I>(points: I) -> Option<Self>
    where
        I: IntoIterator<Item = LatLon>,
    {
        let mut iter = points.into_iter();
        let first = iter.next()?;
        let mut bbox = Self::from_point(first);
        for pt in iter {
            bbox.expand_to_include(pt);
        }
        Some(bbox)
    }

    /// Returns the center coordinate of this bounding box.
    pub fn center(&self) -> LatLon {
        LatLon::new(
            (self.min_lat + self.max_lat) / 2.0,
            (self.min_lon + self.max_lon) / 2.0,
        )
    }

    /// Width in degrees of longitude.
    pub fn width_deg(&self) -> f64 {
        self.max_lon - self.min_lon
    }

    /// Height in degrees of latitude.
    pub fn height_deg(&self) -> f64 {
        self.max_lat - self.min_lat
    }

    /// Checks if a `LatLon` point is inside or on the border of this bounding box.
    pub fn contains_point(&self, point: LatLon) -> bool {
        point.lat() >= self.min_lat
            && point.lat() <= self.max_lat
            && point.lon() >= self.min_lon
            && point.lon() <= self.max_lon
    }

    /// Checks if another bounding box intersects with this one.
    pub fn intersects(&self, other: &Self) -> bool {
        self.min_lat <= other.max_lat
            && self.max_lat >= other.min_lat
            && self.min_lon <= other.max_lon
            && self.max_lon >= other.min_lon
    }

    /// Expands this bounding box to include the specified point.
    pub fn expand_to_include(&mut self, point: LatLon) {
        if point.lat() < self.min_lat {
            self.min_lat = point.lat();
        }
        if point.lat() > self.max_lat {
            self.max_lat = point.lat();
        }
        if point.lon() < self.min_lon {
            self.min_lon = point.lon();
        }
        if point.lon() > self.max_lon {
            self.max_lon = point.lon();
        }
    }

    /// Expands this bounding box to include another bounding box.
    pub fn expand_to_include_bbox(&mut self, other: &Self) {
        if other.min_lat < self.min_lat {
            self.min_lat = other.min_lat;
        }
        if other.max_lat > self.max_lat {
            self.max_lat = other.max_lat;
        }
        if other.min_lon < self.min_lon {
            self.min_lon = other.min_lon;
        }
        if other.max_lon > self.max_lon {
            self.max_lon = other.max_lon;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_and_invalid_lat_lon() {
        assert!(LatLon::try_new(52.5200, 13.4050).is_some());
        assert!(LatLon::try_new(91.0, 0.0).is_none());
        assert!(LatLon::try_new(-90.1, 0.0).is_none());
        assert!(LatLon::try_new(0.0, 181.0).is_none());
        assert!(LatLon::try_new(0.0, -180.1).is_none());
        assert!(LatLon::try_new(f64::NAN, 0.0).is_none());
    }

    #[test]
    fn test_distance_calculation() {
        // Berlin Brandenburger Tor to Alexanderplatz (~3.2 km)
        let brandenburger_tor = LatLon::new(52.5163, 13.3777);
        let alexanderplatz = LatLon::new(52.5219, 13.4132);

        let dist = brandenburger_tor.distance_to(&alexanderplatz);
        assert!((dist - 2486.0).abs() < 100.0, "distance was {dist}");

        // Distance to self must be 0
        assert_eq!(brandenburger_tor.distance_to(&brandenburger_tor), 0.0);
    }

    #[test]
    fn test_bearing() {
        let p1 = LatLon::new(0.0, 0.0);
        let north = LatLon::new(10.0, 0.0);
        let east = LatLon::new(0.0, 10.0);
        let south = LatLon::new(-10.0, 0.0);
        let west = LatLon::new(0.0, -10.0);

        assert!((p1.bearing_to(&north) - 0.0).abs() < 1e-4);
        assert!((p1.bearing_to(&east) - 90.0).abs() < 1e-4);
        assert!((p1.bearing_to(&south) - 180.0).abs() < 1e-4);
        assert!((p1.bearing_to(&west) - 270.0).abs() < 1e-4);
    }

    #[test]
    fn test_mercator_roundtrip() {
        let pt = LatLon::new(48.8566, 2.3522); // Paris
        let (x, y) = pt.to_mercator_meters();
        let reconstructed = LatLon::from_mercator_meters(x, y);

        assert!((pt.lat() - reconstructed.lat()).abs() < 1e-6);
        assert!((pt.lon() - reconstructed.lon()).abs() < 1e-6);
    }

    #[test]
    fn test_tile_coords() {
        let pt = LatLon::new(0.0, 0.0);
        let (tx, ty) = pt.to_tile_coords(1);
        // At zoom 1, (0,0) is center of map (1.0, 1.0)
        assert!((tx - 1.0).abs() < 1e-4);
        assert!((ty - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_bounding_box() {
        let mut bbox = BoundingBox::new(10.0, 20.0, 30.0, 40.0);
        assert!(bbox.contains_point(LatLon::new(15.0, 25.0)));
        assert!(!bbox.contains_point(LatLon::new(5.0, 25.0)));
        assert_eq!(bbox.center(), LatLon::new(20.0, 30.0));
        assert_eq!(bbox.width_deg(), 20.0);
        assert_eq!(bbox.height_deg(), 20.0);

        bbox.expand_to_include(LatLon::new(5.0, 45.0));
        assert_eq!(bbox.min_lat, 5.0);
        assert_eq!(bbox.max_lon, 45.0);
        assert!(bbox.contains_point(LatLon::new(5.0, 45.0)));
    }
}
