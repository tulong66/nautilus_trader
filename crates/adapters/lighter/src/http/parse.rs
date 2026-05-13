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

//! Parsing functions for Lighter DEX HTTP API responses.

use super::types::{
    AccountResponse, FundingHistoryEntry, FundingHistoryResponse, LighterList, LighterResponse,
    LiquidationRiskResponse, LiquidationThresholdResponse, MarginRatioResponse, MarginRiskResponse,
    MarketsResponse, OrderResponse, OrderbookResponse, OrdersResponse, TickerResponse,
    TradesResponse,
};
use crate::error::LighterError;

/// Parses a Lighter markets response from raw JSON bytes.
///
/// # Errors
///
/// Returns an error if:
/// - Deserialization fails
/// - The response indicates an API error
pub fn parse_markets_response(data: &[u8]) -> Result<MarketsResponse, LighterError> {
    let response = serde_json::from_slice::<MarketsResponse>(data)?;
    validate_response(&response)?;
    Ok(response)
}

/// Parses a Lighter orderbook response from raw JSON bytes.
///
/// # Errors
///
/// Returns an error if:
/// - Deserialization fails
/// - The response indicates an API error
pub fn parse_orderbook_response(
    data: &[u8],
) -> Result<LighterResponse<OrderbookResponse>, LighterError> {
    let response = serde_json::from_slice::<LighterResponse<OrderbookResponse>>(data)?;
    validate_response(&response)?;
    Ok(response)
}

/// Parses a Lighter trades response from raw JSON bytes.
///
/// # Errors
///
/// Returns an error if:
/// - Deserialization fails
/// - The response indicates an API error
pub fn parse_trades_response(data: &[u8]) -> Result<TradesResponse, LighterError> {
    let response = serde_json::from_slice::<TradesResponse>(data)?;
    validate_response(&response)?;
    Ok(response)
}

/// Parses a Lighter ticker response from raw JSON bytes.
///
/// # Errors
///
/// Returns an error if:
/// - Deserialization fails
/// - The response indicates an API error
pub fn parse_ticker_response(data: &[u8]) -> Result<LighterResponse<TickerResponse>, LighterError> {
    let response = serde_json::from_slice::<LighterResponse<TickerResponse>>(data)?;
    validate_response(&response)?;
    Ok(response)
}

/// Parses a Lighter account response from raw JSON bytes.
///
/// # Errors
///
/// Returns an error if:
/// - Deserialization fails
/// - The response indicates an API error
pub fn parse_account_response(
    data: &[u8],
) -> Result<LighterResponse<AccountResponse>, LighterError> {
    let response = serde_json::from_slice::<LighterResponse<AccountResponse>>(data)?;
    validate_response(&response)?;
    Ok(response)
}

/// Parses a Lighter order response from raw JSON bytes.
///
/// # Errors
///
/// Returns an error if:
/// - Deserialization fails
/// - The response indicates an API error
pub fn parse_order_response(data: &[u8]) -> Result<LighterResponse<OrderResponse>, LighterError> {
    let response = serde_json::from_slice::<LighterResponse<OrderResponse>>(data)?;
    validate_response(&response)?;
    Ok(response)
}

/// Parses a Lighter orders list response from raw JSON bytes.
///
/// # Errors
///
/// Returns an error if:
/// - Deserialization fails
/// - The response indicates an API error
pub fn parse_orders_response(data: &[u8]) -> Result<OrdersResponse, LighterError> {
    let response = serde_json::from_slice::<OrdersResponse>(data)?;
    validate_response(&response)?;
    Ok(response)
}

/// Parses read-only funding history risk inputs from raw JSON bytes.
///
/// # Errors
///
/// Returns an error if deserialization fails, the response indicates an API error, or numeric
/// string fields contain non-finite / invalid decimal values.
pub fn parse_funding_history_response(data: &[u8]) -> Result<FundingHistoryResponse, LighterError> {
    let response = serde_json::from_slice::<FundingHistoryResponse>(data)?;
    validate_response(&response)?;
    if let Some(data) = &response.data {
        for item in &data.items {
            validate_funding_history_entry(item)?;
        }
    }
    Ok(response)
}

/// Parses read-only margin-ratio risk inputs from raw JSON bytes.
///
/// # Errors
///
/// Returns an error if deserialization fails, the response indicates an API error, or numeric
/// string fields contain non-finite / invalid decimal values.
pub fn parse_margin_ratio_response(data: &[u8]) -> Result<MarginRiskResponse, LighterError> {
    let response = serde_json::from_slice::<MarginRiskResponse>(data)?;
    validate_response(&response)?;
    if let Some(data) = &response.data {
        validate_margin_ratio_response(data)?;
    }
    Ok(response)
}

/// Parses read-only liquidation threshold risk inputs from raw JSON bytes.
///
/// # Errors
///
/// Returns an error if deserialization fails, the response indicates an API error, or numeric
/// string fields contain non-finite / invalid decimal values.
pub fn parse_liquidation_threshold_response(
    data: &[u8],
) -> Result<LiquidationRiskResponse, LighterError> {
    let response = serde_json::from_slice::<LiquidationRiskResponse>(data)?;
    validate_response(&response)?;
    if let Some(data) = &response.data {
        validate_liquidation_threshold_response(data)?;
    }
    Ok(response)
}

/// Validates that a Lighter response indicates success.
///
/// # Errors
///
/// Returns an error if the response success field is false.
fn validate_response<T>(response: &LighterResponse<T>) -> Result<(), LighterError> {
    if !response.success {
        let error_msg = response
            .error
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("Unknown error");
        return Err(LighterError::Http(error_msg.to_string()));
    }
    Ok(())
}

fn validate_funding_history_entry(item: &FundingHistoryEntry) -> Result<(), LighterError> {
    validate_decimal_string("fundingRate", &item.funding_rate)?;
    validate_optional_decimal_string("premiumIndex", item.premium_index.as_deref())?;
    validate_optional_decimal_string("oraclePrice", item.oracle_price.as_deref())?;
    validate_optional_decimal_string("markPrice", item.mark_price.as_deref())?;
    Ok(())
}

fn validate_margin_ratio_response(item: &MarginRatioResponse) -> Result<(), LighterError> {
    validate_decimal_string("initialMarginRatio", &item.initial_margin_ratio)?;
    validate_decimal_string("maintenanceMarginRatio", &item.maintenance_margin_ratio)?;
    validate_decimal_string("closeoutMarginRatio", &item.closeout_margin_ratio)?;
    validate_optional_decimal_string("marginFraction", item.margin_fraction.as_deref())?;
    validate_optional_decimal_string("totalCollateral", item.total_collateral.as_deref())?;
    validate_optional_decimal_string("positionNotional", item.position_notional.as_deref())?;
    Ok(())
}

fn validate_liquidation_threshold_response(
    item: &LiquidationThresholdResponse,
) -> Result<(), LighterError> {
    validate_decimal_string("liquidationPrice", &item.liquidation_price)?;
    validate_optional_decimal_string("bankruptcyPrice", item.bankruptcy_price.as_deref())?;
    validate_optional_decimal_string(
        "maintenanceMarginRequired",
        item.maintenance_margin_required.as_deref(),
    )?;
    validate_optional_decimal_string(
        "closeoutMarginRequired",
        item.closeout_margin_required.as_deref(),
    )?;
    Ok(())
}

fn validate_optional_decimal_string(field: &str, value: Option<&str>) -> Result<(), LighterError> {
    if let Some(value) = value {
        validate_decimal_string(field, value)?;
    }
    Ok(())
}

fn validate_decimal_string(field: &str, value: &str) -> Result<(), LighterError> {
    let parsed = value
        .parse::<f64>()
        .map_err(|_| LighterError::Parse(format!("Invalid decimal string for {field}: {value}")))?;
    if !parsed.is_finite() {
        return Err(LighterError::Parse(format!(
            "Invalid non-finite decimal string for {field}: {value}"
        )));
    }
    Ok(())
}

/// Extracts the data payload from a Lighter response.
///
/// # Errors
///
/// Returns an error if the data field is None.
pub fn extract_data<T>(response: LighterResponse<T>) -> Result<T, LighterError> {
    response
        .data
        .ok_or_else(|| LighterError::Parse("Response data is None".to_string()))
}

/// Extracts a list of items from a Lighter list response.
///
/// # Errors
///
/// Returns an error if the data field is None.
pub fn extract_list<T>(response: LighterResponse<LighterList<T>>) -> Result<Vec<T>, LighterError> {
    let list = extract_data(response)?;
    Ok(list.items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_markets_response_success() {
        let json = r#"{
            "success": true,
            "data": {
                "items": [
                    {
                        "marketIndex": 1,
                        "symbol": "BTC/USDC",
                        "baseAsset": "BTC",
                        "quoteAsset": "USDC",
                        "tickSize": "0.01",
                        "stepSize": "0.001",
                        "minOrderSize": "0.001",
                        "maxOrderSize": "100",
                        "status": "active"
                    },
                    {
                        "marketIndex": 2,
                        "symbol": "ETH/USDC",
                        "baseAsset": "ETH",
                        "quoteAsset": "USDC",
                        "tickSize": "0.01",
                        "stepSize": "0.01",
                        "minOrderSize": "0.01",
                        "maxOrderSize": "1000",
                        "status": "active"
                    }
                ]
            }
        }"#;

        let result = parse_markets_response(json.as_bytes());
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.success);
        assert!(response.data.is_some());
        let data = response.data.unwrap();
        assert_eq!(data.items.len(), 2);
        assert_eq!(data.items[0].symbol.as_str(), "BTC/USDC");
        assert_eq!(data.items[1].symbol.as_str(), "ETH/USDC");
    }

    #[test]
    fn test_parse_markets_response_error() {
        let json = r#"{
            "success": false,
            "error": "Internal server error"
        }"#;

        let result = parse_markets_response(json.as_bytes());
        assert!(result.is_err());
        match result.unwrap_err() {
            LighterError::Http(msg) => assert_eq!(msg, "Internal server error"),
            _ => panic!("Expected Http error"),
        }
    }

    #[test]
    fn test_parse_orderbook_response_success() {
        let json = r#"{
            "success": true,
            "data": {
                "marketIndex": 1,
                "timestamp": 1703001600000,
                "bids": [
                    {"price": "43000.00", "quantity": "1.5"},
                    {"price": "42999.00", "quantity": "2.0"}
                ],
                "asks": [
                    {"price": "43001.00", "quantity": "1.2"},
                    {"price": "43002.00", "quantity": "0.8"}
                ]
            }
        }"#;

        let result = parse_orderbook_response(json.as_bytes());
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.success);
        let orderbook = response.data.unwrap();
        assert_eq!(orderbook.market_index, 1);
        assert_eq!(orderbook.bids.len(), 2);
        assert_eq!(orderbook.asks.len(), 2);
    }

    #[test]
    fn test_parse_trades_response_success() {
        let json = r#"{
            "success": true,
            "data": {
                "items": [
                    {
                        "marketIndex": 1,
                        "tradeId": 12345,
                        "price": "43000.00",
                        "quantity": "0.5",
                        "side": "buy",
                        "timestamp": 1703001600000
                    },
                    {
                        "marketIndex": 1,
                        "tradeId": 12346,
                        "price": "43001.00",
                        "quantity": "0.3",
                        "side": "sell",
                        "timestamp": 1703001601000
                    }
                ]
            }
        }"#;

        let result = parse_trades_response(json.as_bytes());
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.success);
        let data = response.data.unwrap();
        assert_eq!(data.items.len(), 2);
        assert_eq!(data.items[0].trade_id, 12345);
        assert_eq!(data.items[1].trade_id, 12346);
    }

    #[test]
    fn test_parse_ticker_response_success() {
        let json = r#"{
            "success": true,
            "data": {
                "marketIndex": 1,
                "lastPrice": "43000.00",
                "bestBid": "42999.00",
                "bestAsk": "43001.00",
                "volume24h": "1234.56",
                "high24h": "44000.00",
                "low24h": "42000.00",
                "priceChange24h": "1000.00",
                "timestamp": 1703001600000
            }
        }"#;

        let result = parse_ticker_response(json.as_bytes());
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.success);
        let ticker = response.data.unwrap();
        assert_eq!(ticker.market_index, 1);
        assert_eq!(ticker.last_price, "43000.00");
    }

    #[test]
    fn test_parse_order_response_success() {
        let json = r#"{
            "success": true,
            "data": {
                "orderIndex": 98765,
                "clientOrderIndex": 12345,
                "marketIndex": 1,
                "side": "buy",
                "orderType": "limit",
                "price": "43000.00",
                "quantity": "1.0",
                "filledQuantity": "0.5",
                "status": "partially_filled",
                "timeInForce": "good_till_time",
                "createdAt": 1703001600000,
                "updatedAt": 1703001700000
            }
        }"#;

        let result = parse_order_response(json.as_bytes());
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.success);
        let order = response.data.unwrap();
        assert_eq!(order.order_index, 98765);
        assert_eq!(order.client_order_index, 12345);
    }

    #[test]
    fn test_parse_account_response_success() {
        let json = r#"{
            "success": true,
            "data": {
                "accountIndex": 123456,
                "balances": [
                    {
                        "asset": "USDC",
                        "free": "10000.00",
                        "locked": "500.00"
                    },
                    {
                        "asset": "BTC",
                        "free": "0.5",
                        "locked": "0.1"
                    }
                ],
                "positions": [
                    {
                        "marketIndex": 1,
                        "size": "1.5",
                        "entryPrice": "42000.00",
                        "unrealizedPnl": "1500.00",
                        "margin": "2000.00"
                    }
                ]
            }
        }"#;

        let result = parse_account_response(json.as_bytes());
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.success);
        let account = response.data.unwrap();
        assert_eq!(account.account_index, 123456);
        assert_eq!(account.balances.len(), 2);
        assert_eq!(account.positions.len(), 1);
    }

    #[test]
    fn test_parse_funding_history_response_success() {
        let json = r#"{
            "success": true,
            "data": {
                "items": [
                    {
                        "marketIndex": 1,
                        "fundingRate": "0.000125",
                        "premiumIndex": "0.000031",
                        "oraclePrice": "43000.50",
                        "markPrice": "43005.25",
                        "timestamp": 1703001600000
                    },
                    {
                        "marketIndex": 1,
                        "fundingRate": "-0.000075",
                        "premiumIndex": "-0.000010",
                        "oraclePrice": "42990.00",
                        "markPrice": "42988.50",
                        "timestamp": 1703005200000
                    }
                ]
            }
        }"#;

        let response = parse_funding_history_response(json.as_bytes()).unwrap();
        let items = response.data.unwrap().items;
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].market_index, 1);
        assert_eq!(items[0].funding_rate, "0.000125");
        assert_eq!(items[1].funding_rate, "-0.000075");
    }

    #[test]
    fn test_parse_funding_history_response_empty_list_is_deterministic() {
        let json = r#"{
            "success": true,
            "data": { "items": [] }
        }"#;

        let response = parse_funding_history_response(json.as_bytes()).unwrap();
        assert!(response.data.unwrap().items.is_empty());
    }

    #[test]
    fn test_parse_margin_ratio_response_success() {
        let json = r#"{
            "success": true,
            "data": {
                "accountIndex": 878,
                "marketIndex": 1,
                "initialMarginRatio": "0.1000",
                "maintenanceMarginRatio": "0.0500",
                "closeoutMarginRatio": "0.0250",
                "marginFraction": "0.3125",
                "totalCollateral": "150000.00",
                "positionNotional": "48000.00",
                "timestamp": 1703001600000
            }
        }"#;

        let response = parse_margin_ratio_response(json.as_bytes()).unwrap();
        let risk = response.data.unwrap();
        assert_eq!(risk.account_index, 878);
        assert_eq!(risk.market_index, Some(1));
        assert_eq!(risk.initial_margin_ratio, "0.1000");
        assert_eq!(risk.maintenance_margin_ratio, "0.0500");
        assert_eq!(risk.closeout_margin_ratio, "0.0250");
    }

    #[test]
    fn test_parse_liquidation_threshold_response_success() {
        let json = r#"{
            "success": true,
            "data": {
                "accountIndex": 878,
                "marketIndex": 1,
                "liquidationPrice": "37500.25",
                "bankruptcyPrice": "35000.00",
                "maintenanceMarginRequired": "2400.00",
                "closeoutMarginRequired": "1200.00",
                "timestamp": 1703001600000
            }
        }"#;

        let response = parse_liquidation_threshold_response(json.as_bytes()).unwrap();
        let threshold = response.data.unwrap();
        assert_eq!(threshold.account_index, 878);
        assert_eq!(threshold.market_index, 1);
        assert_eq!(threshold.liquidation_price, "37500.25");
        assert_eq!(threshold.bankruptcy_price, Some("35000.00".to_string()));
    }

    #[test]
    fn test_parse_risk_input_missing_required_field_errors() {
        let json = r#"{
            "success": true,
            "data": {
                "accountIndex": 878,
                "marketIndex": 1,
                "maintenanceMarginRatio": "0.0500",
                "closeoutMarginRatio": "0.0250",
                "marginFraction": "0.3125",
                "totalCollateral": "150000.00",
                "positionNotional": "48000.00",
                "timestamp": 1703001600000
            }
        }"#;

        let result = parse_margin_ratio_response(json.as_bytes());
        assert!(matches!(result, Err(LighterError::Parse(_))));
    }

    #[test]
    fn test_parse_risk_input_invalid_number_errors() {
        let json = r#"{
            "success": true,
            "data": {
                "items": [
                    {
                        "marketIndex": 1,
                        "fundingRate": "not-a-number",
                        "premiumIndex": "0.000031",
                        "oraclePrice": "43000.50",
                        "markPrice": "43005.25",
                        "timestamp": 1703001600000
                    }
                ]
            }
        }"#;

        let result = parse_funding_history_response(json.as_bytes());
        assert!(matches!(result, Err(LighterError::Parse(_))));
    }

    #[test]
    fn test_extract_data_success() {
        let response = LighterResponse {
            success: true,
            error: None,
            data: Some(42),
        };

        let result = extract_data(response);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_extract_data_none() {
        let response: LighterResponse<i32> = LighterResponse {
            success: true,
            error: None,
            data: None,
        };

        let result = extract_data(response);
        assert!(result.is_err());
        match result.unwrap_err() {
            LighterError::Parse(msg) => assert_eq!(msg, "Response data is None"),
            _ => panic!("Expected Parse error"),
        }
    }

    #[test]
    fn test_extract_list_success() {
        let response = LighterResponse {
            success: true,
            error: None,
            data: Some(LighterList {
                items: vec![1, 2, 3, 4, 5],
            }),
        };

        let result = extract_list(response);
        assert!(result.is_ok());
        let list = result.unwrap();
        assert_eq!(list.len(), 5);
        assert_eq!(list, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_parse_invalid_json() {
        let json = r#"{"invalid": json"#;

        let result = parse_markets_response(json.as_bytes());
        assert!(result.is_err());
        match result.unwrap_err() {
            LighterError::Parse(_) => (),
            _ => panic!("Expected Parse error"),
        }
    }
}
