//! OAuth 2.0 Authorization Code flow with PKCE (RFC 7636) for OpenStreetMap API authentication.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ring::digest::{digest, SHA256};
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};

use crate::error::NetError;

/// Cryptographic PKCE challenge containing the verifier and challenge strings.
#[derive(Debug, Clone)]
pub struct PkceChallenge {
    pub code_verifier: String,
    pub code_challenge: String,
}

impl PkceChallenge {
    /// Generates a new cryptographically random PKCE code verifier and S256 code challenge.
    pub fn new() -> Result<Self, NetError> {
        let rng = SystemRandom::new();
        let mut random_bytes = [0u8; 32];
        rng.fill(&mut random_bytes).map_err(|_| {
            NetError::Auth("Failed to generate secure random bytes for PKCE".to_string())
        })?;

        let code_verifier = URL_SAFE_NO_PAD.encode(random_bytes);
        let challenge_hash = digest(&SHA256, code_verifier.as_bytes());
        let code_challenge = URL_SAFE_NO_PAD.encode(challenge_hash.as_ref());

        Ok(Self {
            code_verifier,
            code_challenge,
        })
    }
}

/// Configuration parameters for OAuth 2.0 communication with an OSM server.
#[derive(Debug, Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    pub auth_url: String,
    pub token_url: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
}

impl OAuthConfig {
    pub fn production(client_id: impl Into<String>, redirect_uri: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
            auth_url: "https://www.openstreetmap.org/oauth2/authorize".to_string(),
            token_url: "https://www.openstreetmap.org/oauth2/token".to_string(),
            redirect_uri: redirect_uri.into(),
            scopes: vec!["read_prefs".to_string(), "write_api".to_string()],
        }
    }

    pub fn dev(client_id: impl Into<String>, redirect_uri: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
            auth_url: "https://master.apis.dev.openstreetmap.org/oauth2/authorize".to_string(),
            token_url: "https://master.apis.dev.openstreetmap.org/oauth2/token".to_string(),
            redirect_uri: redirect_uri.into(),
            scopes: vec!["read_prefs".to_string(), "write_api".to_string()],
        }
    }
}

/// Response returned by the token endpoint upon successful authorization code exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub scope: Option<String>,
}

/// OAuth 2.0 Client managing URL creation and token acquisition.
pub struct OAuthClient {
    pub config: OAuthConfig,
}

impl OAuthClient {
    pub fn new(config: OAuthConfig) -> Self {
        Self { config }
    }

    /// Constructs the full authorization URL for the user to open in their web browser.
    pub fn build_authorization_url(
        &self,
        pkce: &PkceChallenge,
        state: &str,
    ) -> Result<String, NetError> {
        let scope_str = self.config.scopes.join(" ");
        let mut url = url::Url::parse(&self.config.auth_url)?;
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &self.config.client_id)
            .append_pair("redirect_uri", &self.config.redirect_uri)
            .append_pair("scope", &scope_str)
            .append_pair("code_challenge", &pkce.code_challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("state", state);

        Ok(url.to_string())
    }

    /// Exchanges an authorization code received via redirect for an access token.
    pub fn exchange_code_for_token(
        &self,
        code: &str,
        code_verifier: &str,
    ) -> Result<TokenResponse, NetError> {
        let resp = ureq::post(&self.config.token_url)
            .set("Accept", "application/json")
            .send_form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("client_id", &self.config.client_id),
                ("redirect_uri", &self.config.redirect_uri),
                ("code_verifier", code_verifier),
            ])?;

        let token: TokenResponse = serde_json::from_reader(resp.into_reader())
            .map_err(|e| NetError::Auth(format!("Failed to parse token response: {e}")))?;

        Ok(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pkce_generation() {
        let pkce = PkceChallenge::new().expect("PKCE generation should succeed");
        assert!(!pkce.code_verifier.is_empty());
        assert!(!pkce.code_challenge.is_empty());
        assert_ne!(pkce.code_verifier, pkce.code_challenge);

        // Verify S256 manual computation matches
        let hash = digest(&SHA256, pkce.code_verifier.as_bytes());
        let expected_challenge = URL_SAFE_NO_PAD.encode(hash.as_ref());
        assert_eq!(pkce.code_challenge, expected_challenge);
    }

    #[test]
    fn test_authorization_url_building() {
        let config =
            OAuthConfig::production("rosm_client_123", "http://127.0.0.1:8111/oauth/callback");
        let client = OAuthClient::new(config);
        let pkce = PkceChallenge::new().unwrap();
        let url = client.build_authorization_url(&pkce, "xyz_state").unwrap();

        assert!(url.starts_with("https://www.openstreetmap.org/oauth2/authorize?"));
        assert!(url.contains("client_id=rosm_client_123"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("code_challenge="));
        assert!(url.contains("state=xyz_state"));
    }
}
