// -------------------------------------------------------------------------------------------------
//  Copyright (C) 2015-2025 Nautech Systems Pty Ltd. All rights reserved.
//  https://nautechsystems.io
//
//  Licensed under the GNU Lesser General Public License Version 3.0 (the "License");
//  You may not use this file except in compliance with the License.
//  You may obtain a copy of the License at https://www.gnu.org/licenses/lgpl-3.0.en.html
//
//  Unless required by applicable law or agreed to in writing, software
//  distributed under the License is distributed on an "AS IS" BASIS,
//  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//  See the License for the specific language governing permissions and
//  limitations under the License.
// -------------------------------------------------------------------------------------------------

//! HTTP client for the Lighter DEX REST API.
//!
//! This module provides a raw HTTP client that handles:
//! - Rate limiting
//! - Request/response logging
//! - Automatic retries
//! - Authentication headers
//! - Error handling

use std::{
    collections::HashMap,
    fmt::{Debug, Formatter},
    num::NonZeroU32,
    sync::LazyLock,
};

use nautilus_core::consts::NAUTILUS_USER_AGENT;
use nautilus_network::{
    http::{HttpClient, Method, USER_AGENT},
    ratelimiter::quota::Quota,
    retry::{RetryConfig, RetryManager},
};
use serde::de::DeserializeOwned;
use tokio_util::sync::CancellationToken;
use tracing::{debug, trace};

use crate::{common::LighterEnvironment, error::LighterError, http::retry::LighterRetryPolicy};

/// Default rate limit for Lighter API.
///
/// Lighter API rate limits (based on typical DEX patterns):
/// - Public endpoints: ~10 requests per second
/// - Private endpoints: ~5 requests per second
///
/// We use a conservative 5 requests per second as the default global quota.
pub static LIGHTER_REST_QUOTA: LazyLock<Quota> = LazyLock::new(|| {
    Quota::per_second(NonZeroU32::new(5).unwrap()).expect("non-zero per-second quota")
});

/// Authentication header name for Lighter API.
const AUTH_HEADER: &str = "X-Lighter-Auth";

/// Provides a raw HTTP client for the Lighter DEX REST API.
///
/// This client handles rate-limiting, retries, authentication, and basic
/// error handling for all HTTP requests to the Lighter API.
///
/// # Authentication
///
/// Some endpoints require authentication via the `X-Lighter-Auth` header.
/// The auth token can be provided during client construction or set later.
///
/// # Example
///
/// ```no_run
/// use nautilus_lighter::http::LighterRawHttpClient;
/// use nautilus_lighter::common::LighterEnvironment;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let client = LighterRawHttpClient::new(
///         LighterEnvironment::Testnet,
///         None,  // auth_token
///         Some(30),  // timeout_secs
///         None,  // proxy_url
///         None,  // retry_config
///     )?;
///
///     // Make requests...
///     Ok(())
/// }
/// ```
pub struct LighterRawHttpClient {
    environment: LighterEnvironment,
    base_url: String,
    client: HttpClient,
    retry_manager: RetryManager<LighterError>,
    retry_policy: LighterRetryPolicy,
    cancellation_token: CancellationToken,
    auth_token: Option<String>,
}

impl Default for LighterRawHttpClient {
    fn default() -> Self {
        Self::new(LighterEnvironment::Testnet, None, Some(30), None, None)
            .expect("Failed to create default LighterRawHttpClient")
    }
}

impl Debug for LighterRawHttpClient {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(stringify!(LighterRawHttpClient))
            .field("environment", &self.environment)
            .field("base_url", &self.base_url)
            .field("has_auth", &self.auth_token.is_some())
            .finish_non_exhaustive()
    }
}

impl LighterRawHttpClient {
    /// Creates a new [`LighterRawHttpClient`].
    ///
    /// # Parameters
    ///
    /// - `environment`: Lighter environment (Mainnet or Testnet)
    /// - `auth_token`: Optional authentication token for private endpoints
    /// - `timeout_secs`: Request timeout in seconds (default: 30)
    /// - `proxy_url`: Optional proxy URL for HTTP requests
    /// - `retry_config`: Optional custom retry configuration
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be initialized.
    pub fn new(
        environment: LighterEnvironment,
        auth_token: Option<String>,
        timeout_secs: Option<u64>,
        proxy_url: Option<String>,
        retry_config: Option<RetryConfig>,
    ) -> Result<Self, LighterError> {
        let base_url = environment.http_url().to_string();

        let retry_policy = retry_config
            .map(LighterRetryPolicy::from_retry_config)
            .unwrap_or_else(|| LighterRetryPolicy::for_latency_tier(300));
        let retry_manager = retry_policy.retry_manager();

        // Build default headers
        let mut headers = HashMap::new();
        headers.insert(USER_AGENT.to_string(), NAUTILUS_USER_AGENT.to_string());
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        // Add authentication header if token is provided
        if let Some(ref token) = auth_token {
            headers.insert(AUTH_HEADER.to_string(), token.clone());
        }

        let client = HttpClient::new(
            headers,
            vec![],                    // No specific headers to extract
            vec![],                    // No keyed quotas
            Some(*LIGHTER_REST_QUOTA), // Global rate limit
            timeout_secs,
            proxy_url,
        )
        .map_err(|e| LighterError::Http(format!("Failed to create HTTP client: {e}")))?;

        debug!("Created LighterRawHttpClient for {:?}", environment);

        Ok(Self {
            environment,
            base_url,
            client,
            retry_manager,
            retry_policy,
            cancellation_token: CancellationToken::new(),
            auth_token,
        })
    }

    /// Get the current environment.
    #[must_use]
    pub const fn environment(&self) -> LighterEnvironment {
        self.environment
    }

    /// Get the base URL.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Check if authentication token is set.
    #[must_use]
    pub fn has_auth(&self) -> bool {
        self.auth_token.is_some()
    }

    /// Set or update the authentication token.
    ///
    /// Note: This will be used in future requests. For the change to take effect
    /// immediately, you may need to create a new client instance.
    pub fn set_auth_token(&mut self, token: String) {
        self.auth_token = Some(token);
    }

    /// Clear the authentication token.
    pub fn clear_auth_token(&mut self) {
        self.auth_token = None;
    }

    /// Cancel all pending HTTP requests.
    pub fn cancel_all_requests(&self) {
        debug!("Canceling all pending Lighter HTTP requests");
        self.cancellation_token.cancel();
    }

    /// Get the cancellation token.
    pub fn cancellation_token(&self) -> &CancellationToken {
        &self.cancellation_token
    }

    /// Creates a new [`LighterRawHttpClient`] with a custom base URL.
    ///
    /// This is primarily used for testing with mock servers.
    ///
    /// # Parameters
    ///
    /// - `base_url`: Custom base URL for all requests
    /// - `auth_token`: Optional authentication token for private endpoints
    /// - `timeout_secs`: Request timeout in seconds (default: 30)
    /// - `proxy_url`: Optional proxy URL for HTTP requests
    /// - `retry_config`: Optional custom retry configuration
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be initialized.
    pub fn with_base_url(
        base_url: &str,
        auth_token: Option<String>,
        timeout_secs: Option<u64>,
        proxy_url: Option<String>,
        retry_config: Option<RetryConfig>,
    ) -> Result<Self, LighterError> {
        let retry_policy = retry_config
            .map(LighterRetryPolicy::from_retry_config)
            .unwrap_or_else(|| LighterRetryPolicy::for_latency_tier(300));
        let retry_manager = retry_policy.retry_manager();

        // Build default headers
        let mut headers = HashMap::new();
        headers.insert(USER_AGENT.to_string(), NAUTILUS_USER_AGENT.to_string());
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        // Add authentication header if token is provided
        if let Some(ref token) = auth_token {
            headers.insert(AUTH_HEADER.to_string(), token.clone());
        }

        let client = HttpClient::new(
            headers,
            vec![],                    // No specific headers to extract
            vec![],                    // No keyed quotas
            Some(*LIGHTER_REST_QUOTA), // Global rate limit
            timeout_secs,
            proxy_url,
        )
        .map_err(|e| LighterError::Http(format!("Failed to create HTTP client: {e}")))?;

        debug!(
            "Created LighterRawHttpClient with custom base URL: {}",
            base_url
        );

        Ok(Self {
            environment: LighterEnvironment::Testnet, // Default for custom URL
            base_url: base_url.to_string(),
            client,
            retry_manager,
            retry_policy,
            cancellation_token: CancellationToken::new(),
            auth_token,
        })
    }

    /// Send a GET request to a Lighter API endpoint.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails
    /// - The response status is not successful
    /// - The response cannot be deserialized
    /// - The request is canceled
    pub async fn get<T>(
        &self,
        endpoint: &str,
        query_params: Option<&str>,
    ) -> Result<T, LighterError>
    where
        T: DeserializeOwned,
    {
        self.send_request(Method::GET, endpoint, query_params, None)
            .await
    }

    /// Send a POST request to a Lighter API endpoint.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails
    /// - The response status is not successful
    /// - The response cannot be deserialized
    /// - The request is canceled
    pub async fn post<T, B>(&self, endpoint: &str, body: Option<B>) -> Result<T, LighterError>
    where
        T: DeserializeOwned,
        B: serde::Serialize,
    {
        let body_bytes = if let Some(body) = body {
            Some(
                serde_json::to_vec(&body)
                    .map_err(|e| LighterError::Parse(format!("Failed to serialize body: {e}")))?,
            )
        } else {
            None
        };

        self.send_request(Method::POST, endpoint, None, body_bytes)
            .await
    }

    /// Send an HTTP request to a Lighter API endpoint.
    ///
    /// This is the core request method that handles:
    /// - URL construction
    /// - Request execution with retry logic
    /// - Response validation
    /// - JSON deserialization
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or response is invalid.
    async fn send_request<T>(
        &self,
        method: Method,
        endpoint: &str,
        query_params: Option<&str>,
        body: Option<Vec<u8>>,
    ) -> Result<T, LighterError>
    where
        T: DeserializeOwned,
    {
        let url = if let Some(params) = query_params {
            format!("{}{endpoint}?{params}", self.base_url)
        } else {
            format!("{}{endpoint}", self.base_url)
        };

        trace!("Lighter HTTP {} {}", method, url);

        let operation = || async {
            // Add auth header if token is available
            let headers = self.auth_token.as_ref().map(|token| {
                let mut headers = HashMap::new();
                headers.insert(AUTH_HEADER.to_string(), token.clone());
                headers
            });

            let response = self
                .client
                .request_with_ustr_keys(
                    method.clone(),
                    url.clone(),
                    None,    // Query params already in URL
                    headers, // Pass auth header dynamically
                    body.clone(),
                    None, // Use default timeout
                    None, // Use global rate limit
                )
                .await
                .map_err(|e| LighterError::Http(format!("HTTP request failed: {e}")))?;

            // Check HTTP status
            if !response.status.is_success() {
                let status = response.status.as_u16();
                let body_str = String::from_utf8_lossy(&response.body);

                return Err(match status {
                    401 | 403 => LighterError::Auth(format!("Authentication failed: {body_str}")),
                    429 => LighterError::RateLimit(format!("Rate limit exceeded: {body_str}")),
                    408 | 504 => LighterError::Timeout(format!("Request timeout: {body_str}")),
                    _ => LighterError::Http(format!("HTTP {status}: {body_str}")),
                });
            }

            Ok(response)
        };

        // Execute with retry. The policy retries only transient transport/status failures,
        // timeouts, and 429 rate limits; auth/config/parse/signing/business rejects fail fast.
        let response = self
            .retry_manager
            .execute_with_retry_with_cancel(
                endpoint,
                operation,
                |error| self.retry_policy.should_retry(error),
                LighterRetryPolicy::create_retry_manager_error,
                &self.cancellation_token,
            )
            .await?;

        trace!("Lighter HTTP response: {} bytes", response.body.len());

        // Deserialize response
        serde_json::from_slice(&response.body).map_err(|e| {
            LighterError::Parse(format!(
                "Failed to deserialize response: {e}. Body: {}",
                String::from_utf8_lossy(&response.body)
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client =
            LighterRawHttpClient::new(LighterEnvironment::Testnet, None, Some(30), None, None)
                .unwrap();

        assert_eq!(client.environment(), LighterEnvironment::Testnet);
        assert!(client.base_url().contains("testnet"));
        assert!(!client.has_auth());
    }

    #[test]
    fn test_client_with_auth() {
        let client = LighterRawHttpClient::new(
            LighterEnvironment::Mainnet,
            Some("test-token".to_string()),
            Some(30),
            None,
            None,
        )
        .unwrap();

        assert!(client.has_auth());
    }

    #[test]
    fn test_set_auth_token() {
        let mut client =
            LighterRawHttpClient::new(LighterEnvironment::Testnet, None, Some(30), None, None)
                .unwrap();

        assert!(!client.has_auth());

        client.set_auth_token("new-token".to_string());
        assert!(client.has_auth());

        client.clear_auth_token();
        assert!(!client.has_auth());
    }

    #[test]
    fn test_default_client() {
        let client = LighterRawHttpClient::default();
        assert_eq!(client.environment(), LighterEnvironment::Testnet);
    }
}
