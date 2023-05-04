/* This file is part of DarkFi (https://dark.fi)
 *
 * Copyright (C) 2020-2023 Dyne.org foundation
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
use std::collections::HashMap;

use darkfi::{
    consensus::{
        ValidatorState, ValidatorStatePtr, TESTNET_BOOTSTRAP_TIMESTAMP, TESTNET_GENESIS_HASH_BYTES,
        TESTNET_GENESIS_TIMESTAMP, TESTNET_INITIAL_DISTRIBUTION,
    },
    tx::Transaction,
    wallet::{WalletDb, WalletPtr},
    zk::{empty_witnesses, halo2::Field, ProvingKey, ZkCircuit},
    zkas::ZkBinary,
    Result,
};
use darkfi_money_contract::client::OwnCoin;
use darkfi_sdk::{
    crypto::{
        Keypair, MerkleTree, PublicKey, CONSENSUS_CONTRACT_ID, DARK_TOKEN_ID, MONEY_CONTRACT_ID,
    },
    db::SMART_CONTRACT_ZKAS_DB_NAME,
    pasta::pallas,
    ContractCall,
};
use darkfi_serial::{serialize, Encodable};
use log::warn;
use rand::rngs::OsRng;

use darkfi_money_contract::{
    client::transfer_v1::TransferCallBuilder, model::MoneyTransferParamsV1, MoneyFunction,
    MONEY_CONTRACT_ZKAS_BURN_NS_V1, MONEY_CONTRACT_ZKAS_MINT_NS_V1,
};

pub fn init_logger() {
    let mut cfg = simplelog::ConfigBuilder::new();
    cfg.add_filter_ignore("sled".to_string());
    cfg.add_filter_ignore("blockchain::contractstore".to_string());
    // We check this error so we can execute same file tests in parallel,
    // otherwise second one fails to init logger here.
    if let Err(_) = simplelog::TermLogger::init(
        //simplelog::LevelFilter::Info,
        simplelog::LevelFilter::Debug,
        //simplelog::LevelFilter::Trace,
        cfg.build(),
        simplelog::TerminalMode::Mixed,
        simplelog::ColorChoice::Auto,
    ) {
        warn!(target: "money_harness", "Logger already initialized");
    }
}

pub struct Wallet {
    pub keypair: Keypair,
    pub state: ValidatorStatePtr,
    pub merkle_tree: MerkleTree,
    pub consensus_merkle_tree: MerkleTree,
    pub wallet: WalletPtr,
    pub coins: Vec<OwnCoin>,
    pub spent_coins: Vec<OwnCoin>,
}

impl Wallet {
    async fn new(keypair: Keypair, faucet_pubkeys: &[PublicKey]) -> Result<Self> {
        let wallet = WalletDb::new("sqlite::memory:", "foo").await?;
        let sled_db = sled::Config::new().temporary(true).open()?;

        let state = ValidatorState::new(
            &sled_db,
            *TESTNET_BOOTSTRAP_TIMESTAMP,
            *TESTNET_GENESIS_TIMESTAMP,
            *TESTNET_GENESIS_HASH_BYTES,
            *TESTNET_INITIAL_DISTRIBUTION,
            wallet.clone(),
            faucet_pubkeys.to_vec(),
            false,
            false,
        )
        .await?;

        let merkle_tree = MerkleTree::new(100);
        let consensus_merkle_tree = MerkleTree::new(100);

        let coins = vec![];
        let spent_coins = vec![];

        Ok(Self { keypair, state, merkle_tree, consensus_merkle_tree, wallet, coins, spent_coins })
    }
}

pub struct ConsensusTestHarness {
    pub faucet: Wallet,
    pub alice: Wallet,
    pub proving_keys: HashMap<&'static str, (ProvingKey, ZkBinary)>,
}

impl ConsensusTestHarness {
    pub async fn new() -> Result<Self> {
        let faucet_kp = Keypair::random(&mut OsRng);
        let faucet_pubkeys = vec![faucet_kp.public];
        let faucet = Wallet::new(faucet_kp, &faucet_pubkeys).await?;

        let alice_kp = Keypair::random(&mut OsRng);
        let alice = Wallet::new(alice_kp, &faucet_pubkeys).await?;

        // Get the zkas circuits and build proving keys
        let mut proving_keys = HashMap::new();
        let alice_sled = alice.state.read().await.blockchain.sled_db.clone();
        let mut db_handle = alice.state.read().await.blockchain.contracts.lookup(
            &alice_sled,
            &MONEY_CONTRACT_ID,
            SMART_CONTRACT_ZKAS_DB_NAME,
        )?;

        macro_rules! mkpk {
            ($ns:expr) => {
                let zkbin = db_handle.get(&serialize(&$ns))?.unwrap();
                let zkbin = ZkBinary::decode(&zkbin)?;
                let witnesses = empty_witnesses(&zkbin);
                let circuit = ZkCircuit::new(witnesses, zkbin.clone());
                let pk = ProvingKey::build(13, &circuit);
                proving_keys.insert($ns, (pk, zkbin));
            };
        }

        mkpk!(MONEY_CONTRACT_ZKAS_MINT_NS_V1);
        mkpk!(MONEY_CONTRACT_ZKAS_BURN_NS_V1);

        db_handle = alice.state.read().await.blockchain.contracts.lookup(
            &alice_sled,
            &CONSENSUS_CONTRACT_ID,
            SMART_CONTRACT_ZKAS_DB_NAME,
        )?;
        mkpk!(MONEY_CONTRACT_ZKAS_MINT_NS_V1);
        mkpk!(MONEY_CONTRACT_ZKAS_BURN_NS_V1);

        Ok(Self { faucet, alice, proving_keys })
    }

    pub fn airdrop_native(
        &self,
        value: u64,
        recipient: PublicKey,
    ) -> Result<(Transaction, MoneyTransferParamsV1)> {
        let (mint_pk, mint_zkbin) = self.proving_keys.get(&MONEY_CONTRACT_ZKAS_MINT_NS_V1).unwrap();
        let (burn_pk, burn_zkbin) = self.proving_keys.get(&MONEY_CONTRACT_ZKAS_BURN_NS_V1).unwrap();

        let builder = TransferCallBuilder {
            keypair: self.faucet.keypair,
            recipient,
            value,
            token_id: *DARK_TOKEN_ID,
            rcpt_spend_hook: pallas::Base::zero(),
            rcpt_user_data: pallas::Base::zero(),
            rcpt_user_data_blind: pallas::Base::random(&mut OsRng),
            change_spend_hook: pallas::Base::zero(),
            change_user_data: pallas::Base::zero(),
            change_user_data_blind: pallas::Base::random(&mut OsRng),
            coins: vec![],
            tree: self.faucet.merkle_tree.clone(),
            mint_zkbin: mint_zkbin.clone(),
            mint_pk: mint_pk.clone(),
            burn_zkbin: burn_zkbin.clone(),
            burn_pk: burn_pk.clone(),
            clear_input: true,
        };

        let debris = builder.build()?;

        let mut data = vec![MoneyFunction::TransferV1 as u8];
        debris.params.encode(&mut data)?;
        let calls = vec![ContractCall { contract_id: *MONEY_CONTRACT_ID, data }];
        let proofs = vec![debris.proofs];
        let mut tx = Transaction { calls, proofs, signatures: vec![] };
        let sigs = tx.create_sigs(&mut OsRng, &debris.signature_secrets)?;
        tx.signatures = vec![sigs];

        Ok((tx, debris.params))
    }
}
