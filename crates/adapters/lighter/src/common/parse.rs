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

//! Price and quantity parsing utilities for Lighter.
//!
//! Lighter uses dynamic price precision - each market has different `price_decimals`.
//! This module provides conversion functions between float prices and integer representations.
//!
//! # Price Conversion Formula
//!
//! ```text
//! price_int = price_usd × (10 ** price_decimals)
//! ```
//!
//! # Examples
//!
//! | Asset | price_decimals | USD Price | Integer |
//! |-------|----------------|-----------|---------|
//! | ETH   | 2              | $4127.39  | 412739  |
//! | BTC   | 1              | $114357.8 | 1143578 |
//! | SOL   | 3              | $199.058  | 199058  |
//! | DOGE  | 6              | $0.202095 | 202095  |
//!
//! # Quantity Multiplier
//!
//! ```text
//! quantity_multiplier = 10^(6 - price_decimals)
//! ```

/// Convert a USD price to Lighter's integer representation.
///
/// # Arguments
/// * `price` - Price in USD (e.g., 4127.39)
/// * `price_decimals` - Number of decimals for this market
///
/// # Returns
/// Integer price representation (e.g., 412739 for ETH with decimals=2)
#[must_use]
pub fn price_to_int(price: f64, price_decimals: u8) -> u64 {
    (price * 10f64.powi(price_decimals as i32)).round() as u64
}

/// Convert Lighter's integer price to USD.
///
/// # Arguments
/// * `price_int` - Integer price from Lighter
/// * `price_decimals` - Number of decimals for this market
///
/// # Returns
/// Price in USD (e.g., 4127.39)
#[must_use]
pub fn int_to_price(price_int: u64, price_decimals: u8) -> f64 {
    price_int as f64 / 10f64.powi(price_decimals as i32)
}

/// Get the quantity multiplier for a market.
///
/// Formula: 10^(6 - price_decimals)
///
/// # Arguments
/// * `price_decimals` - Number of decimals for this market
///
/// # Returns
/// Quantity multiplier
#[must_use]
pub fn quantity_multiplier(price_decimals: u8) -> u64 {
    10u64.pow(6 - price_decimals as u32)
}

/// Convert a base quantity to Lighter's integer representation.
///
/// # Arguments
/// * `quantity` - Quantity in base units (e.g., 1.5 ETH)
/// * `price_decimals` - Number of decimals for this market
///
/// # Returns
/// Integer quantity for Lighter API
#[must_use]
pub fn quantity_to_int(quantity: f64, price_decimals: u8) -> i64 {
    let multiplier = quantity_multiplier(price_decimals) as f64;
    // Lighter uses 8 decimals for base amounts
    (quantity * multiplier * 100_000_000.0).round() as i64
}

/// Convert Lighter's integer quantity to base units.
///
/// # Arguments
/// * `quantity_int` - Integer quantity from Lighter
/// * `price_decimals` - Number of decimals for this market
///
/// # Returns
/// Quantity in base units (e.g., 1.5 ETH)
#[must_use]
pub fn int_to_quantity(quantity_int: i64, price_decimals: u8) -> f64 {
    let multiplier = quantity_multiplier(price_decimals) as f64;
    quantity_int as f64 / (multiplier * 100_000_000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rstest::rstest]
    #[case(4127.39, 2, 412739)]      // ETH
    #[case(114357.8, 1, 1143578)]    // BTC
    #[case(199.058, 3, 199058)]      // SOL
    #[case(0.202095, 6, 202095)]     // DOGE
    #[case(1.0, 2, 100)]             // Simple case
    #[case(0.01, 2, 1)]              // Minimum
    fn test_price_to_int(#[case] price: f64, #[case] decimals: u8, #[case] expected: u64) {
        assert_eq!(price_to_int(price, decimals), expected);
    }

    #[rstest::rstest]
    #[case(412739, 2, 4127.39)]      // ETH
    #[case(1143578, 1, 114357.8)]    // BTC
    #[case(199058, 3, 199.058)]      // SOL
    #[case(202095, 6, 0.202095)]     // DOGE
    fn test_int_to_price(#[case] price_int: u64, #[case] decimals: u8, #[case] expected: f64) {
        let result = int_to_price(price_int, decimals);
        assert!((result - expected).abs() < 0.0001, "Expected {expected}, got {result}");
    }

    #[rstest::rstest]
    #[case(2, 10000)]   // ETH: 10^(6-2) = 10^4
    #[case(1, 100000)]  // BTC: 10^(6-1) = 10^5
    #[case(3, 1000)]    // SOL: 10^(6-3) = 10^3
    #[case(6, 1)]       // DOGE: 10^(6-6) = 10^0
    fn test_quantity_multiplier(#[case] decimals: u8, #[case] expected: u64) {
        assert_eq!(quantity_multiplier(decimals), expected);
    }

    #[test]
    fn test_roundtrip_price() {
        let original = 4127.39;
        let decimals = 2;
        let int_repr = price_to_int(original, decimals);
        let back = int_to_price(int_repr, decimals);
        assert!((original - back).abs() < 0.01);
    }
}
