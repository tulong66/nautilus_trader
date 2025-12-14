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

//! Nonce management for Lighter transactions.
//!
//! Lighter requires each transaction to have a unique, incrementing nonce.
//! This module provides thread-safe nonce management with rollback support.
//!
//! # Important
//!
//! - Nonces are per-API-key, not global
//! - Nonces must be strictly incrementing
//! - On signing failure, the nonce should be rolled back
//! - On startup or error recovery, sync nonce from API

use std::sync::atomic::{AtomicU64, Ordering};
use tracing::debug;

/// Thread-safe nonce manager with rollback support.
///
/// Uses `AtomicU64` for lock-free concurrent access.
#[derive(Debug)]
pub struct NonceManager {
    nonce: AtomicU64,
    account_index: u32,
    api_key_index: u8,
}

impl NonceManager {
    /// Create a new nonce manager.
    ///
    /// # Arguments
    /// * `initial_nonce` - Initial nonce value (typically from API)
    /// * `account_index` - Account index on Lighter
    /// * `api_key_index` - API key index (2-254)
    #[must_use]
    pub fn new(initial_nonce: u64, account_index: u32, api_key_index: u8) -> Self {
        Self {
            nonce: AtomicU64::new(initial_nonce),
            account_index,
            api_key_index,
        }
    }

    /// Get the next nonce and increment the counter.
    ///
    /// This is atomic and thread-safe.
    pub fn next(&self) -> u64 {
        self.nonce.fetch_add(1, Ordering::SeqCst)
    }

    /// Get the current nonce value without incrementing.
    #[must_use]
    pub fn current(&self) -> u64 {
        self.nonce.load(Ordering::SeqCst)
    }

    /// Rollback the nonce after a failed transaction.
    ///
    /// Call this when signing succeeds but the transaction fails,
    /// to reuse the nonce for the retry.
    pub fn rollback(&self) {
        self.nonce.fetch_sub(1, Ordering::SeqCst);
        debug!("Nonce rolled back to {}", self.current());
    }

    /// Reset the nonce to a specific value.
    ///
    /// Use this for error recovery when syncing with the server.
    pub fn reset(&self, value: u64) {
        let old = self.nonce.swap(value, Ordering::SeqCst);
        debug!("Nonce reset from {} to {}", old, value);
    }

    /// Get the account index.
    #[must_use]
    pub fn account_index(&self) -> u32 {
        self.account_index
    }

    /// Get the API key index.
    #[must_use]
    pub fn api_key_index(&self) -> u8 {
        self.api_key_index
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_nonce_manager_basic() {
        let nm = NonceManager::new(100, 1, 2);
        assert_eq!(nm.current(), 100);
        assert_eq!(nm.next(), 100);
        assert_eq!(nm.current(), 101);
        assert_eq!(nm.next(), 101);
        assert_eq!(nm.current(), 102);
    }

    #[test]
    fn test_nonce_manager_rollback() {
        let nm = NonceManager::new(100, 1, 2);
        assert_eq!(nm.next(), 100);
        assert_eq!(nm.current(), 101);
        nm.rollback();
        assert_eq!(nm.current(), 100);
    }

    #[test]
    fn test_nonce_manager_reset() {
        let nm = NonceManager::new(100, 1, 2);
        nm.next();
        nm.next();
        assert_eq!(nm.current(), 102);
        nm.reset(500);
        assert_eq!(nm.current(), 500);
    }

    #[test]
    fn test_nonce_manager_concurrent() {
        let nm = Arc::new(NonceManager::new(0, 1, 2));
        let mut handles = Vec::new();

        // Spawn 10 threads, each calling next() 100 times
        for _ in 0..10 {
            let nm_clone = nm.clone();
            handles.push(thread::spawn(move || {
                let mut nonces = Vec::new();
                for _ in 0..100 {
                    nonces.push(nm_clone.next());
                }
                nonces
            }));
        }

        // Collect all nonces
        let mut all_nonces: Vec<u64> = handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect();

        // Verify all nonces are unique
        all_nonces.sort();
        let unique_count = all_nonces.windows(2).filter(|w| w[0] != w[1]).count() + 1;
        assert_eq!(unique_count, 1000, "All nonces should be unique");

        // Verify final nonce value
        assert_eq!(nm.current(), 1000);
    }

    #[test]
    fn test_nonce_manager_account_info() {
        let nm = NonceManager::new(100, 878, 2);
        assert_eq!(nm.account_index(), 878);
        assert_eq!(nm.api_key_index(), 2);
    }
}
