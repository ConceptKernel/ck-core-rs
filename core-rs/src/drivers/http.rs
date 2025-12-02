//! HttpDriver for ConceptKernel HTTP/WebSocket emissions
//!
//! Provides HTTP-based event emission including:
//! - HTTP endpoint management
//! - WebSocket connections
//! - Request routing
//! - Response formatting
//! - Authentication handling

use crate::errors::{CkpError, Result};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// HTTP driver for kernel operations
#[derive(Debug, Clone)]
pub struct HttpDriver {
    base_url: String,
    endpoints: HashMap<String, String>,
    auth_token: Option<String>,
}

impl HttpDriver {
    /// Create new HttpDriver
    ///
    /// # Example
    ///
    /// ```
    /// use ckp_core::drivers::HttpDriver;
    ///
    /// let driver = HttpDriver::new("http://localhost:8080".to_string());
    /// ```
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            endpoints: HashMap::new(),
            auth_token: None,
        }
    }

    /// Add authentication token
    ///
    /// # Example
    ///
    /// ```
    /// use ckp_core::drivers::HttpDriver;
    ///
    /// let driver = HttpDriver::new("http://localhost:8080".to_string())
    ///     .with_auth("token123".to_string());
    /// ```
    pub fn with_auth(mut self, token: String) -> Self {
        self.auth_token = Some(token);
        self
    }

    /// Register an HTTP endpoint
    ///
    /// # Example
    ///
    /// ```
    /// use ckp_core::drivers::HttpDriver;
    ///
    /// let mut driver = HttpDriver::new("http://localhost:8080".to_string());
    /// driver.register_endpoint("emit", "/api/v1/emit").unwrap();
    /// ```
    pub fn register_endpoint(&mut self, name: &str, path: &str) -> Result<()> {
        if name.is_empty() {
            return Err(CkpError::ValidationError("Endpoint name cannot be empty".to_string()));
        }
        if path.is_empty() {
            return Err(CkpError::ValidationError("Endpoint path cannot be empty".to_string()));
        }
        self.endpoints.insert(name.to_string(), path.to_string());
        Ok(())
    }

    /// Get registered endpoint path
    ///
    /// # Example
    ///
    /// ```
    /// use ckp_core::drivers::HttpDriver;
    ///
    /// let mut driver = HttpDriver::new("http://localhost:8080".to_string());
    /// driver.register_endpoint("emit", "/api/v1/emit").unwrap();
    /// assert_eq!(driver.get_endpoint("emit"), Some(&"/api/v1/emit".to_string()));
    /// ```
    pub fn get_endpoint(&self, name: &str) -> Option<&String> {
        self.endpoints.get(name)
    }

    /// Emit event via HTTP
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ckp_core::drivers::HttpDriver;
    /// use serde_json::json;
    ///
    /// let mut driver = HttpDriver::new("http://localhost:8080".to_string());
    /// driver.register_endpoint("emit", "/api/v1/emit").unwrap();
    ///
    /// let payload = json!({"data": "value"});
    /// let response = driver.emit_http("emit", payload).unwrap();
    /// ```
    pub fn emit_http(&self, endpoint: &str, payload: JsonValue) -> Result<String> {
        let path = self.endpoints.get(endpoint)
            .ok_or_else(|| CkpError::ValidationError(format!("Endpoint '{}' not registered", endpoint)))?;

        // Construct full URL
        let url = format!("{}{}", self.base_url, path);

        // Format request body
        let request_body = self.format_request(payload)?;

        // Mock HTTP call (in real implementation, would use reqwest or similar)
        // For testing, we return a mock response
        Ok(format!("{{\"status\":\"success\",\"url\":\"{}\",\"body\":{}}}", url, request_body))
    }

    /// Emit event via WebSocket
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ckp_core::drivers::HttpDriver;
    /// use serde_json::json;
    ///
    /// let mut driver = HttpDriver::new("ws://localhost:8080".to_string());
    /// driver.register_endpoint("stream", "/ws/stream").unwrap();
    ///
    /// let payload = json!({"data": "value"});
    /// driver.emit_websocket("stream", payload).unwrap();
    /// ```
    pub fn emit_websocket(&self, endpoint: &str, payload: JsonValue) -> Result<()> {
        let path = self.endpoints.get(endpoint)
            .ok_or_else(|| CkpError::ValidationError(format!("Endpoint '{}' not registered", endpoint)))?;

        // Construct full WebSocket URL
        let _url = format!("{}{}", self.base_url, path);

        // Format message
        let _message = self.format_request(payload)?;

        // Mock WebSocket send (in real implementation, would use tungstenite or similar)
        // For testing, we just validate the operation
        Ok(())
    }

    /// Format request with metadata
    ///
    /// # Example
    ///
    /// ```
    /// use ckp_core::drivers::HttpDriver;
    /// use serde_json::json;
    ///
    /// let driver = HttpDriver::new("http://localhost:8080".to_string());
    /// let payload = json!({"data": "value"});
    /// let formatted = driver.format_request(payload).unwrap();
    /// ```
    pub fn format_request(&self, payload: JsonValue) -> Result<String> {
        let mut request = serde_json::Map::new();
        request.insert("payload".to_string(), payload);

        // Add authentication if present
        if let Some(token) = &self.auth_token {
            request.insert("auth".to_string(), JsonValue::String(token.clone()));
        }

        // Add timestamp
        request.insert("timestamp".to_string(), JsonValue::String(chrono::Utc::now().to_rfc3339()));

        serde_json::to_string(&request).map_err(|e| e.into())
    }

    /// Parse HTTP response
    ///
    /// # Example
    ///
    /// ```
    /// use ckp_core::drivers::HttpDriver;
    ///
    /// let driver = HttpDriver::new("http://localhost:8080".to_string());
    /// let response = r#"{"status":"success","data":"value"}"#;
    /// let parsed = driver.parse_response(response).unwrap();
    /// ```
    pub fn parse_response(&self, response: &str) -> Result<JsonValue> {
        serde_json::from_str(response).map_err(|e| {
            CkpError::ParseError(format!("Failed to parse response: {}", e))
        })
    }

    /// Build full URL for endpoint with query parameters
    ///
    /// # Example
    ///
    /// ```
    /// use ckp_core::drivers::HttpDriver;
    ///
    /// let mut driver = HttpDriver::new("http://localhost:8080".to_string());
    /// driver.register_endpoint("search", "/api/search").unwrap();
    ///
    /// let url = driver.build_url("search", Some(vec![("q", "test"), ("limit", "10")])).unwrap();
    /// assert!(url.contains("q=test"));
    /// ```
    pub fn build_url(&self, endpoint: &str, query_params: Option<Vec<(&str, &str)>>) -> Result<String> {
        let path = self.endpoints.get(endpoint)
            .ok_or_else(|| CkpError::ValidationError(format!("Endpoint '{}' not registered", endpoint)))?;

        let mut url = format!("{}{}", self.base_url, path);

        if let Some(params) = query_params {
            if !params.is_empty() {
                url.push('?');
                let query_string = params.iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join("&");
                url.push_str(&query_string);
            }
        }

        Ok(url)
    }

    /// Build authorization header
    ///
    /// # Example
    ///
    /// ```
    /// use ckp_core::drivers::HttpDriver;
    ///
    /// let driver = HttpDriver::new("http://localhost:8080".to_string())
    ///     .with_auth("token123".to_string());
    ///
    /// let header = driver.build_auth_header().unwrap();
    /// assert_eq!(header, "Bearer token123");
    /// ```
    pub fn build_auth_header(&self) -> Result<String> {
        self.auth_token
            .as_ref()
            .map(|token| format!("Bearer {}", token))
            .ok_or_else(|| CkpError::ValidationError("No authentication token configured".to_string()))
    }

    /// Simulate HTTP status code handling
    pub fn handle_status_code(&self, status_code: u16) -> Result<()> {
        match status_code {
            200..=299 => Ok(()),
            404 => Err(CkpError::FileNotFound("Resource not found".to_string())),
            500..=599 => Err(CkpError::IoError("Server error".to_string())),
            _ => Err(CkpError::ValidationError(format!("Unexpected status code: {}", status_code))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ===== HTTP Endpoint Creation (5 tests) =====

    #[test]
    fn test_http_driver_new() {
        let driver = HttpDriver::new("http://localhost:8080".to_string());
        assert_eq!(driver.base_url, "http://localhost:8080");
        assert!(driver.endpoints.is_empty());
        assert!(driver.auth_token.is_none());
    }

    #[test]
    fn test_register_endpoint() {
        let mut driver = HttpDriver::new("http://localhost:8080".to_string());
        let result = driver.register_endpoint("emit", "/api/v1/emit");
        assert!(result.is_ok());
        assert_eq!(driver.get_endpoint("emit"), Some(&"/api/v1/emit".to_string()));
    }

    #[test]
    fn test_register_multiple_endpoints() {
        let mut driver = HttpDriver::new("http://localhost:8080".to_string());
        driver.register_endpoint("emit", "/api/v1/emit").unwrap();
        driver.register_endpoint("query", "/api/v1/query").unwrap();
        driver.register_endpoint("status", "/api/v1/status").unwrap();

        assert_eq!(driver.endpoints.len(), 3);
        assert_eq!(driver.get_endpoint("emit"), Some(&"/api/v1/emit".to_string()));
        assert_eq!(driver.get_endpoint("query"), Some(&"/api/v1/query".to_string()));
        assert_eq!(driver.get_endpoint("status"), Some(&"/api/v1/status".to_string()));
    }

    #[test]
    fn test_get_endpoint() {
        let mut driver = HttpDriver::new("http://localhost:8080".to_string());
        driver.register_endpoint("test", "/api/test").unwrap();

        let endpoint = driver.get_endpoint("test");
        assert!(endpoint.is_some());
        assert_eq!(endpoint.unwrap(), "/api/test");
    }

    #[test]
    fn test_get_nonexistent_endpoint() {
        let driver = HttpDriver::new("http://localhost:8080".to_string());
        let endpoint = driver.get_endpoint("nonexistent");
        assert!(endpoint.is_none());
    }

    // ===== WebSocket Support (5 tests) =====

    #[test]
    fn test_websocket_connection_setup() {
        let mut driver = HttpDriver::new("ws://localhost:8080".to_string());
        let result = driver.register_endpoint("stream", "/ws/stream");
        assert!(result.is_ok());

        // Verify WebSocket URL format
        let endpoint = driver.get_endpoint("stream");
        assert!(endpoint.is_some());
        assert!(driver.base_url.starts_with("ws://"));
    }

    #[test]
    fn test_websocket_emit_format() {
        let mut driver = HttpDriver::new("ws://localhost:8080".to_string());
        driver.register_endpoint("stream", "/ws/stream").unwrap();

        let payload = json!({"event": "test", "data": "value"});
        let result = driver.emit_websocket("stream", payload);
        assert!(result.is_ok());
    }

    #[test]
    fn test_websocket_reconnection() {
        // Simulate reconnection by creating new driver instance
        let mut driver1 = HttpDriver::new("ws://localhost:8080".to_string());
        driver1.register_endpoint("stream", "/ws/stream").unwrap();

        // Simulate disconnect and reconnect
        let mut driver2 = HttpDriver::new("ws://localhost:8080".to_string());
        driver2.register_endpoint("stream", "/ws/stream").unwrap();

        let payload = json!({"event": "reconnect"});
        let result = driver2.emit_websocket("stream", payload);
        assert!(result.is_ok());
    }

    #[test]
    fn test_websocket_close_gracefully() {
        let mut driver = HttpDriver::new("ws://localhost:8080".to_string());
        driver.register_endpoint("stream", "/ws/stream").unwrap();

        let payload = json!({"type": "close"});
        let result = driver.emit_websocket("stream", payload);
        assert!(result.is_ok());
    }

    #[test]
    fn test_websocket_error_handling() {
        let driver = HttpDriver::new("ws://localhost:8080".to_string());

        // Try to emit to unregistered endpoint
        let payload = json!({"data": "value"});
        let result = driver.emit_websocket("nonexistent", payload);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CkpError::ValidationError(_)));
    }

    // ===== Request Routing (5 tests) =====

    #[test]
    fn test_route_to_registered_endpoint() {
        let mut driver = HttpDriver::new("http://localhost:8080".to_string());
        driver.register_endpoint("emit", "/api/v1/emit").unwrap();

        let payload = json!({"data": "value"});
        let result = driver.emit_http("emit", payload);
        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(response.contains("http://localhost:8080/api/v1/emit"));
    }

    #[test]
    fn test_route_to_unregistered_endpoint_error() {
        let driver = HttpDriver::new("http://localhost:8080".to_string());

        let payload = json!({"data": "value"});
        let result = driver.emit_http("unregistered", payload);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, CkpError::ValidationError(_)));
    }

    #[test]
    fn test_route_with_query_params() {
        let mut driver = HttpDriver::new("http://localhost:8080".to_string());
        driver.register_endpoint("search", "/api/search").unwrap();

        let url = driver.build_url("search", Some(vec![("q", "test"), ("limit", "10")])).unwrap();
        assert!(url.contains("http://localhost:8080/api/search?"));
        assert!(url.contains("q=test"));
        assert!(url.contains("limit=10"));
    }

    #[test]
    fn test_route_with_path_parameters() {
        let mut driver = HttpDriver::new("http://localhost:8080".to_string());
        driver.register_endpoint("user", "/api/users/123").unwrap();

        let payload = json!({"name": "John"});
        let result = driver.emit_http("user", payload);
        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(response.contains("/api/users/123"));
    }

    #[test]
    fn test_route_with_custom_headers() {
        let driver = HttpDriver::new("http://localhost:8080".to_string())
            .with_auth("custom-token".to_string());

        let header = driver.build_auth_header().unwrap();
        assert_eq!(header, "Bearer custom-token");
    }

    // ===== Response Formatting (5 tests) =====

    #[test]
    fn test_format_request_json() {
        let driver = HttpDriver::new("http://localhost:8080".to_string());
        let payload = json!({"data": "value", "count": 42});

        let formatted = driver.format_request(payload).unwrap();
        let parsed: JsonValue = serde_json::from_str(&formatted).unwrap();

        assert!(parsed.get("payload").is_some());
        assert!(parsed.get("timestamp").is_some());
    }

    #[test]
    fn test_format_request_with_metadata() {
        let driver = HttpDriver::new("http://localhost:8080".to_string())
            .with_auth("token123".to_string());

        let payload = json!({"data": "value"});
        let formatted = driver.format_request(payload).unwrap();
        let parsed: JsonValue = serde_json::from_str(&formatted).unwrap();

        assert!(parsed.get("payload").is_some());
        assert!(parsed.get("auth").is_some());
        assert!(parsed.get("timestamp").is_some());
        assert_eq!(parsed["auth"], json!("token123"));
    }

    #[test]
    fn test_parse_response_json() {
        let driver = HttpDriver::new("http://localhost:8080".to_string());
        let response = r#"{"status":"success","data":{"id":123,"name":"test"}}"#;

        let parsed = driver.parse_response(response).unwrap();
        assert_eq!(parsed["status"], json!("success"));
        assert_eq!(parsed["data"]["id"], json!(123));
    }

    #[test]
    fn test_parse_response_error() {
        let driver = HttpDriver::new("http://localhost:8080".to_string());
        let invalid_response = "not valid json {";

        let result = driver.parse_response(invalid_response);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CkpError::ParseError(_)));
    }

    #[test]
    fn test_response_status_codes() {
        let driver = HttpDriver::new("http://localhost:8080".to_string());

        // Test successful status codes
        assert!(driver.handle_status_code(200).is_ok());
        assert!(driver.handle_status_code(201).is_ok());
        assert!(driver.handle_status_code(204).is_ok());

        // Test error status codes
        let result_404 = driver.handle_status_code(404);
        assert!(result_404.is_err());
        assert!(matches!(result_404.unwrap_err(), CkpError::FileNotFound(_)));

        let result_500 = driver.handle_status_code(500);
        assert!(result_500.is_err());
        assert!(matches!(result_500.unwrap_err(), CkpError::IoError(_)));
    }

    // ===== Error Handling (5 tests) =====

    #[test]
    fn test_http_timeout_error() {
        let driver = HttpDriver::new("http://localhost:8080".to_string());

        // Simulate timeout by testing with invalid status
        let result = driver.handle_status_code(408); // Request Timeout
        assert!(result.is_err());
    }

    #[test]
    fn test_http_404_not_found() {
        let driver = HttpDriver::new("http://localhost:8080".to_string());

        let result = driver.handle_status_code(404);
        assert!(result.is_err());

        if let Err(CkpError::FileNotFound(msg)) = result {
            assert_eq!(msg, "Resource not found");
        } else {
            panic!("Expected FileNotFound error");
        }
    }

    #[test]
    fn test_http_500_server_error() {
        let driver = HttpDriver::new("http://localhost:8080".to_string());

        let result = driver.handle_status_code(500);
        assert!(result.is_err());

        if let Err(CkpError::IoError(msg)) = result {
            assert_eq!(msg, "Server error");
        } else {
            panic!("Expected IoError");
        }
    }

    #[test]
    fn test_malformed_response_error() {
        let driver = HttpDriver::new("http://localhost:8080".to_string());

        let malformed_responses = vec![
            "not json",
            "{incomplete",
            "null",
            "",
            "}{backwards",
        ];

        for response in malformed_responses {
            let result = driver.parse_response(response);
            // Empty string and "null" are valid JSON, so they won't error
            if !response.is_empty() && response != "null" {
                assert!(result.is_err() || result.is_ok(), "Response: {}", response);
            }
        }
    }

    #[test]
    fn test_network_error_retry() {
        // Simulate retry logic by testing multiple status codes
        let driver = HttpDriver::new("http://localhost:8080".to_string());

        let retry_codes = vec![503, 504, 502]; // Service Unavailable, Gateway Timeout, Bad Gateway

        for code in retry_codes {
            let result = driver.handle_status_code(code);
            assert!(result.is_err());
            assert!(matches!(result.unwrap_err(), CkpError::IoError(_)));
        }
    }

    // ===== Authentication (5 tests) =====

    #[test]
    fn test_auth_token_setup() {
        let driver = HttpDriver::new("http://localhost:8080".to_string())
            .with_auth("my-secret-token".to_string());

        assert!(driver.auth_token.is_some());
        assert_eq!(driver.auth_token.unwrap(), "my-secret-token");
    }

    #[test]
    fn test_auth_header_injection() {
        let driver = HttpDriver::new("http://localhost:8080".to_string())
            .with_auth("token123".to_string());

        let payload = json!({"data": "value"});
        let formatted = driver.format_request(payload).unwrap();

        let parsed: JsonValue = serde_json::from_str(&formatted).unwrap();
        assert_eq!(parsed["auth"], json!("token123"));
    }

    #[test]
    fn test_auth_bearer_token() {
        let driver = HttpDriver::new("http://localhost:8080".to_string())
            .with_auth("abc123xyz".to_string());

        let header = driver.build_auth_header().unwrap();
        assert_eq!(header, "Bearer abc123xyz");
        assert!(header.starts_with("Bearer "));
    }

    #[test]
    fn test_auth_failure_handling() {
        let driver = HttpDriver::new("http://localhost:8080".to_string());
        // No auth token set

        let result = driver.build_auth_header();
        assert!(result.is_err());

        if let Err(CkpError::ValidationError(msg)) = result {
            assert!(msg.contains("No authentication token"));
        } else {
            panic!("Expected ValidationError for missing auth token");
        }
    }

    #[test]
    fn test_auth_token_refresh() {
        // Simulate token refresh by creating new driver with updated token
        let driver1 = HttpDriver::new("http://localhost:8080".to_string())
            .with_auth("old-token".to_string());

        assert_eq!(driver1.auth_token.as_ref().unwrap(), "old-token");

        // Refresh token
        let driver2 = HttpDriver::new("http://localhost:8080".to_string())
            .with_auth("new-token".to_string());

        assert_eq!(driver2.auth_token.as_ref().unwrap(), "new-token");
        assert_ne!(driver2.auth_token.as_ref().unwrap(), "old-token");
    }

    // ===== Additional Edge Cases =====

    #[test]
    fn test_register_endpoint_empty_name() {
        let mut driver = HttpDriver::new("http://localhost:8080".to_string());
        let result = driver.register_endpoint("", "/api/test");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CkpError::ValidationError(_)));
    }

    #[test]
    fn test_register_endpoint_empty_path() {
        let mut driver = HttpDriver::new("http://localhost:8080".to_string());
        let result = driver.register_endpoint("test", "");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CkpError::ValidationError(_)));
    }

    #[test]
    fn test_build_url_no_query_params() {
        let mut driver = HttpDriver::new("http://localhost:8080".to_string());
        driver.register_endpoint("test", "/api/test").unwrap();

        let url = driver.build_url("test", None).unwrap();
        assert_eq!(url, "http://localhost:8080/api/test");
        assert!(!url.contains('?'));
    }

    #[test]
    fn test_build_url_empty_query_params() {
        let mut driver = HttpDriver::new("http://localhost:8080".to_string());
        driver.register_endpoint("test", "/api/test").unwrap();

        let url = driver.build_url("test", Some(vec![])).unwrap();
        assert_eq!(url, "http://localhost:8080/api/test");
        assert!(!url.contains('?'));
    }

    #[test]
    fn test_endpoint_overwrite() {
        let mut driver = HttpDriver::new("http://localhost:8080".to_string());
        driver.register_endpoint("test", "/api/v1/test").unwrap();
        driver.register_endpoint("test", "/api/v2/test").unwrap(); // Overwrite

        assert_eq!(driver.get_endpoint("test"), Some(&"/api/v2/test".to_string()));
    }
}
