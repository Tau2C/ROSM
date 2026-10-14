//! `rosm_io`: OpenStreetMap format I/O (OSM XML, OSM Change .osc, etc.).

pub mod error;
pub mod xml;

pub use error::IoError;
pub use xml::{read_osm_xml, write_osm_xml};
