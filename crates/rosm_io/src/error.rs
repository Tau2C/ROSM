//! Errors for OSM file I/O and format parsing.

use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IoError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("XML parsing error: {0}")]
    Xml(#[from] quick_xml::Error),

    #[error("Invalid coordinate: {0}")]
    InvalidCoordinate(String),

    #[error("Missing required attribute '{0}' on element <{1}>")]
    MissingAttribute(&'static str, &'static str),

    #[error("Number parse error on attribute '{0}': {1}")]
    ParseNumber(&'static str, String),

    #[error("Unknown element or member type: {0}")]
    UnknownType(String),
}
