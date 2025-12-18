# SPDX-License-Identifier: MIT OR Apache-2.0

"""
This module contains constants used throughout the Floresta tests.
"""

import os

# defaults to import...
GENESIS_BLOCK_HEIGHT = 0
GENESIS_BLOCK_HASH = "0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206"
GENESIS_BLOCK_DIFFICULTY_INT = 1
GENESIS_BLOCK_DIFFICULTY_FLOAT = 4.656542373906925e-10
GENESIS_BLOCK_LEAF_COUNT = 0
CHAIN_NAME = "regtest"
FLORESTA_TEMP_DIR = os.getenv("FLORESTA_TEMP_DIR")
