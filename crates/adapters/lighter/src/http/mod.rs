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

//! HTTP client integration for the Lighter DEX REST API.
//!
//! This module provides HTTP client infrastructure following the NautilusTrader
//! adapter pattern:
//!
//! - [`LighterRawHttpClient`]: Low-level HTTP methods matching Lighter API endpoints.
//! - [`endpoints`]: API endpoint constants.
//!
//! ## Key Features
//!
//! - Rate-limiting for API compliance
//! - Request/response logging with tracing
//! - Automatic retry for transient failures
//! - Authentication via X-Lighter-Auth header
//! - Error handling using [`crate::error::LighterError`]
//!
//! # Official Documentation
//!
//! Lighter API documentation: <https://docs.lighter.xyz>

pub mod client;
pub mod endpoints;
pub mod parse;
pub mod types;

pub use client::LighterRawHttpClient;
pub use endpoints::*;
pub use parse::*;
pub use types::*;
