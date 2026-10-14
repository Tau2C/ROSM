//! Changeset metadata, configuration, and XML serialization.

use std::collections::HashMap;

/// Configuration parameters for creating an OSM changeset.
#[derive(Debug, Clone)]
pub struct ChangesetConfig {
    pub comment: String,
    pub source: Option<String>,
    pub imagery_used: Option<String>,
    pub created_by: String,
    pub extra_tags: HashMap<String, String>,
}

impl ChangesetConfig {
    pub fn new(comment: impl Into<String>) -> Self {
        Self {
            comment: comment.into(),
            source: None,
            imagery_used: None,
            created_by: format!("ROSM {}", env!("CARGO_PKG_VERSION")),
            extra_tags: HashMap::new(),
        }
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn with_imagery(mut self, imagery: impl Into<String>) -> Self {
        self.imagery_used = Some(imagery.into());
        self
    }

    pub fn with_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_tags.insert(key.into(), value.into());
        self
    }

    /// Serializes the changeset creation payload to XML.
    pub fn to_xml(&self) -> String {
        let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        xml.push_str("<osm version=\"0.6\" generator=\"ROSM\">\n");
        xml.push_str("  <changeset>\n");

        xml.push_str(&format!(
            "    <tag k=\"comment\" v=\"{}\"/>\n",
            quick_xml::escape::escape(&self.comment)
        ));
        xml.push_str(&format!(
            "    <tag k=\"created_by\" v=\"{}\"/>\n",
            quick_xml::escape::escape(&self.created_by)
        ));

        if let Some(ref src) = self.source {
            xml.push_str(&format!(
                "    <tag k=\"source\" v=\"{}\"/>\n",
                quick_xml::escape::escape(src)
            ));
        }

        if let Some(ref img) = self.imagery_used {
            xml.push_str(&format!(
                "    <tag k=\"imagery_used\" v=\"{}\"/>\n",
                quick_xml::escape::escape(img)
            ));
        }

        for (k, v) in &self.extra_tags {
            xml.push_str(&format!(
                "    <tag k=\"{}\" v=\"{}\"/>\n",
                quick_xml::escape::escape(k),
                quick_xml::escape::escape(v)
            ));
        }

        xml.push_str("  </changeset>\n");
        xml.push_str("</osm>\n");
        xml
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_changeset_xml_serialization() {
        let config = ChangesetConfig::new("Surveying local shops")
            .with_source("survey")
            .with_imagery("Bing")
            .with_tag("bot", "no");

        let xml = config.to_xml();
        assert!(xml.contains("<tag k=\"comment\" v=\"Surveying local shops\"/>"));
        assert!(xml.contains("<tag k=\"source\" v=\"survey\"/>"));
        assert!(xml.contains("<tag k=\"imagery_used\" v=\"Bing\"/>"));
        assert!(xml.contains("<tag k=\"bot\" v=\"no\"/>"));
        assert!(xml.contains("created_by"));
    }
}
