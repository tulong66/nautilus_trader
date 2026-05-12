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

use nautilus_lighter::signing::{
    LighterSigner, LighterStrategySigner, SigningError, StrategySigningCapability,
};

#[test]
fn strategy_signing_surface_only_exposes_order_cancel_and_auth_token_operations() {
    let allowed = LighterSigner::strategy_signing_surface();

    assert_eq!(
        allowed,
        [
            "create_auth_token",
            "sign_create_order",
            "sign_cancel_order",
            "sign_cancel_all_orders",
        ]
    );
}

#[test]
fn strategy_signing_surface_excludes_funds_and_account_mutation_operations() {
    let allowed = LighterSigner::strategy_signing_surface();
    let denied = [
        "withdraw",
        "transfer",
        "sign_withdraw",
        "sign_transfer",
        "update_leverage",
        "update_margin",
        "set_leverage",
        "set_margin",
        "adjust_margin",
        "create_sub_account",
        "stake",
        "unstake",
    ];

    for operation in denied {
        assert!(!allowed.contains(&operation), "{operation} must not be strategy-reachable");
    }
}

#[test]
fn strategy_signer_surface_matches_low_level_strategy_surface() {
    assert_eq!(
        LighterStrategySigner::strategy_signing_surface(),
        LighterSigner::strategy_signing_surface()
    );
}

#[test]
fn strategy_capabilities_are_limited_to_order_cancel_and_auth_token() {
    let capabilities = [
        StrategySigningCapability::CreateAuthToken,
        StrategySigningCapability::CreateOrder,
        StrategySigningCapability::CancelOrder,
        StrategySigningCapability::CancelAllOrders,
    ];
    let names = capabilities.map(StrategySigningCapability::as_str);

    assert_eq!(
        names,
        [
            "create_auth_token",
            "sign_create_order",
            "sign_cancel_order",
            "sign_cancel_all_orders",
        ]
    );
}

#[test]
fn strategy_signer_rejects_signing_when_live_signing_disabled() {
    let private_key =
        "00000000000000000000000000000000000000000000000000000000000000000000000000000001";
    let inner = LighterSigner::new(private_key, LighterSigner::CHAIN_ID_TESTNET, 2, 42, 0)
        .expect("test signer");

    let signer = LighterStrategySigner::new(inner, false);
    let err = signer
        .create_auth_token(1_900_000_000)
        .expect_err("live signing should be disabled");

    assert!(matches!(err, SigningError::LiveSigningDisabled));
}

#[test]
fn strategy_signer_allows_auth_token_when_live_signing_enabled() {
    let private_key =
        "00000000000000000000000000000000000000000000000000000000000000000000000000000001";
    let inner = LighterSigner::new(private_key, LighterSigner::CHAIN_ID_TESTNET, 2, 42, 0)
        .expect("test signer");

    let signer = LighterStrategySigner::new(inner, true);
    let token = signer
        .create_auth_token(1_900_000_000)
        .expect("auth token signing should be allowed");

    assert!(!token.is_empty());
}
