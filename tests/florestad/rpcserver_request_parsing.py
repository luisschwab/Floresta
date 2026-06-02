# SPDX-License-Identifier: MIT OR Apache-2.0

"""
Tests for JSON-RPC request parsing in florestad.

Validates that the RPC server correctly handles:
- Positional (array) parameters
- Named (object) parameters
- Null / omitted parameters
- Default values for optional parameters
- Proper JSON-RPC error codes per the spec (-32700, -32600, -32601, -32602, -32603)
- HTTP status codes (400, 404, 500, 503)
- Methods that require no params vs methods that require params
- JSON-RPC 1.0 and 2.0 version acceptance
- Content-type handling
"""

from test_framework.constants import (
    JSONRPC_ERRCODE_INVALID_PARAMS,
    JSONRPC_ERRCODE_INVALID_REQUEST,
    JSONRPC_ERRCODE_METHOD_NOT_FOUND,
    JSONRPC_ERRMSG_INVALID_VERSION,
    JSONRPC_ERRMSG_METHOD_NOT_FOUND,
    JSONRPC_ERRMSG_MISSING_PARAMS,
    JSONRPC_ERRMSG_WRONG_PARAM_TYPE,
    METHODS_REQUIRING_PARAMS,
    NO_PARAM_METHODS,
)
from test_framework.rpc.base import (
    assert_rpc_error,
    assert_rpc_success,
    make_raw_data_request,
    make_raw_request,
    make_request,
)


class TestRpcServerRequestParsing:
    """
    Test JSON-RPC request parsing, parameter extraction (positional and named),
    error codes, and edge cases on the florestad RPC server.
    """

    def test_noparammethods_omittedparams_succeeds(self, shared_florestad_node):
        """Verify all no-param methods succeed when the params field is omitted."""
        for method in NO_PARAM_METHODS:
            resp = make_request(shared_florestad_node, method)
            assert_rpc_success(resp)

    def test_noparammethods_nullparams_succeeds(self, shared_florestad_node):
        """Verify all no-param methods succeed when params is explicitly null."""
        for method in NO_PARAM_METHODS:
            resp = make_request(shared_florestad_node, method, params=None)
            assert_rpc_success(resp)

    def test_noparammethods_emptyarray_succeeds(self, shared_florestad_node):
        """Verify all no-param methods succeed when params is an empty array."""
        for method in NO_PARAM_METHODS:
            resp = make_request(shared_florestad_node, method, params=[])
            assert_rpc_success(resp)

    def test_positionalparams_validargs_succeeds(self, shared_florestad_node):
        """Verify methods accept valid positional (array) parameters."""
        resp = make_request(shared_florestad_node, "getblockhash", params=[0])
        assert_rpc_success(resp)
        genesis_hash = resp["body"]["result"]
        resp = make_request(
            shared_florestad_node, "getblockheader", params=[genesis_hash]
        )
        assert_rpc_success(resp)
        resp = make_request(shared_florestad_node, "getblock", params=[genesis_hash, 1])
        assert_rpc_success(resp)

    def test_namedparams_validargs_succeeds(self, shared_florestad_node):
        """Verify methods accept valid named (object) parameters."""
        resp = make_request(
            shared_florestad_node, "getblockhash", params={"block_height": 0}
        )
        assert_rpc_success(resp)
        genesis_hash = resp["body"]["result"]
        resp = make_request(
            shared_florestad_node,
            "getblockheader",
            params={"block_hash": genesis_hash},
        )
        assert_rpc_success(resp)
        resp = make_request(
            shared_florestad_node,
            "getblock",
            params={"block_hash": genesis_hash, "verbosity": 0},
        )
        assert_rpc_success(resp)

    def test_optionalparams_omitted_usesdefaults(self, shared_florestad_node):
        """Verify omitted optional parameters fall back to their defaults."""
        genesis_hash = shared_florestad_node.rpc.get_bestblockhash()
        resp_default = make_request(
            shared_florestad_node, "getblock", params=[genesis_hash]
        )
        assert_rpc_success(resp_default)
        result = resp_default["body"]["result"]
        # Check that the default verbosity was enabled.
        assert "hash" in result
        assert "tx" in result

        resp_explicit = make_request(
            shared_florestad_node, "getblock", params=[genesis_hash, 1]
        )
        assert_rpc_success(resp_explicit)
        assert resp_default["body"]["result"] == resp_explicit["body"]["result"]

        resp = make_request(
            shared_florestad_node, "getblock", params={"block_hash": genesis_hash}
        )
        assert_rpc_success(resp)
        assert resp_default["body"]["result"] == resp_explicit["body"]["result"]
        assert "hash" in resp["body"]["result"]

    def test_unknownmethod_anyparams_returnsmethodnotfound(self, shared_florestad_node):
        """Verify unknown methods return METHOD_NOT_FOUND (-32601)."""
        resp = make_request(shared_florestad_node, "nonexistent_method", params=[])
        assert_rpc_error(
            resp,
            expected_status_code=404,
            expected_rpcerror_code=JSONRPC_ERRCODE_METHOD_NOT_FOUND,
            expected_message=JSONRPC_ERRMSG_METHOD_NOT_FOUND,
        )

    def test_requiredparams_missing_returnsinvalidparams(self, shared_florestad_node):
        """Verify missing required parameters return INVALID_PARAMS (-32602)."""
        resp = make_request(shared_florestad_node, "getblockhash", params=[])
        assert_rpc_error(
            resp,
            expected_status_code=400,
            expected_rpcerror_code=JSONRPC_ERRCODE_INVALID_PARAMS,
            expected_message=JSONRPC_ERRMSG_MISSING_PARAMS,
        )

        # {} is an empty object, so it should be accepted as an object
        # but raise that is missing the fields
        resp = make_request(shared_florestad_node, "getblockhash", params={})
        assert_rpc_error(
            resp,
            expected_status_code=400,
            expected_rpcerror_code=JSONRPC_ERRCODE_INVALID_PARAMS,
            expected_message=JSONRPC_ERRMSG_MISSING_PARAMS,
        )

    def test_paramtypes_wrongtype_returnsinvalidparams(self, shared_florestad_node):
        """Verify wrong parameter types return INVALID_PARAMS (-32602)."""

        # getblockhash expects a number, but "not_a_number" is a string - params must be array
        resp = make_request(
            shared_florestad_node, "getblockhash", params=["not_a_number"]
        )
        assert_rpc_error(
            resp,
            expected_status_code=400,
            expected_rpcerror_code=JSONRPC_ERRCODE_INVALID_PARAMS,
            expected_message=JSONRPC_ERRMSG_WRONG_PARAM_TYPE,
        )

        # getblock hash expects a string, but 12345 is a number - params must be array
        resp = make_request(shared_florestad_node, "getblock", params=[12345])
        assert_rpc_error(
            resp,
            expected_status_code=400,
            expected_rpcerror_code=JSONRPC_ERRCODE_INVALID_PARAMS,
            expected_message=JSONRPC_ERRMSG_WRONG_PARAM_TYPE,
        )

        genesis_hash = shared_florestad_node.rpc.get_bestblockhash()
        resp = make_request(
            shared_florestad_node,
            "getblock",
            params=[genesis_hash, "invalid_verbosity"],
        )

        assert_rpc_error(
            resp,
            expected_status_code=400,
            expected_rpcerror_code=JSONRPC_ERRCODE_INVALID_PARAMS,
            expected_message=JSONRPC_ERRMSG_WRONG_PARAM_TYPE,
        )

    def test_jsonrpcversion_invalid_returnsrejection(self, shared_florestad_node):
        """Verify invalid jsonrpc versions are rejected and valid ones accepted."""
        resp = make_raw_request(
            shared_florestad_node,
            {"jsonrpc": "3.0", "id": "test", "method": "getblockcount", "params": []},
        )

        assert_rpc_error(
            resp,
            expected_status_code=400,
            expected_rpcerror_code=JSONRPC_ERRCODE_INVALID_REQUEST,
            expected_message=JSONRPC_ERRMSG_INVALID_VERSION,
        )

        for version in ["1.0", "2.0"]:
            resp = make_raw_request(
                shared_florestad_node,
                {
                    "jsonrpc": version,
                    "id": "test",
                    "method": "getblockcount",
                    "params": [],
                },
            )
            assert_rpc_success(resp)

        resp = make_raw_request(
            shared_florestad_node, {"id": "test", "method": "getblockcount"}
        )
        assert_rpc_success(resp)

    def test_parammethods_omittedparams_returnserror(self, shared_florestad_node):
        """Verify methods that require params fail when params are omitted."""
        for method in METHODS_REQUIRING_PARAMS:
            resp = make_request(shared_florestad_node, method)
            assert_rpc_error(
                resp,
                expected_status_code=400,
                expected_rpcerror_code=JSONRPC_ERRCODE_INVALID_PARAMS,
                expected_message=JSONRPC_ERRMSG_MISSING_PARAMS,
            )

    def test_responsestructure_success_matchesjsonrpcspec(self, shared_florestad_node):
        """Verify successful responses match the JSON-RPC spec structure."""
        resp = make_raw_request(
            shared_florestad_node,
            {"jsonrpc": "2.0", "id": "struct_test", "method": "getblockcount"},
        )
        body = resp["body"]
        assert "result" in body
        assert "id" in body
        assert body["id"] == "struct_test"
        assert body.get("result") is not None

    def test_responsestructure_error_matchesjsonrpcspec(self, shared_florestad_node):
        """Verify error responses match the JSON-RPC spec structure."""
        resp = make_raw_request(
            shared_florestad_node,
            {
                "jsonrpc": "2.0",
                "id": "struct_err",
                "method": "nonexistent",
                "params": [],
            },
        )
        body = resp["body"]
        assert "error" in body
        assert "id" in body
        assert body["id"] == "struct_err"
        err = body["error"]
        assert "code" in err
        assert "message" in err
        assert isinstance(err["code"], int)

    def test_jsonrpc_v1_explicit_version_succeeds(self, shared_florestad_node):
        """Verify requests with explicit jsonrpc 1.0 version succeed."""
        resp = make_raw_request(
            shared_florestad_node,
            {"jsonrpc": "1.0", "id": "test", "method": "getblockcount", "params": []},
        )
        assert_rpc_success(resp)

    def test_jsonrpc_v1_omitted_version_succeeds(self, shared_florestad_node):
        """Verify requests without jsonrpc field succeed (JSON-RPC 1.0 style)."""
        resp = make_raw_request(
            shared_florestad_node,
            {"id": "test", "method": "getblockcount"},
        )
        assert_rpc_success(resp)

    def test_contenttype_applicationjson_succeeds(self, shared_florestad_node):
        """Verify requests with application/json content-type succeed."""
        resp = make_raw_request(
            shared_florestad_node,
            {"jsonrpc": "2.0", "id": "test", "method": "getblockcount"},
            content_type="application/json",
        )
        assert_rpc_success(resp)

    def test_contenttype_textplain_succeeds(self, shared_florestad_node):
        """Verify requests with text/plain content-type succeed."""
        resp = make_raw_request(
            shared_florestad_node,
            {"jsonrpc": "2.0", "id": "test", "method": "getblockcount"},
            content_type="text/plain",
        )
        assert_rpc_success(resp)

    def test_contenttype_nonjson_body_rejected(self, shared_florestad_node):
        """Verify non-JSON body is rejected regardless of content-type."""
        resp = make_raw_data_request(
            shared_florestad_node,
            data="this is not json",
            content_type="text/plain",
        )
        assert_rpc_error(resp, expected_status_code=400)
