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

//! FFI bindings to the lighter-go signing library.
//!
//! This module provides safe Rust wrappers around the C FFI functions
//! exported by the lighter-go shared library.
//!
//! # Library Loading
//!
//! The shared library (`liblighter-signer.so`) must be available at runtime.
//! Set `LD_LIBRARY_PATH` to include the library directory:
//!
//! ```bash
//! export LD_LIBRARY_PATH=/path/to/libs/linux/amd64:$LD_LIBRARY_PATH
//! ```

use std::ffi::{c_char, c_int, c_longlong, CStr, CString};

use tracing::{debug, warn};

use crate::error::LighterError;

/// Result struct from Go FFI functions.
#[repr(C)]
pub struct StrOrErr {
    /// Result string pointer (null if error).
    pub str_: *mut c_char,
    /// Error string pointer (null if success).
    pub err: *mut c_char,
}

// FFI function declarations
#[link(name = "lighter-signer")]
unsafe extern "C" {
    fn CreateClient(
        url: *mut c_char,
        private_key: *mut c_char,
        chain_id: c_int,
        api_key_index: c_int,
        account_index: c_longlong,
    ) -> *mut c_char;

    fn SignCreateOrder(
        market_index: c_int,
        client_order_index: c_longlong,
        base_amount: c_longlong,
        price: c_int,
        is_ask: c_int,
        order_type: c_int,
        time_in_force: c_int,
        reduce_only: c_int,
        trigger_price: c_int,
        order_expiry: c_longlong,
        nonce: c_longlong,
    ) -> StrOrErr;

    fn SignCancelOrder(
        market_index: c_int,
        order_index: c_longlong,
        nonce: c_longlong,
    ) -> StrOrErr;

    fn SignCancelAllOrders(
        time_in_force: c_int,
        time: c_longlong,
        nonce: c_longlong,
    ) -> StrOrErr;

    fn CreateAuthToken(deadline: c_longlong) -> StrOrErr;
}

/// Signed transaction result.
#[derive(Clone, Debug)]
pub struct SignedTransaction {
    /// Transaction type (e.g., "CreateOrder", "CancelOrder").
    pub tx_type: String,
    /// Transaction info as JSON string.
    pub tx_info: String,
    /// Transaction hash/signature.
    pub tx_hash: String,
}

/// Initialize the FFI client.
///
/// This must be called before any signing operations.
///
/// # Arguments
/// * `url` - REST API URL (e.g., "https://testnet.zklighter.elliot.ai")
/// * `private_key` - 40-byte API key (80 hex chars, with or without 0x prefix)
/// * `chain_id` - Chain ID (300 for testnet, 304 for mainnet)
/// * `api_key_index` - API key index (2-254)
/// * `account_index` - Account index on Lighter
///
/// # Errors
/// Returns error if client initialization fails.
pub fn create_client(
    url: &str,
    private_key: &str,
    chain_id: u32,
    api_key_index: u8,
    account_index: i64,
) -> Result<(), LighterError> {
    // Remove 0x prefix if present
    let key = private_key.strip_prefix("0x").unwrap_or(private_key);

    // Create C strings
    let c_url = CString::new(url)
        .map_err(|_| LighterError::Config("Invalid URL".to_string()))?;
    let c_key = CString::new(key)
        .map_err(|_| LighterError::Config("Invalid private key".to_string()))?;

    // Call Go FFI to create client
    let result = unsafe {
        CreateClient(
            c_url.into_raw(),
            c_key.into_raw(),
            chain_id as c_int,
            api_key_index as c_int,
            account_index as c_longlong,
        )
    };

    // Check for errors
    if !result.is_null() {
        let err_str = unsafe { CStr::from_ptr(result).to_string_lossy().to_string() };
        unsafe { libc::free(result as *mut libc::c_void) };
        return Err(LighterError::Signing(err_str));
    }

    debug!("FFI client created successfully");
    Ok(())
}

/// Sign a create order transaction.
///
/// # Arguments
/// * `market_index` - Market ID
/// * `client_order_index` - Client-side order ID
/// * `base_amount` - Order size in base units
/// * `price` - Order price (integer representation)
/// * `is_ask` - true for sell, false for buy
/// * `order_type` - Order type (0=limit, 1=market)
/// * `time_in_force` - TIF (0=GoodTillTime, 1=IOC, 2=FOK, 3=PostOnly)
/// * `reduce_only` - Whether reduce-only
/// * `trigger_price` - Trigger price for stop orders (0 for none)
/// * `order_expiry` - Order expiry timestamp (-1 for 28 days default)
/// * `nonce` - Transaction nonce
///
/// # Errors
/// Returns error if signing fails.
pub fn sign_create_order(
    market_index: u16,
    client_order_index: i64,
    base_amount: i64,
    price: u32,
    is_ask: bool,
    order_type: u8,
    time_in_force: u8,
    reduce_only: bool,
    trigger_price: u32,
    order_expiry: i64,
    nonce: u64,
) -> Result<SignedTransaction, LighterError> {
    let result = unsafe {
        SignCreateOrder(
            market_index as c_int,
            client_order_index as c_longlong,
            base_amount as c_longlong,
            price as c_int,
            is_ask as c_int,
            order_type as c_int,
            time_in_force as c_int,
            reduce_only as c_int,
            trigger_price as c_int,
            order_expiry as c_longlong,
            nonce as c_longlong,
        )
    };

    parse_result(result, "CreateOrder")
}

/// Sign a cancel order transaction.
///
/// # Arguments
/// * `market_index` - Market ID
/// * `order_index` - Exchange order ID to cancel
/// * `nonce` - Transaction nonce
///
/// # Errors
/// Returns error if signing fails.
pub fn sign_cancel_order(
    market_index: u16,
    order_index: i64,
    nonce: u64,
) -> Result<SignedTransaction, LighterError> {
    let result = unsafe {
        SignCancelOrder(
            market_index as c_int,
            order_index as c_longlong,
            nonce as c_longlong,
        )
    };

    parse_result(result, "CancelOrder")
}

/// Sign a cancel all orders transaction.
///
/// # Arguments
/// * `time_in_force` - Filter by TIF (0 for all)
/// * `time` - Current timestamp in milliseconds
/// * `nonce` - Transaction nonce
///
/// # Errors
/// Returns error if signing fails.
pub fn sign_cancel_all_orders(
    time_in_force: u8,
    time: i64,
    nonce: u64,
) -> Result<SignedTransaction, LighterError> {
    let result = unsafe {
        SignCancelAllOrders(
            time_in_force as c_int,
            time as c_longlong,
            nonce as c_longlong,
        )
    };

    parse_result(result, "CancelAllOrders")
}

/// Create an authentication token for WebSocket connections.
///
/// # Arguments
/// * `deadline` - Token expiry timestamp in seconds (0 for default)
///
/// # Errors
/// Returns error if token creation fails.
pub fn create_auth_token(deadline: i64) -> Result<String, LighterError> {
    let result = unsafe { CreateAuthToken(deadline as c_longlong) };

    // Check for errors
    if !result.err.is_null() {
        let err_str = unsafe { CStr::from_ptr(result.err).to_string_lossy().to_string() };
        unsafe {
            libc::free(result.err as *mut libc::c_void);
            if !result.str_.is_null() {
                libc::free(result.str_ as *mut libc::c_void);
            }
        }
        return Err(LighterError::Signing(err_str));
    }

    if result.str_.is_null() {
        return Err(LighterError::Signing("Null auth token".to_string()));
    }

    let token = unsafe { CStr::from_ptr(result.str_).to_string_lossy().to_string() };
    unsafe { libc::free(result.str_ as *mut libc::c_void) };

    Ok(token)
}

/// Parse FFI result into SignedTransaction.
fn parse_result(result: StrOrErr, tx_type: &str) -> Result<SignedTransaction, LighterError> {
    // Check for errors
    if !result.err.is_null() {
        let err_str = unsafe { CStr::from_ptr(result.err).to_string_lossy().to_string() };
        unsafe {
            libc::free(result.err as *mut libc::c_void);
            if !result.str_.is_null() {
                libc::free(result.str_ as *mut libc::c_void);
            }
        }
        return Err(LighterError::Signing(err_str));
    }

    if result.str_.is_null() {
        return Err(LighterError::Signing("Null result".to_string()));
    }

    let tx_info = unsafe { CStr::from_ptr(result.str_).to_string_lossy().to_string() };
    unsafe { libc::free(result.str_ as *mut libc::c_void) };

    // Parse JSON to extract signature if present
    let json: serde_json::Value = serde_json::from_str(&tx_info)
        .map_err(|e| LighterError::Signing(format!("Invalid JSON: {e}")))?;

    let tx_hash = json.get("L2Sig")
        .or_else(|| json.get("Signature"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    Ok(SignedTransaction {
        tx_type: tx_type.to_string(),
        tx_info,
        tx_hash,
    })
}

#[cfg(test)]
mod tests {
    // FFI tests require the shared library at runtime
    // These are integration tests that should be run with LD_LIBRARY_PATH set
}
