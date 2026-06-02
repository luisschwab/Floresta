// SPDX-License-Identifier: MIT OR Apache-2.0

//! This module defines the structure for JSON-RPC requests and provides utility functions to
//! extract parameters from the request.

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
/// Represents a JSON-RPC request (versions 1.0 and 2.0).
pub struct RpcRequest {
    /// The JSON-RPC version, typically "2.0".
    ///
    /// For JSON-RPC 2.0, this field is required. For earlier versions, it may be omitted.
    ///
    /// Source: <`https://json-rpc.dev/docs/reference/version-diff`>
    pub jsonrpc: Option<String>,

    /// The method to be invoked, e.g., "getblock", "sendtransaction".
    pub method: String,

    /// The parameters for the method, json value that must be an array or an object.
    pub params: Option<Value>,

    /// An optional identifier for the request, which can be used to match responses.
    pub id: Value,
}

/// Some utility functions to extract parameters from the request. These
/// methods already handle the case where the parameter is missing or has an
/// unexpected type, returning an error if so.
pub mod arg_parser {

    use serde::Deserialize;
    use serde_json::Value;

    use crate::json_rpc::res::jsonrpc_interface::JsonRpcError;

    /// Extracts a parameter from the request parameters at the specified index.
    ///
    /// This function can extract any type that implements `FromStr`, such as `BlockHash` or
    /// `Txid`. It checks if the parameter exists and is a valid string representation of the type.
    /// Returns an error otherwise.
    pub fn get_at<'de, T: Deserialize<'de>>(
        params: &'de Value,
        index: usize,
        field_name: &str,
    ) -> Result<T, JsonRpcError> {
        if params.is_null() {
            return Err(JsonRpcError::MissingParameter(field_name.to_string()));
        }

        let v = match (params.is_array(), params.is_object()) {
            (true, false) => params.get(index),
            (false, true) => params.get(field_name),
            _ => {
                return Err(JsonRpcError::InvalidParameterStructure(
                    (*params).to_string(),
                ));
            }
        };

        let value = v.ok_or(JsonRpcError::MissingParameter(field_name.to_string()))?;

        T::deserialize(value)
            .map_err(|e| JsonRpcError::InvalidParameterType(format!("{field_name}: {e}")))
    }

    /// Wraps a parameter extraction result so that a missing parameter yields `Ok(None)`
    /// instead of an error. Other errors are propagated unchanged.
    pub fn try_into_optional<T>(
        result: Result<T, JsonRpcError>,
    ) -> Result<Option<T>, JsonRpcError> {
        match result {
            Ok(t) => Ok(Some(t)),
            Err(JsonRpcError::MissingParameter(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Like [`get_at`], but returns `default` when the parameter is missing instead of
    /// an error. Type mismatches are still propagated as errors.
    pub fn get_with_default<'de, T: Deserialize<'de>>(
        v: &'de Value,
        index: usize,
        field_name: &str,
        default: T,
    ) -> Result<T, JsonRpcError> {
        match get_at(v, index, field_name) {
            Ok(t) => Ok(t),
            Err(JsonRpcError::MissingParameter(_)) => Ok(default),
            Err(e) => Err(e),
        }
    }
}
