// -------------------------------------------------------------------------------------------------
//  Copyright (C) 2015-2025 Nautech Systems Pty Ltd. All rights reserved.
//  https://nautechsystems.io
//
//  Licensed under the GNU Lesser General Public License Version 3.0 (the "License");
//  You may not use this file except in compliance with the License.
//  You may obtain a copy of the License at https://www.gnu.org/licenses/lgpl-3.0.en.html
// -------------------------------------------------------------------------------------------------

use std::{sync::Arc, time::Duration};

use nautilus_lighter::{
    error::LighterError,
    http::retry::{LighterRetryClass, LighterRetryPolicy},
};
use tokio::sync::Mutex;

#[tokio::test]
async fn retry_budget_limits_transient_server_errors() {
    let policy = LighterRetryPolicy::for_latency_tier(200).with_zero_delay_for_tests();
    let attempts = Arc::new(Mutex::new(0_u32));

    let result = policy
        .execute("server-error", {
            let attempts = Arc::clone(&attempts);
            move || {
                let attempts = Arc::clone(&attempts);
                async move {
                    let mut guard = attempts.lock().await;
                    *guard += 1;
                    Err::<(), _>(LighterError::Http("HTTP 503: overloaded".to_string()))
                }
            }
        })
        .await;

    assert!(matches!(result, Err(LighterError::Http(message)) if message.contains("503")));
    assert_eq!(
        *attempts.lock().await,
        4,
        "initial request plus three retries"
    );
}

#[tokio::test]
async fn request_timeout_is_retryable_until_budget_is_exhausted() {
    let policy = LighterRetryPolicy::for_latency_tier(200)
        .with_operation_timeout_for_tests(Duration::from_millis(5))
        .with_zero_delay_for_tests()
        .with_max_retries_for_tests(1);
    let attempts = Arc::new(Mutex::new(0_u32));

    let result = policy
        .execute("timeout", {
            let attempts = Arc::clone(&attempts);
            move || {
                let attempts = Arc::clone(&attempts);
                async move {
                    let mut guard = attempts.lock().await;
                    *guard += 1;
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    Ok::<_, LighterError>(())
                }
            }
        })
        .await;

    assert!(matches!(result, Err(LighterError::Timeout(message)) if message.contains("Timed out")));
    assert_eq!(*attempts.lock().await, 2, "timeout consumes retry budget");
}

#[tokio::test]
async fn rate_limit_errors_use_backoff_before_retrying() {
    let policy = LighterRetryPolicy::for_latency_tier(200)
        .with_backoff_for_tests(Duration::from_millis(20), Duration::from_millis(20))
        .with_jitter_for_tests(Duration::ZERO)
        .with_max_retries_for_tests(1);
    let attempts = Arc::new(Mutex::new(0_u32));
    let started = std::time::Instant::now();

    let result = policy
        .execute("rate-limit", {
            let attempts = Arc::clone(&attempts);
            move || {
                let attempts = Arc::clone(&attempts);
                async move {
                    let mut guard = attempts.lock().await;
                    *guard += 1;
                    if *guard == 1 {
                        Err(LighterError::RateLimit("HTTP 429: retry later".to_string()))
                    } else {
                        Ok("recovered")
                    }
                }
            }
        })
        .await;

    assert_eq!(result.unwrap(), "recovered");
    assert_eq!(*attempts.lock().await, 2);
    assert!(started.elapsed() >= Duration::from_millis(20));
}

#[test]
fn retry_classification_marks_non_retryable_paths_fail_fast() {
    let policy = LighterRetryPolicy::for_latency_tier(300);

    assert_eq!(
        policy.classify_error(&LighterError::RateLimit("429".to_string())),
        LighterRetryClass::RetryableRateLimit
    );
    assert_eq!(
        policy.classify_error(&LighterError::Http("HTTP 503".to_string())),
        LighterRetryClass::RetryableTransient
    );
    assert_eq!(
        policy.classify_error(&LighterError::Timeout("timeout".to_string())),
        LighterRetryClass::RetryableTimeout
    );

    assert_eq!(
        policy.classify_error(&LighterError::Auth("missing token".to_string())),
        LighterRetryClass::FailFast
    );
    assert_eq!(
        policy.classify_error(&LighterError::Config("invalid signing gate".to_string())),
        LighterRetryClass::FailFast
    );
    assert_eq!(
        policy.classify_error(&LighterError::Parse("bad request".to_string())),
        LighterRetryClass::FailFast
    );
    assert_eq!(
        policy.classify_error(&LighterError::OrderRejected {
            code: 4001,
            message: "insufficient balance".to_string()
        }),
        LighterRetryClass::FailFast
    );
}
