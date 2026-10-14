//! Overpass QL query engine and API client.

use std::time::Duration;

use rosm_core::geo::BoundingBox;
use rosm_core::DataSet;

use crate::error::NetError;

/// Public Overpass API server endpoints.
pub mod endpoints {
    pub const MAIN: &str = "https://overpass-api.de/api/interpreter";
    pub const KUMI: &str = "https://overpass.kumi.systems/api/interpreter";
    pub const MAIL_RU: &str = "https://maps.mail.ru/osm/tools/overpass/api/interpreter";
}

/// Client for running Overpass QL queries and streaming results into `DataSet`.
pub struct OverpassClient {
    endpoint: String,
    agent: ureq::Agent,
    user_agent: String,
}

impl Default for OverpassClient {
    fn default() -> Self {
        Self::new(endpoints::MAIN)
    }
}

impl OverpassClient {
    /// Creates a new `OverpassClient` with the specified interpreter endpoint URL.
    pub fn new(endpoint: impl Into<String>) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(10))
            .timeout_read(Duration::from_secs(120))
            .build();

        Self {
            endpoint: endpoint.into(),
            agent,
            user_agent: format!("ROSM/{} (Rust OSM Editor)", env!("CARGO_PKG_VERSION")),
        }
    }

    /// Sets a custom user agent.
    pub fn with_user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = ua.into();
        self
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Executes an Overpass QL query and parses the returned OSM XML into a `DataSet`.
    pub fn query(&self, ql: &str) -> Result<DataSet, NetError> {
        let trimmed = ql.trim();
        // Ensure XML output format is used
        let effective_query = if !trimmed.contains("[out:xml]") && !trimmed.contains("[out:json]") {
            format!("[out:xml];\n{trimmed}")
        } else {
            trimmed.to_string()
        };

        let req = self
            .agent
            .post(&self.endpoint)
            .set("User-Agent", &self.user_agent)
            .set("Accept", "application/xml, text/xml");

        let resp = req.send_form(&[("data", &effective_query)])?;
        let reader = std::io::BufReader::new(resp.into_reader());

        let dataset = rosm_io::read_osm_xml(reader)
            .map_err(|e| NetError::XmlParse(format!("Failed to parse Overpass response: {e}")))?;

        Ok(dataset)
    }

    /// Generates standard Overpass QL for fetching elements matching a key/value filter inside a bounding box.
    pub fn build_bbox_query(bbox: &BoundingBox, filter: Option<&str>, timeout_secs: u32) -> String {
        let filter_clause = filter.unwrap_or("");
        format!(
            "[out:xml][timeout:{}];\n(\n  node{}({:.6},{:.6},{:.6},{:.6});\n  way{}({:.6},{:.6},{:.6},{:.6});\n  relation{}({:.6},{:.6},{:.6},{:.6});\n);\nout meta;\n>;\nout meta qt;\n",
            timeout_secs,
            filter_clause,
            bbox.min_lat,
            bbox.min_lon,
            bbox.max_lat,
            bbox.max_lon,
            filter_clause,
            bbox.min_lat,
            bbox.min_lon,
            bbox.max_lat,
            bbox.max_lon,
            filter_clause,
            bbox.min_lat,
            bbox.min_lon,
            bbox.max_lat,
            bbox.max_lon
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overpass_query_builder() {
        let bbox = BoundingBox::new(51.50, -0.15, 51.52, -0.10);
        let ql = OverpassClient::build_bbox_query(&bbox, Some("[\"highway\"]"), 30);

        assert!(ql.contains("[out:xml][timeout:30];"));
        assert!(ql.contains("node[\"highway\"](51.500000,-0.150000,51.520000,-0.100000);"));
        assert!(ql.contains("way[\"highway\"](51.500000,-0.150000,51.520000,-0.100000);"));
        assert!(ql.contains(">;\nout meta qt;"));
    }
}
