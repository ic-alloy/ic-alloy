#![doc = include_str!("../README.md")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/alloy-rs/core/main/assets/alloy.jpg",
    html_favicon_url = "https://raw.githubusercontent.com/alloy-rs/core/main/assets/favicon.ico"
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![allow(
    missing_docs,
    missing_debug_implementations,
    non_upper_case_globals,
    non_snake_case,
    clippy::enum_variant_names,
    clippy::large_enum_variant
)]
mod evm_rpc;

use alloy_json_rpc::{RequestPacket, ResponsePacket, SerializedRequest};
use alloy_transport::{TransportError, TransportFut};
use ic_cdk::api::call::CallResult;
use std::task;
use tower::Service;

pub use evm_rpc::*;

const DEFAULT_CALL_CYCLES: u128 = 60_000_000_000;

const MAX_RESPONSE_SIZE_SMALL: u64 = 1_000;
const MAX_RESPONSE_SIZE_MEDIUM: u64 = 2_000;
const MAX_RESPONSE_SIZE_UNKNOWN: u64 = 5_000;

/// Configuration details for an ICP transport.
#[derive(Clone, Debug)]
#[doc(hidden)]
pub struct IcpConfig {
    rpc_service: RpcService,
    call_cycles: Option<u128>,
    max_response_size: Option<u64>,
}

impl IcpConfig {
    /// Create a new [`IcpConfig`] with the given [`RpcService`] and default values for call cycles
    /// and max response size.
    pub const fn new(rpc_service: RpcService) -> Self {
        Self { rpc_service, call_cycles: None, max_response_size: None }
    }

    /// Set the call cycles for this config.
    pub const fn set_call_cycles(mut self, call_cycles: u128) -> Self {
        self.call_cycles = Some(call_cycles);
        self
    }

    /// Set the max response size for this config.
    pub const fn set_max_response_size(mut self, max_response_size: u64) -> Self {
        self.max_response_size = Some(max_response_size);
        self
    }
}

/// An ICP transport.
///
/// The user must provide an [`RpcService`] that specifies what
/// chain and provider to use
#[derive(Clone, Debug)]
pub struct IcpTransport {
    rpc_service: RpcService,
    call_cycles: Option<u128>,
    max_response_size: Option<u64>,
}

impl IcpTransport {
    /// Create a new [`IcpTransport`] using the given [`IcpConfig`] details.
    pub fn with_config(config: IcpConfig) -> Self {
        Self {
            rpc_service: config.rpc_service,
            call_cycles: config.call_cycles,
            max_response_size: config.max_response_size,
        }
    }

    /// Set the [`RpcService`] for this transport.
    pub fn set_rpc_service(&mut self, rpc_service: RpcService) {
        self.rpc_service = rpc_service;
    }

    /// Get a reference to the rpc service.
    pub const fn rpc_service(&self) -> &RpcService {
        &self.rpc_service
    }

    /// Set the call cycles for this transport.
    pub fn set_call_cycles(&mut self, call_cycles: u128) {
        self.call_cycles = Some(call_cycles);
    }

    /// Get the call cycles for this transport.
    pub const fn call_cycles(&self) -> Option<u128> {
        self.call_cycles
    }

    /// Set the max response size for this transport.
    pub fn set_max_response_size(&mut self, max_response_size: u64) {
        self.max_response_size = Some(max_response_size);
    }

    /// Get the max response size for this transport.
    pub const fn max_response_size(&self) -> Option<u64> {
        self.max_response_size
    }

    /// Check if the transport is local. Always `false` for now.
    pub const fn is_local(&self) -> bool {
        // Currently always returns false. We could add a check here to see
        // which DFX_NETWORK is being used and return true if it's local.
        // Not sure if that's necessary though.
        false
    }

    fn estimate_max_response_size(&self, request_packet: &RequestPacket) -> u64 {
        let max_response_size = |serialized_request: &SerializedRequest| -> u64 {
            match serialized_request.meta().method.as_ref() {
                "eth_blockNumber" => MAX_RESPONSE_SIZE_SMALL,
                "eth_getBalance" => MAX_RESPONSE_SIZE_SMALL,
                "eth_chainId" => MAX_RESPONSE_SIZE_SMALL,
                "eth_feeHistory" => MAX_RESPONSE_SIZE_MEDIUM,
                "eth_estimateGas" => MAX_RESPONSE_SIZE_SMALL,
                "eth_gasPrice" => MAX_RESPONSE_SIZE_SMALL,
                "eth_getBlockTransactionCountByHash" => MAX_RESPONSE_SIZE_SMALL,
                "eth_getBlockTransactionCountByNumber" => MAX_RESPONSE_SIZE_SMALL,
                "eth_getCode" => MAX_RESPONSE_SIZE_SMALL,
                "eth_getProof" => MAX_RESPONSE_SIZE_SMALL,
                "eth_getStorageAt" => MAX_RESPONSE_SIZE_SMALL,
                "eth_getTransactionByBlockHashAndIndex" => MAX_RESPONSE_SIZE_MEDIUM,
                "eth_getTransactionByHash" => MAX_RESPONSE_SIZE_MEDIUM,
                "eth_getTransactionCount" => MAX_RESPONSE_SIZE_SMALL,
                "eth_getUncleCountByBlockHash" => MAX_RESPONSE_SIZE_SMALL,
                "eth_getUncleCountByBlockNumber" => MAX_RESPONSE_SIZE_SMALL,
                "eth_maxPriorityFeePerGas" => MAX_RESPONSE_SIZE_SMALL,
                "eth_protocolVersion" => MAX_RESPONSE_SIZE_SMALL,
                _ => MAX_RESPONSE_SIZE_UNKNOWN,
            }
        };

        match request_packet {
            RequestPacket::Single(req) => max_response_size(req),
            RequestPacket::Batch(reqs) => reqs.iter().map(max_response_size).sum(),
        }
    }

    /// Make an EVM RPC request by calling the `request` method on the EVM RPC canister.
    fn request(&self, request_packet: RequestPacket) -> TransportFut<'static> {
        let rpc_service = self.rpc_service.clone();
        let max_response_size =
            self.max_response_size.unwrap_or(self.estimate_max_response_size(&request_packet));
        let call_cycles = self.call_cycles.unwrap_or(DEFAULT_CALL_CYCLES);

        Box::pin(async move {
            let serialized_request = request_packet.serialize().map_err(TransportError::ser_err)?;

            let call_result: CallResult<(RequestResult,)> = evm_rpc
                .request(
                    rpc_service,
                    serialized_request.to_string(),
                    max_response_size,
                    call_cycles,
                )
                .await;

            match call_result {
                Ok((request_result,)) => match request_result {
                    RequestResult::Ok(ok_result) => serde_json::from_str(&ok_result)
                        .map_err(|err| TransportError::deser_err(err, &ok_result)),
                    RequestResult::Err(rpc_error) => {
                        Err(TransportError::ErrorResp(alloy_json_rpc::ErrorPayload {
                            code: 6, // RPC error
                            message: format!("{:?}", rpc_error),
                            data: None,
                        }))
                    }
                },
                Err(err) => Err(TransportError::ErrorResp(alloy_json_rpc::ErrorPayload {
                    code: err.0 as i64,
                    message: err.1,
                    data: None,
                })),
            }
        })
    }
}

impl Service<RequestPacket> for IcpTransport {
    type Response = ResponsePacket;
    type Error = TransportError;
    type Future = TransportFut<'static>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut task::Context<'_>) -> task::Poll<Result<(), Self::Error>> {
        // The IcpTransport is always ready to make requests.
        task::Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: RequestPacket) -> Self::Future {
        self.request(req)
    }
}

impl Service<RequestPacket> for &IcpTransport {
    type Response = ResponsePacket;
    type Error = TransportError;
    type Future = TransportFut<'static>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut task::Context<'_>) -> task::Poll<Result<(), Self::Error>> {
        // The IcpTransport is always ready to make requests.
        task::Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: RequestPacket) -> Self::Future {
        self.request(req)
    }
}
