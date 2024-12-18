#![doc = include_str!("../README.md")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/alloy-rs/core/main/assets/alloy.jpg",
    html_favicon_url = "https://raw.githubusercontent.com/alloy-rs/core/main/assets/favicon.ico"
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

/// Type alias for a [`RootProvider`] using the [`Http`] transport and a
/// reqwest client.
///
/// [`Http`]: alloy_transport_http::Http
#[cfg(any(test, feature = "reqwest"))]
pub type ReqwestProvider<N = alloy_network::Ethereum> =
    crate::RootProvider<alloy_transport_http::Http<reqwest::Client>, N>;

/// Type alias for a [`RootProvider`] using the [`Http`] transport and a hyper
/// client.
///
/// [`Http`]: alloy_transport_http::Http
#[cfg(feature = "hyper")]
pub type HyperProvider<N = alloy_network::Ethereum> =
    crate::RootProvider<alloy_transport_http::Http<alloy_transport_http::HyperClient>, N>;

/// Type alias for a [`RootProvider`] using the [`IcpTransport`] transport.
///
/// [`IcpTransport`]: alloy_transport_icp::IcpTransport
#[cfg(any(test, feature = "icp"))]
pub type IcpProvider<N = alloy_network::Ethereum> =
    crate::RootProvider<alloy_transport_icp::IcpTransport, N>;

#[macro_use]
extern crate tracing;

mod builder;
pub use builder::{Identity, ProviderBuilder, ProviderLayer, Stack};

pub mod ext;

pub mod fillers;
pub mod layers;

mod chain;

mod heart;
pub use heart::{
    PendingTransaction, PendingTransactionBuilder, PendingTransactionConfig,
    PendingTransactionError, WatchTxError,
};

mod provider;
pub use provider::{
    builder, EthCall, FilterPollerBuilder, Provider, RootProvider, RpcWithBlock, SendableTx,
    WalletProvider,
};

pub mod utils;

#[doc(no_inline)]
pub use alloy_network::{self as network, Network};

#[cfg(feature = "ws")]
pub use alloy_rpc_client::WsConnect;

#[cfg(feature = "ipc")]
pub use alloy_rpc_client::IpcConnect;
