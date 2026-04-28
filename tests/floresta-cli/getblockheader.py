# SPDX-License-Identifier: MIT OR Apache-2.0

"""
floresta_cli_getblockheader.py

This functional test cli utility to interact with a Floresta node with `getblockheader`
"""

import pytest

from test_framework.constants import GENESIS_BLOCK_HASH


@pytest.mark.rpc
def test_get_block_header(florestad_node):
    """
    Test `getblockheader` to get the genesis block header.
    """

    result = florestad_node.rpc.get_blockheader(GENESIS_BLOCK_HASH)

    assert result["version"] == 1
    assert (
        result["prev_blockhash"]
        == "0000000000000000000000000000000000000000000000000000000000000000"
    )
    assert (
        result["merkle_root"]
        == "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b"
    )
    assert result["time"] == 1296688602
    assert result["bits"] == 545259519
    assert result["nonce"] == 2
