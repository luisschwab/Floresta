//! A collection of functions that implement the consensus rules for the Bitcoin Network.
//! This module contains functions that are used to verify blocks and transactions, and doesn't
//! assume anything about the chainstate, so it can be used in any context.
//! We use this to avoid code reuse among the different implementations of the chainstate.
extern crate alloc;

use core::ffi::c_uint;

use bitcoin::block::Header as BlockHeader;
use bitcoin::consensus::Encodable;
use bitcoin::hashes::sha256;
use bitcoin::hashes::Hash;
use bitcoin::Block;
use bitcoin::BlockHash;
use bitcoin::CompactTarget;
use bitcoin::OutPoint;
use bitcoin::ScriptBuf;
use bitcoin::Target;
use bitcoin::Transaction;
use bitcoin::TxIn;
use bitcoin::TxOut;
use bitcoin::Txid;
use bitcoin::WitnessVersion;
use floresta_common::prelude::*;
use rustreexo::accumulator::node_hash::NodeHash;
use rustreexo::accumulator::proof::Proof;
use rustreexo::accumulator::stump::Stump;
use sha2::Digest;
use sha2::Sha512_256;

use super::chainparams::ChainParams;
use super::error::BlockValidationErrors;
use super::error::BlockchainError;
use crate::TransactionError;

/// The value of a single coin in satoshis.
pub const COIN_VALUE: u64 = 100_000_000;

/// The version tag to be prepended to the leafhash. It's just the sha512 hash of the string
/// `UtreexoV1` represented as a vector of [u8] ([85 116 114 101 101 120 111 86 49]).
/// The same tag is "5574726565786f5631" as a hex string.
pub const UTREEXO_TAG_V1: [u8; 64] = [
    0x5b, 0x83, 0x2d, 0xb8, 0xca, 0x26, 0xc2, 0x5b, 0xe1, 0xc5, 0x42, 0xd6, 0xcc, 0xed, 0xdd, 0xa8,
    0xc1, 0x45, 0x61, 0x5c, 0xff, 0x5c, 0x35, 0x72, 0x7f, 0xb3, 0x46, 0x26, 0x10, 0x80, 0x7e, 0x20,
    0xae, 0x53, 0x4d, 0xc3, 0xf6, 0x42, 0x99, 0x19, 0x99, 0x31, 0x77, 0x2e, 0x03, 0x78, 0x7d, 0x18,
    0x15, 0x6e, 0xb3, 0x15, 0x1e, 0x0e, 0xd1, 0xb3, 0x09, 0x8b, 0xdc, 0x84, 0x45, 0x86, 0x18, 0x85,
];

/// This struct contains all the information and methods needed to validate a block,
/// it is used by the [ChainState] to validate blocks and transactions.
#[derive(Debug, Clone)]
pub struct Consensus {
    /// The parameters of the chain we are validating, it is usually hardcoded
    /// constants. See [ChainParams] for more information.
    pub parameters: ChainParams,
}

impl Consensus {
    /// Returns the amount of block subsidy to be paid in a block, given it's height.
    /// Bitcoin Core source: https://github.com/bitcoin/bitcoin/blob/2b211b41e36f914b8d0487e698b619039cc3c8e2/src/validation.cpp#L1501-L1512
    pub fn get_subsidy(&self, height: u32) -> u64 {
        let halvings = height / self.parameters.subsidy_halving_interval as u32;
        // Force block reward to zero when right shift is undefined.
        if halvings >= 64 {
            return 0;
        }
        let mut subsidy = 50 * COIN_VALUE;
        // Subsidy is cut in half every 210,000 blocks which will occur approximately every 4 years.
        subsidy >>= halvings;
        subsidy
    }

    /// Returns the hash of a leaf node in the utreexo accumulator.
    #[inline]
    fn get_leaf_hashes(
        transaction: &Transaction,
        vout: u32,
        height: u32,
        block_hash: BlockHash,
    ) -> sha256::Hash {
        let header_code = height << 1;

        let mut ser_utxo = Vec::new();
        let utxo = transaction.output.get(vout as usize).unwrap();
        utxo.consensus_encode(&mut ser_utxo).unwrap();
        let header_code = if transaction.is_coinbase() {
            header_code | 1
        } else {
            header_code
        };

        let leaf_hash = Sha512_256::new()
            .chain_update(UTREEXO_TAG_V1)
            .chain_update(UTREEXO_TAG_V1)
            .chain_update(block_hash)
            .chain_update(transaction.compute_txid())
            .chain_update(vout.to_le_bytes())
            .chain_update(header_code.to_le_bytes())
            .chain_update(ser_utxo)
            .finalize();
        sha256::Hash::from_slice(leaf_hash.as_slice())
            .expect("parent_hash: Engines shouldn't be Err")
    }
    /// Verify if all transactions in a block are valid. Here we check the following:
    /// - The block must contain at least one transaction, and this transaction must be coinbase
    /// - The first transaction in the block must be coinbase
    /// - The coinbase transaction must have the correct value (subsidy + fees)
    /// - The block must not create more coins than allowed
    /// - All transactions must be valid:
    ///     - The transaction must not be coinbase (already checked)
    ///     - The transaction must not have duplicate inputs
    ///     - The transaction must not spend more coins than it claims in the inputs
    ///     - The transaction must have valid scripts
    #[allow(unused)]
    pub fn verify_block_transactions(
        height: u32,
        mut utxos: HashMap<OutPoint, TxOut>,
        transactions: &[Transaction],
        subsidy: u64,
        verify_script: bool,
        flags: c_uint,
    ) -> Result<(), BlockchainError> {
        // TODO: RETURN A GENERIC WRAPPER TYPE.
        // Blocks must contain at least one transaction
        if transactions.is_empty() {
            return Err(BlockValidationErrors::EmptyBlock.into());
        }
        let mut fee = 0;
        let mut wu: u64 = 0;
        // Skip the coinbase tx
        for (n, transaction) in transactions.iter().enumerate() {
            // We don't need to verify the coinbase inputs, as it spends newly generated coins
            if transaction.is_coinbase() && n == 0 {
                Self::verify_coinbase(transaction.clone(), n as u16).map_err(|err| {
                    TransactionError {
                        txid: transaction.compute_txid(),
                        error: err,
                    }
                });
                continue;
            }
            // Amount of all outputs
            let mut output_value = 0;
            for output in transaction.output.iter() {
                Self::get_out_value(output, &mut output_value).map_err(|err| TransactionError {
                    txid: transaction.compute_txid(),
                    error: err,
                });
                Self::validate_script_size(&output.script_pubkey).map_err(|err| TransactionError {
                    txid: transaction.compute_txid(),
                    error: err,
                });
            }
            // Amount of all inputs
            let mut in_value = 0;
            for input in transaction.input.iter() {
                Self::consume_utxos(input, &mut utxos, &mut in_value).map_err(|err| {
                    TransactionError {
                        txid: transaction.compute_txid(),
                        error: err,
                    }
                });
                Self::validate_script_size(&input.script_sig).map_err(|err| TransactionError {
                    txid: transaction.compute_txid(),
                    error: err,
                });
            }
            // Value in should be greater or equal to value out. Otherwise, inflation.
            if output_value > in_value {
                return Err(TransactionError {
                    txid: transaction.compute_txid(),
                    error: BlockValidationErrors::NotEnoughMoney,
                }
                .into());
            }
            if output_value > 21_000_000 * 100_000_000 {
                return Err(BlockValidationErrors::TooManyCoins.into());
            }
            // Fee is the difference between inputs and outputs
            fee += in_value - output_value;
            // Verify the tx script
            #[cfg(feature = "bitcoinconsensus")]
            if verify_script {
                transaction
                    .verify_with_flags(|outpoint| utxos.remove(outpoint), flags)
                    .map_err(|err| TransactionError {
                        txid: transaction.compute_txid(),
                        error: BlockValidationErrors::ScriptValidationError(err.to_string()),
                    });
            };

            //checks vbytes validation
            //After all the checks, we sum the transaction weight to the block weight
            wu += transaction.weight().to_wu();
        }
        //checks if the block weight is fine.
        if wu > 4_000_000 {
            return Err(BlockValidationErrors::BlockTooBig.into());
        }
        // Checks if the miner isn't trying to create inflation
        if fee + subsidy
            < transactions[0]
                .output
                .iter()
                .fold(0, |acc, out| acc + out.value.to_sat())
        {
            return Err(BlockValidationErrors::BadCoinbaseOutValue.into());
        }
        Ok(())
    }
    /// Consumes the UTXOs from the hashmap, and returns the value of the consumed UTXOs.
    /// If we do not find the UTXO, we return an error invalidating the input that tried to
    /// consume that UTXO.
    fn consume_utxos(
        input: &TxIn,
        utxos: &mut HashMap<OutPoint, TxOut>,
        value_var: &mut u64,
    ) -> Result<(), BlockValidationErrors> {
        match utxos.get(&input.previous_output) {
            Some(prevout) => {
                *value_var += prevout.value.to_sat();
                utxos.remove(&input.previous_output);
            }
            None => {
                return Err(BlockValidationErrors::UtxoAlreadySpent(
                    //This is the case when the spender:
                    // - Spends an UTXO that doesn't exist
                    // - Spends an UTXO that was already spent
                    input.previous_output.txid,
                ));
            }
        };
        Ok(())
    }
    #[allow(unused)]
    fn validate_locktime(
        input: &TxIn,
        transaction: &Transaction,
        height: u32,
    ) -> Result<(), BlockValidationErrors> {
        unimplemented!("validate_locktime")
    }
    /// Validates the script size and the number of sigops in a script.
    fn validate_script_size(script: &ScriptBuf) -> Result<(), BlockValidationErrors> {
        let scriptpubkeysize = script.len();
        let is_taproot =
            script.witness_version() == Some(WitnessVersion::V1) && scriptpubkeysize == 32;
        if scriptpubkeysize > 520 || scriptpubkeysize < 2 && !is_taproot {
            //the scriptsig size must be between 2 and 100 bytes unless is taproot
            return Err(BlockValidationErrors::ScriptError);
        }
        if script.count_sigops() > 80_000 {
            return Err(BlockValidationErrors::ScriptError);
        }
        Ok(())
    }
    fn get_out_value(out: &TxOut, value_var: &mut u64) -> Result<(), BlockValidationErrors> {
        if out.value.to_sat() > 0 {
            *value_var += out.value.to_sat()
        } else {
            return Err(BlockValidationErrors::InvalidOutput);
        }
        Ok(())
    }
    fn verify_coinbase(transaction: Transaction, index: u16) -> Result<(), BlockValidationErrors> {
        if index != 0 {
            // A block must contain only one coinbase, and it should be the fist thing inside it
            return Err(BlockValidationErrors::FirstTxIsnNotCoinbase);
        }
        //the prevout input of a coinbase must be all zeroes
        if transaction.input[0].previous_output.txid != Txid::all_zeros() {
            return Err(BlockValidationErrors::InvalidCoinbase(
                "Invalid coinbase txid".to_string(),
            ));
        }
        let scriptsig = transaction.input[0].script_sig.clone();
        let scriptsigsize = scriptsig.clone().into_bytes().len();
        if !(2..=100).contains(&scriptsigsize) {
            //the scriptsig size must be between 2 and 100 bytes
            return Err(BlockValidationErrors::InvalidCoinbase(
                "Invalid ScriptSig size".to_string(),
            ));
        }
        Ok(())
    }
    /// Calculates the next target for the proof of work algorithm, given the
    /// current target and the time it took to mine the last 2016 blocks.
    pub fn calc_next_work_required(
        last_block: &BlockHeader,
        first_block: &BlockHeader,
        params: ChainParams,
    ) -> Target {
        let actual_timespan = last_block.time - first_block.time;

        CompactTarget::from_next_work_required(first_block.bits, actual_timespan as u64, params)
            .into()
    }
    /// Updates our accumulator with the new block. This is done by calculating the new
    /// root hash of the accumulator, and then verifying the proof of inclusion of the
    /// deleted nodes. If the proof is valid, we return the new accumulator. Otherwise,
    /// we return an error.
    /// This function is pure, it doesn't modify the accumulator, but returns a new one.
    pub fn update_acc(
        acc: &Stump,
        block: &Block,
        height: u32,
        proof: Proof,
        del_hashes: Vec<sha256::Hash>,
    ) -> Result<Stump, BlockchainError> {
        let block_hash = block.block_hash();
        let mut leaf_hashes = Vec::new();
        let del_hashes = del_hashes
            .iter()
            .map(|hash| NodeHash::from(hash.as_byte_array()))
            .collect::<Vec<_>>();
        // Get inputs from the block, we'll need this HashSet to check if an output is spent
        // in the same block. If it is, we don't need to add it to the accumulator.
        let mut block_inputs = HashSet::new();
        for transaction in block.txdata.iter() {
            for input in transaction.input.iter() {
                block_inputs.insert((input.previous_output.txid, input.previous_output.vout));
            }
        }

        // Get all leaf hashes that will be added to the accumulator
        for transaction in block.txdata.iter() {
            for (i, output) in transaction.output.iter().enumerate() {
                if !Self::is_unspendable(&output.script_pubkey)
                    && !block_inputs.contains(&(transaction.compute_txid(), i as u32))
                {
                    leaf_hashes.push(Self::get_leaf_hashes(
                        transaction,
                        i as u32,
                        height,
                        block_hash,
                    ))
                }
            }
        }
        // Convert the leaf hashes to NodeHashes used in Rustreexo
        let hashes: Vec<NodeHash> = leaf_hashes
            .iter()
            .map(|&hash| NodeHash::from(hash.as_byte_array()))
            .collect();
        // Update the accumulator
        let acc = acc.modify(&hashes, &del_hashes, &proof)?.0;
        Ok(acc)
    }

    fn is_unspendable(script: &ScriptBuf) -> bool {
        if script.len() > 10_000 {
            return true;
        }

        if !script.is_empty() && script.as_bytes()[0] == 0x6a {
            return true;
        }

        false
    }
}
#[cfg(test)]
mod tests {
    use bitcoin::absolute::LockTime;
    use bitcoin::hashes::sha256d::Hash;
    use bitcoin::transaction::Version;
    use bitcoin::Amount;
    use bitcoin::OutPoint;
    use bitcoin::ScriptBuf;
    use bitcoin::Sequence;
    use bitcoin::Transaction;
    use bitcoin::TxIn;
    use bitcoin::TxOut;
    use bitcoin::Txid;
    use bitcoin::Witness;

    use super::*;

    fn coinbase(is_valid: bool) -> Transaction {
        //This coinbase transactions was retrieved from https://learnmeabitcoin.com/explorer/block/0000000000000a0f82f8be9ec24ebfca3d5373fde8dc4d9b9a949d538e9ff679
        // Create inputs
        let input_txid = Txid::from_raw_hash(Hash::from_str(&format!("{:0>64}", "")).unwrap());

        let input_vout = 0;
        let input_outpoint = OutPoint::new(input_txid, input_vout);
        let input_script_sig = if is_valid {
            ScriptBuf::from_hex("03f0a2a4d9f0a2").unwrap()
        } else {
            //This should invalidate the coinbase transaction since is a big, really big, script.
            ScriptBuf::from_hex(&format!("{:0>420}", "")).unwrap()
        };

        let input_sequence = Sequence::MAX;
        let input = TxIn {
            previous_output: input_outpoint,
            script_sig: input_script_sig,
            sequence: input_sequence,
            witness: Witness::new(),
        };

        // Create outputs
        let output_value = Amount::from_sat(5_000_350_000);
        let output_script_pubkey = ScriptBuf::from_hex("41047eda6bd04fb27cab6e7c28c99b94977f073e912f25d1ff7165d9c95cd9bbe6da7e7ad7f2acb09e0ced91705f7616af53bee51a238b7dc527f2be0aa60469d140ac").unwrap();
        let output = TxOut {
            value: output_value,
            script_pubkey: output_script_pubkey,
        };

        // Create transaction
        let version = Version(1);
        let lock_time = LockTime::from_height(150_007).unwrap();

        Transaction {
            version,
            lock_time,
            input: vec![input],
            output: vec![output],
        }
    }

    #[test]
    fn test_validate_get_out_value() {
        let output = TxOut {
            value: Amount::from_sat(5_000_350_000),
            script_pubkey: ScriptBuf::from_hex("41047eda6bd04fb27cab6e7c28c99b94977f073e912f25d1ff7165d9c95cd9bbe6da7e7ad7f2acb09e0ced91705f7616af53bee51a238b7dc527f2be0aa60469d140ac").unwrap(),
        };
        let mut value_var = 0;
        assert!(Consensus::get_out_value(&output, &mut value_var).is_ok());
        assert_eq!(value_var, 5_000_350_000);
    }

    #[test]
    fn test_validate_script_size() {
        //the case when the script is too big
        let invalid_script = ScriptBuf::from_hex(&format!("{:0>1220}", "")).unwrap();
        //the valid script < 520 bytes
        let valid_script =
            ScriptBuf::from_hex("76a9149206a30c09cc853bb03bd917a4f9f29b089c1bc788ac").unwrap();
        assert!(Consensus::validate_script_size(&valid_script).is_ok());
        assert!(Consensus::validate_script_size(&invalid_script).is_err());
    }

    #[test]
    fn test_validate_coinbase() {
        let valid_one = coinbase(true);
        let invalid_one = coinbase(false);
        //The case that should be valid
        assert!(Consensus::verify_coinbase(valid_one.clone(), 0).is_ok());
        //Coinbase at wrong index
        assert_eq!(
            Consensus::verify_coinbase(valid_one, 1)
                .unwrap_err()
                .to_string(),
            "The first transaction in a block isn't a coinbase"
        );
        //Invalid coinbase script
        assert_eq!(
            Consensus::verify_coinbase(invalid_one, 0)
                .unwrap_err()
                .to_string(),
            "Invalid coinbase: \"Invalid ScriptSig size\""
        );
    }
    #[test]
    fn test_consume_utxos() {
        // Transaction extracted from https://learnmeabitcoin.com/explorer/tx/0094492b6f010a5e39c2aacc97396ce9b6082dc733a7b4151ccdbd580f789278
        // Mock data for testing

        let mut utxos = HashMap::new();
        let outpoint1 = OutPoint::new(
            Txid::from_raw_hash(
                Hash::from_str("5baf640769ebdf2b79868d0a259db69a2c1587232f83ba226ecf3dd0737759bd")
                    .unwrap(),
            ),
            1,
        );
        let input = TxIn {
            previous_output: outpoint1,
            script_sig: ScriptBuf::from_hex("493046022100841d4f503f44dd6cef8781270e7260db73d0e3c26c4f1eea61d008760000b01e022100bc2675b8598773984bcf0bb1a7cad054c649e8a34cb522a118b072a453de1bf6012102de023224486b81d3761edcd32cedda7cbb30a4263e666c87607883197c914022").unwrap(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        };
        let prevout = TxOut {
            value: Amount::from_sat(18000000),
            script_pubkey: ScriptBuf::from_hex(
                "76a9149206a30c09cc853bb03bd917a4f9f29b089c1bc788ac",
            )
            .unwrap(),
        };

        utxos.insert(outpoint1, prevout.clone());

        // Test consuming UTXOs
        let mut value_var: u64 = 0;
        assert!(Consensus::consume_utxos(&input, &mut utxos, &mut value_var).is_ok());
        assert_eq!(value_var, prevout.value.to_sat());

        // Test double consuming UTXOs
        assert_eq!(
            Consensus::consume_utxos(&input, &mut utxos, &mut value_var)
                .unwrap_err()
                .to_string(),
            "Utxo 5baf640769ebdf2b79868d0a259db69a2c1587232f83ba226ecf3dd0737759bd already spent"
        );
    }
}
