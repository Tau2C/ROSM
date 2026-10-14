//! OpenStreetMap tag collection and inspection helpers.

use std::collections::BTreeMap;
use std::fmt;

/// A collection of key-value tags associated with an OSM primitive.
/// Stored in sorted order (`BTreeMap`) for deterministic serialization, diffing, and display.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Tags {
    inner: BTreeMap<String, String>,
}

impl Tags {
    /// Creates an empty collection of tags.
    pub fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    /// Number of tags.
    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if there are no tags.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Clears all tags.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Gets the value for a tag key if it exists.
    #[inline]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.inner.get(key).map(|s| s.as_str())
    }

    /// Checks if a key exists.
    #[inline]
    pub fn contains_key(&self, key: &str) -> bool {
        self.inner.contains_key(key)
    }

    /// Checks if a key has a specific value.
    #[inline]
    pub fn has_value(&self, key: &str, value: &str) -> bool {
        self.get(key) == Some(value)
    }

    /// Inserts or updates a tag key-value pair.
    /// Returns previous value if it was replaced.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> Option<String> {
        self.inner.insert(key.into(), value.into())
    }

    /// Removes a tag by key, returning its value if it was present.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.inner.remove(key)
    }

    /// Returns an iterator over key-value pairs sorted alphabetically by key.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.inner.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Returns the `name` tag if present.
    pub fn name(&self) -> Option<&str> {
        self.get("name")
    }

    /// Checks if this is a highway (e.g. road, path, cycleway).
    pub fn is_highway(&self) -> bool {
        self.contains_key("highway")
    }

    /// Checks if this represents a building.
    pub fn is_building(&self) -> bool {
        self.get("building").is_some_and(|v| v != "no")
    }

    /// Checks if this represents a waterway.
    pub fn is_waterway(&self) -> bool {
        self.contains_key("waterway")
    }

    /// Checks if this represents a railway.
    pub fn is_railway(&self) -> bool {
        self.contains_key("railway")
    }

    /// Checks if this represents a landuse polygon.
    pub fn is_landuse(&self) -> bool {
        self.contains_key("landuse")
    }

    /// Checks if this represents a natural feature.
    pub fn is_natural(&self) -> bool {
        self.contains_key("natural")
    }

    /// Checks if this is a oneway street (e.g. `oneway=yes`, `oneway=1`, `oneway=true` or roundabouts).
    pub fn is_oneway(&self) -> bool {
        match self.get("oneway") {
            Some("yes" | "1" | "true") => true,
            Some("-1" | "reverse") => true,
            _ => self.has_value("junction", "roundabout"),
        }
    }

    /// Returns parsed `layer` attribute or 0 if omitted/invalid.
    pub fn layer(&self) -> i32 {
        self.get("layer")
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0)
    }

    /// Determines if a closed way with these tags represents an area (polygon) according to OSM semantics.
    pub fn is_area(&self) -> bool {
        if let Some(val) = self.get("area") {
            return val == "yes" || val == "1" || val == "true";
        }
        self.is_building()
            || self.is_landuse()
            || self.contains_key("leisure")
            || self.contains_key("amenity")
            || self.contains_key("boundary")
            || self.has_value("natural", "wood")
            || self.has_value("natural", "water")
            || self.has_value("natural", "scrub")
            || self.has_value("natural", "wetland")
    }

    /// Returns differences between two tag collections.
    pub fn diff(&self, other: &Self) -> TagDiff {
        let mut added = BTreeMap::new();
        let mut removed = BTreeMap::new();
        let mut modified = BTreeMap::new();

        for (k, v) in &self.inner {
            match other.inner.get(k) {
                None => {
                    removed.insert(k.clone(), v.clone());
                }
                Some(other_v) if other_v != v => {
                    modified.insert(k.clone(), (v.clone(), other_v.clone()));
                }
                _ => {}
            }
        }

        for (k, v) in &other.inner {
            if !self.inner.contains_key(k) {
                added.insert(k.clone(), v.clone());
            }
        }

        TagDiff {
            added,
            removed,
            modified,
        }
    }
}

/// Differences between two Tag collections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagDiff {
    pub added: BTreeMap<String, String>,
    pub removed: BTreeMap<String, String>,
    pub modified: BTreeMap<String, (String, String)>,
}

impl TagDiff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.modified.is_empty()
    }
}

impl<K, V> FromIterator<(K, V)> for Tags
where
    K: Into<String>,
    V: Into<String>,
{
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        let mut tags = Self::new();
        for (k, v) in iter {
            tags.insert(k, v);
        }
        tags
    }
}

impl fmt::Display for Tags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{")?;
        for (i, (k, v)) in self.inner.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{k}={v}")?;
        }
        write!(f, "}}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tags_basic_operations() {
        let mut tags = Tags::new();
        assert!(tags.is_empty());

        tags.insert("highway", "residential");
        tags.insert("name", "Main Street");
        tags.insert("maxspeed", "50");

        assert_eq!(tags.len(), 3);
        assert_eq!(tags.get("highway"), Some("residential"));
        assert_eq!(tags.name(), Some("Main Street"));
        assert!(tags.is_highway());
        assert!(!tags.is_building());

        assert_eq!(tags.remove("maxspeed"), Some("50".to_string()));
        assert_eq!(tags.len(), 2);
    }

    #[test]
    fn test_is_area_heuristics() {
        let mut building_tags = Tags::new();
        building_tags.insert("building", "yes");
        assert!(building_tags.is_area());

        let mut highway_tags = Tags::new();
        highway_tags.insert("highway", "pedestrian");
        assert!(!highway_tags.is_area());

        highway_tags.insert("area", "yes");
        assert!(highway_tags.is_area());

        let mut overridden_building = Tags::new();
        overridden_building.insert("building", "yes");
        overridden_building.insert("area", "no");
        assert!(!overridden_building.is_area());
    }

    #[test]
    fn test_tag_diff() {
        let mut t1 = Tags::new();
        t1.insert("name", "Old Name");
        t1.insert("highway", "residential");
        t1.insert("surface", "asphalt");

        let mut t2 = Tags::new();
        t2.insert("name", "New Name");
        t2.insert("highway", "residential");
        t2.insert("maxspeed", "30");

        let diff = t1.diff(&t2);
        assert!(!diff.is_empty());
        assert_eq!(diff.removed.get("surface"), Some(&"asphalt".to_string()));
        assert_eq!(diff.added.get("maxspeed"), Some(&"30".to_string()));
        assert_eq!(
            diff.modified.get("name"),
            Some(&("Old Name".to_string(), "New Name".to_string()))
        );
    }
}
