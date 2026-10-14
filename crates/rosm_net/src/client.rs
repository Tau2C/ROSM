//! OpenStreetMap API v0.6 client implementation.

use std::time::Duration;

use rosm_core::geo::BoundingBox;
use rosm_core::id::{NodeId, RelationId, WayId};
use rosm_core::primitive::{Node, Relation, Way};
use rosm_core::DataSet;

use crate::changeset::ChangesetConfig;
use crate::diff::DiffResult;
use crate::error::NetError;

/// The target OpenStreetMap server environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OsmEnvironment {
    /// Official OpenStreetMap production server (`https://api.openstreetmap.org/api/0.6/`).
    Production,
    /// Official OpenStreetMap development testing server (`https://master.apis.dev.openstreetmap.org/api/0.6/`).
    Dev,
    /// Custom OpenStreetMap API endpoint.
    Custom(String),
}

impl OsmEnvironment {
    pub fn base_url(&self) -> &str {
        match self {
            Self::Production => "https://api.openstreetmap.org/api/0.6/",
            Self::Dev => "https://master.apis.dev.openstreetmap.org/api/0.6/",
            Self::Custom(url) => url.as_str(),
        }
    }
}

/// Client for communicating with OpenStreetMap API v0.6.
pub struct OsmClient {
    environment: OsmEnvironment,
    agent: ureq::Agent,
    auth_token: Option<String>,
    user_agent: String,
}

impl OsmClient {
    /// Creates a new `OsmClient` for the given environment.
    pub fn new(environment: OsmEnvironment) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(10))
            .timeout_read(Duration::from_secs(90))
            .build();

        Self {
            environment,
            agent,
            auth_token: None,
            user_agent: format!("ROSM/{} (Rust OSM Editor)", env!("CARGO_PKG_VERSION")),
        }
    }

    /// Sets the Bearer access token for authenticated API requests.
    pub fn with_auth(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    /// Overrides the User-Agent header string.
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = user_agent.into();
        self
    }

    pub fn environment(&self) -> &OsmEnvironment {
        &self.environment
    }

    pub fn is_authenticated(&self) -> bool {
        self.auth_token.is_some()
    }

    fn apply_headers(&self, req: ureq::Request) -> ureq::Request {
        let mut r = req.set("User-Agent", &self.user_agent);
        if let Some(ref token) = self.auth_token {
            r = r.set("Authorization", &format!("Bearer {token}"));
        }
        r
    }

    /// Downloads all OSM primitives inside the given bounding box.
    pub fn download_bbox(&self, bbox: &BoundingBox) -> Result<DataSet, NetError> {
        let url = format!(
            "{}map?bbox={:.6},{:.6},{:.6},{:.6}",
            self.environment.base_url(),
            bbox.min_lon,
            bbox.min_lat,
            bbox.max_lon,
            bbox.max_lat
        );

        let req = self.apply_headers(self.agent.get(&url));
        let resp = req.call()?;
        let reader = std::io::BufReader::new(resp.into_reader());

        let dataset = rosm_io::read_osm_xml(reader)
            .map_err(|e| NetError::XmlParse(format!("Failed to parse OSM XML from {url}: {e}")))?;

        Ok(dataset)
    }

    /// Fetches an individual Node by ID from the API.
    pub fn get_node(&self, id: NodeId) -> Result<Node, NetError> {
        let url = format!("{}node/{}", self.environment.base_url(), id.0);
        let req = self.apply_headers(self.agent.get(&url));
        let resp = req.call()?;
        let reader = std::io::BufReader::new(resp.into_reader());
        let mut dataset = rosm_io::read_osm_xml(reader)
            .map_err(|e| NetError::XmlParse(format!("Failed to parse Node XML: {e}")))?;

        dataset.remove_node(id).ok_or_else(|| NetError::NotFound {
            primitive: format!("Node {}", id.0),
        })
    }

    /// Fetches an individual Way by ID from the API.
    pub fn get_way(&self, id: WayId) -> Result<Way, NetError> {
        let url = format!("{}way/{}", self.environment.base_url(), id.0);
        let req = self.apply_headers(self.agent.get(&url));
        let resp = req.call()?;
        let reader = std::io::BufReader::new(resp.into_reader());
        let mut dataset = rosm_io::read_osm_xml(reader)
            .map_err(|e| NetError::XmlParse(format!("Failed to parse Way XML: {e}")))?;

        dataset.remove_way(id).ok_or_else(|| NetError::NotFound {
            primitive: format!("Way {}", id.0),
        })
    }

    /// Fetches an individual Relation by ID from the API.
    pub fn get_relation(&self, id: RelationId) -> Result<Relation, NetError> {
        let url = format!("{}relation/{}", self.environment.base_url(), id.0);
        let req = self.apply_headers(self.agent.get(&url));
        let resp = req.call()?;
        let reader = std::io::BufReader::new(resp.into_reader());
        let mut dataset = rosm_io::read_osm_xml(reader)
            .map_err(|e| NetError::XmlParse(format!("Failed to parse Relation XML: {e}")))?;

        dataset
            .remove_relation(id)
            .ok_or_else(|| NetError::NotFound {
                primitive: format!("Relation {}", id.0),
            })
    }

    /// Opens a new changeset with the given configuration tags. Returns the created changeset ID.
    pub fn create_changeset(&self, config: &ChangesetConfig) -> Result<i64, NetError> {
        if !self.is_authenticated() {
            return Err(NetError::Auth(
                "Authentication required to create a changeset".into(),
            ));
        }
        if config.comment.trim().is_empty() {
            return Err(NetError::InvalidData(
                "A non-empty changeset comment is required".into(),
            ));
        }

        let url = format!("{}changeset/create", self.environment.base_url());
        let body = config.to_xml();

        let req = self
            .apply_headers(self.agent.put(&url))
            .set("Content-Type", "application/xml");

        let resp = req.send_string(&body)?;
        let id_str = resp.into_string()?;
        let changeset_id: i64 = id_str.trim().parse().map_err(|e| {
            NetError::XmlParse(format!("Invalid changeset ID received: '{id_str}': {e}"))
        })?;

        Ok(changeset_id)
    }

    /// Uploads an OsmChange (.osc) diff to an open changeset. Returns the parsed `DiffResult`.
    pub fn upload_diff(&self, changeset_id: i64, osc_xml: &str) -> Result<DiffResult, NetError> {
        if !self.is_authenticated() {
            return Err(NetError::Auth(
                "Authentication required to upload changes".into(),
            ));
        }

        let url = format!(
            "{}changeset/{}/upload",
            self.environment.base_url(),
            changeset_id
        );

        let req = self
            .apply_headers(self.agent.post(&url))
            .set("Content-Type", "application/xml");

        let resp = req.send_string(osc_xml)?;
        let resp_xml = resp.into_string()?;
        DiffResult::parse(&resp_xml)
    }

    /// Closes an open changeset.
    pub fn close_changeset(&self, changeset_id: i64) -> Result<(), NetError> {
        if !self.is_authenticated() {
            return Err(NetError::Auth(
                "Authentication required to close a changeset".into(),
            ));
        }

        let url = format!(
            "{}changeset/{}/close",
            self.environment.base_url(),
            changeset_id
        );

        let req = self.apply_headers(self.agent.put(&url));
        req.call()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_environment_urls() {
        let prod = OsmClient::new(OsmEnvironment::Production);
        assert_eq!(
            prod.environment().base_url(),
            "https://api.openstreetmap.org/api/0.6/"
        );

        let dev = OsmClient::new(OsmEnvironment::Dev);
        assert_eq!(
            dev.environment().base_url(),
            "https://master.apis.dev.openstreetmap.org/api/0.6/"
        );

        let custom = OsmClient::new(OsmEnvironment::Custom(
            "http://localhost:3000/api/0.6/".into(),
        ));
        assert_eq!(
            custom.environment().base_url(),
            "http://localhost:3000/api/0.6/"
        );
    }

    #[test]
    fn test_unauthenticated_changeset_rejection() {
        let client = OsmClient::new(OsmEnvironment::Dev);
        let config = ChangesetConfig::new("Test comment");
        let result = client.create_changeset(&config);
        assert!(matches!(result, Err(NetError::Auth(_))));
    }

    #[test]
    fn test_empty_comment_changeset_rejection() {
        let client = OsmClient::new(OsmEnvironment::Dev).with_auth("mock_token");
        let config = ChangesetConfig::new("   ");
        let result = client.create_changeset(&config);
        assert!(matches!(result, Err(NetError::InvalidData(_))));
    }
}
