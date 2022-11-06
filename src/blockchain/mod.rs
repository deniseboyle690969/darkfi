/* This file is part of DarkFi (https://dark.fi)
 *
 * Copyright (C) 2020-2022 Dyne.org foundation
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation, either version 3 of the
 * License, or (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU Affero General Public License for more details.
 *
 * You should have received a copy of the GNU Affero General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

use darkfi_serial::serialize;
use log::debug;

use crate::{
    consensus::{Block, BlockInfo},
    util::time::Timestamp,
    Result,
};

pub mod blockstore;
pub use blockstore::{BlockOrderStore, BlockStore, HeaderStore};

pub mod nfstore;
pub use nfstore::NullifierStore;

pub mod rootstore;
pub use rootstore::RootStore;

pub mod txstore;
pub use txstore::TxStore;

pub mod contractstore;
pub use contractstore::ContractStore;

/// Structure holding all sled trees that define the concept of Blockchain.
#[derive(Clone)]
pub struct Blockchain {
    /// Main pointer to the sled db connection
    pub sled_db: sled::Db,
    /// Headers sled tree
    pub headers: HeaderStore,
    /// Blocks sled tree
    pub blocks: BlockStore,
    /// Block order sled tree
    pub order: BlockOrderStore,
    /// Transactions sled tree
    pub transactions: TxStore,
    /// Nullifiers sled tree
    pub nullifiers: NullifierStore,
    /// Merkle roots sled tree
    pub merkle_roots: RootStore,
    /// Contract states
    pub contracts: ContractStore,
}

impl Blockchain {
    //FIXME why the blockchain taking genesis_data on the constructor as a hash?
    //genesis data are supposed to be a a hash?
    /// Instantiate a new `Blockchain` with the given `sled` database.
    pub fn new(db: &sled::Db, genesis_ts: Timestamp, genesis_data: blake3::Hash) -> Result<Self> {
        let headers = HeaderStore::new(db, genesis_ts, genesis_data)?;
        let blocks = BlockStore::new(db, genesis_ts, genesis_data)?;
        let order = BlockOrderStore::new(db, genesis_ts, genesis_data)?;
        let transactions = TxStore::new(db)?;
        let nullifiers = NullifierStore::new(db)?;
        let merkle_roots = RootStore::new(db)?;
        let contracts = ContractStore::new(db)?;

        Ok(Self {
            sled_db: db.clone(),
            headers,
            blocks,
            order,
            transactions,
            nullifiers,
            merkle_roots,
            contracts,
        })
    }

    /// Insert a given slice of [`BlockInfo`] into the blockchain database.
    /// This functions wraps all the logic of separating the block into specific
    /// data that can be fed into the different trees of the database.
    /// Upon success, the functions returns a vector of the block hashes that
    /// were given and appended to the ledger.
    pub fn add(&self, blocks: &[BlockInfo]) -> Result<Vec<blake3::Hash>> {
        let mut ret = Vec::with_capacity(blocks.len());

        for block in blocks {
            // Store transactions
            let _tx_hashes = self.transactions.insert(&block.txs)?;

            // Store header
            let headerhash = self.headers.insert(&[block.header.clone()])?;
            ret.push(headerhash[0]);

            // Store block
            //let _block = Block::new(headerhash[0], tx_hashes, block.m.clone());
            //self.blocks.insert(&[_block])?;
            let blk: Block = Block::from(block.clone());
            self.blocks.insert(&[blk])?;

            // Store block order
            self.order.insert(&[block.header.slot], &[headerhash[0]])?;

            // NOTE: The nullifiers and Merkle roots are applied in the state
            // transition apply function.
        }

        Ok(ret)
    }

    /// Check if the given [`BlockInfo`] is in the database and all trees.
    pub fn has_block(&self, block: &BlockInfo) -> Result<bool> {
        let blockhash = match self.order.get(&[block.header.slot], true) {
            Ok(v) => v[0].unwrap(),
            Err(_) => return Ok(false),
        };

        // TODO: Check if we have all transactions

        // Check provided info produces the same hash
        Ok(blockhash == block.header.headerhash())
    }

    /// Retrieve [`BlockInfo`]s by given hashes. Fails if any of them are not found.
    pub fn get_blocks_by_hash(&self, hashes: &[blake3::Hash]) -> Result<Vec<BlockInfo>> {
        let mut ret = Vec::with_capacity(hashes.len());

        let headers = self.headers.get(hashes, true)?;
        let blocks = self.blocks.get(hashes, true)?;

        for (i, header) in headers.iter().enumerate() {
            let header = header.clone().unwrap();
            let block = blocks[i].clone().unwrap();

            let txs = self.transactions.get(&block.txs, true)?;
            let txs = txs.iter().map(|x| x.clone().unwrap()).collect();

            let info = BlockInfo::new(header, txs, block.metadata.clone());
            ret.push(info);
        }

        Ok(ret)
    }

    /// Retrieve [`BlockInfo`]s by given slots. Does not fail if any of them are not found.
    pub fn get_blocks_by_slot(&self, slots: &[u64]) -> Result<Vec<BlockInfo>> {
        debug!("get_blocks_by_slot(): {:?}", slots);
        let blockhashes = self.order.get(slots, false)?;

        let mut hashes = vec![];
        for i in blockhashes.into_iter().flatten() {
            hashes.push(i);
        }

        self.get_blocks_by_hash(&hashes)
    }

    /// Retrieve n blocks after given start slot.
    pub fn get_blocks_after(&self, slot: u64, n: u64) -> Result<Vec<BlockInfo>> {
        debug!("get_blocks_after(): {} -> {}", slot, n);
        let hashes = self.order.get_after(slot, n)?;
        self.get_blocks_by_hash(&hashes)
    }

    /// Retrieve the last block slot and hash.
    pub fn last(&self) -> Result<(u64, blake3::Hash)> {
        self.order.get_last()
    }

    /// Retrieve last finalized block leader proof hash.
    pub fn get_last_proof_hash(&self) -> Result<blake3::Hash> {
        let (slot, _) = self.last().unwrap();
        let block = &self.get_blocks_by_slot(&[slot]).unwrap()[0];
        let hash = blake3::hash(&serialize(&block.metadata.proof));
        Ok(hash)
    }
}
