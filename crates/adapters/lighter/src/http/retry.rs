// -------------------------------------------------------------------------------------------------
//  Copyright (C) 2015-2025 Nautech Systems Pty Ltd. All rights reserved.
//  https://nautechsystems.io
//
//  Licensed under the GNU Lesser General Public License Version 3.0 (the "License");
//  You may not use this file except in compliance with the License.
//  You may obtain a copy of the License at https://www.gnu.org/licenses/lgpl-3.0.en.html
// -------------------------------------------------------------------------------------------------

//! Retry, timeout, and rate-limit policy for Lighter REST requests.
//!
//! The policy is intentionally explicit so adapter tests can verify which paths
//! retry and which paths fail fast before any live execution path is enabled.

use std::future::Future;

use nautilus_network::retry::{RetryConfig, RetryManager};

use crate::error::LighterError;

/// Classification used by the Lighter HTTP retry policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LighterRetryClass {
    /// HTTP 429 or adapter rate-limit errors; retry with normal backoff.
    RetryableRateLimit,
    /// Request or operation timeout; retry while budget remains.
    RetryableTimeout,
    /// Network or transient HTTP 5xx/408/504 style failures.
    RetryableTransient,
    /// Authentication, configuration, validation, parse, signing, and business rejects.
    FailFast,
}

/// Adapter-level retry policy for Lighter REST requests.
#[derive(Clone, Debug)]
pub struct LighterRetryPolicy {
    config: RetryConfig,
}

impl Default for LighterRetryPolicy {
    fn default() -> Self {
        Self::for_latency_tier(300)
    }
}

impl LighterRetryPolicy {
    /// Builds a policy for the documented Lighter Standard latency tier.
    ///
    /// `latency_ms` is usually 200ms or 300ms. The adapter uses that value as
    /// the initial retry delay, caps backoff at 5x that tier, and keeps the
    /// network operation timeout separate from the HTTP client's socket timeout.
    #[must_use]
    pub fn for_latency_tier(latency_ms: u64) -> Self {
        let base_delay_ms = latency_ms.max(1);
        Self {
            config: RetryConfig {
                max_retries: 3,
                initial_delay_ms: base_delay_ms,
                max_delay_ms: base_delay_ms.saturating_mul(5),
                backoff_factor: 2.0,
                jitter_ms: 25,
                operation_timeout_ms: Some(30_000),
                immediate_first: false,
                max_elapsed_ms: Some(30_000_u64.saturating_add(base_delay_ms.saturating_mul(8))),
            },
        }
    }

    /// Builds a policy from an existing network retry config.
    #[must_use]
    pub const fn from_retry_config(config: RetryConfig) -> Self {
        Self { config }
    }

    /// Returns the underlying retry config used by Nautilus network retry manager.
    #[must_use]
    pub const fn config(&self) -> &RetryConfig {
        &self.config
    }

    /// Classifies an adapter error for retry/fail-fast behavior.
    #[must_use]
    pub fn classify_error(&self, error: &LighterError) -> LighterRetryClass {
        match error {
            LighterError::RateLimit(_) => LighterRetryClass::RetryableRateLimit,
            LighterError::Timeout(_) => LighterRetryClass::RetryableTimeout,
            LighterError::Http(message) if is_retryable_http_message(message) => {
                LighterRetryClass::RetryableTransient
            }
            LighterError::Http(message) if is_transport_http_message(message) => {
                LighterRetryClass::RetryableTransient
            }
            _ => LighterRetryClass::FailFast,
        }
    }

    /// Returns true when an error may be retried under this policy.
    #[must_use]
    pub fn should_retry(&self, error: &LighterError) -> bool {
        !matches!(self.classify_error(error), LighterRetryClass::FailFast)
    }

    /// Executes an async operation under this retry policy.
    ///
    /// # Errors
    ///
    /// Returns the final operation error, a timeout error, or an internal retry
    /// manager error if the retry config is invalid or cancellation is requested.
    pub async fn execute<F, Fut, T>(
        &self,
        operation_name: &str,
        operation: F,
    ) -> Result<T, LighterError>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, LighterError>>,
    {
        let manager = RetryManager::new(self.config.clone());
        manager
            .execute_with_retry(
                operation_name,
                operation,
                |error| self.should_retry(error),
                Self::create_retry_manager_error,
            )
            .await
    }

    pub(crate) fn retry_manager(&self) -> RetryManager<LighterError> {
        RetryManager::new(self.config.clone())
    }

    pub(crate) fn create_retry_manager_error(msg: String) -> LighterError {
        if msg == "canceled" {
            LighterError::Internal("Request canceled".to_string())
        } else if msg.starts_with("Timed out after") {
            LighterError::Timeout(msg)
        } else {
            LighterError::Internal(msg)
        }
    }

    #[must_use]
    pub fn with_zero_delay_for_tests(mut self) -> Self {
        self.config.initial_delay_ms = 1;
        self.config.max_delay_ms = 1;
        self.config.jitter_ms = 0;
        self
    }

    #[must_use]
    pub fn with_operation_timeout_for_tests(mut self, timeout: std::time::Duration) -> Self {
        self.config.operation_timeout_ms = Some(timeout.as_millis() as u64);
        self
    }

    #[must_use]
    pub fn with_backoff_for_tests(
        mut self,
        initial: std::time::Duration,
        max: std::time::Duration,
    ) -> Self {
        self.config.initial_delay_ms = initial.as_millis() as u64;
        self.config.max_delay_ms = max.as_millis() as u64;
        self
    }

    #[must_use]
    pub fn with_jitter_for_tests(mut self, jitter: std::time::Duration) -> Self {
        self.config.jitter_ms = jitter.as_millis() as u64;
        self
    }

    #[must_use]
    pub fn with_max_retries_for_tests(mut self, max_retries: u32) -> Self {
        self.config.max_retries = max_retries;
        self
    }
}

fn is_retryable_http_message(message: &str) -> bool {
    [
        "HTTP 408", "HTTP 429", "HTTP 500", "HTTP 502", "HTTP 503", "HTTP 504",
    ]
    .iter()
    .any(|needle| message.contains(needle))
}

fn is_transport_http_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("request failed")
        || lower.contains("connection")
        || lower.contains("connect")
        || lower.contains("dns")
        || lower.contains("reset")
        || lower.contains("closed")
}
