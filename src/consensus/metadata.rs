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

use darkfi_sdk::{
    crypto::{schnorr::Signature, Address, Keypair},
    pasta::pallas,
};
use darkfi_serial::{SerialDecodable, SerialEncodable};
use rand::rngs::OsRng;

use super::Participant;
use crate::{
    crypto::{
        lead_proof,
        leadcoin::LeadCoin,
        proof::{Proof, ProvingKey, VerifyingKey},
        types::*,
    },
    VerifyResult,
};

/// This struct represents [`Block`](super::Block) information used by the consensus protocol.
#[derive(Debug, Clone, PartialEq, Eq, SerialEncodable, SerialDecodable)]
pub struct Metadata {
    /// Block owner signature
    pub signature: Signature,
    /// Block owner address
    pub address: Address,
    /// Block owner slot competing coins public inputs
    pub public_inputs: Vec<pallas::Base>,
    /// Block owner winning coin index
    pub winning_index: usize,
    /// Response of global random oracle, or it's emulation.
    pub eta: [u8; 32],
    /// Leader NIZK proof
    pub proof: LeadProof,
    /// Nodes participating in the consensus process
    pub participants: Vec<Participant>,
}

impl Default for Metadata {
    fn default() -> Self {
        let keypair = Keypair::random(&mut OsRng);
        let address = Address::from(keypair.public);
        let signature = Signature::dummy();
        let public_inputs = vec![];
        let winning_index = 0;
        let eta: [u8; 32] = *blake3::hash(b"let there be dark!").as_bytes();
        let proof = LeadProof::default();
        let participants = vec![];
        Self { signature, address, public_inputs, winning_index, eta, proof, participants }
    }
}

impl Metadata {
    pub fn new(
        signature: Signature,
        address: Address,
        public_inputs: Vec<pallas::Base>,
        winning_index: usize,
        eta: [u8; 32],
        proof: LeadProof,
        participants: Vec<Participant>,
    ) -> Self {
        Self { signature, address, public_inputs, winning_index, eta, proof, participants }
    }
}

/// Wrapper over the Proof, for future additions.
#[derive(Default, Debug, Clone, PartialEq, Eq, SerialEncodable, SerialDecodable)]
pub struct LeadProof {
    /// Leadership proof
    pub proof: Proof,
}

impl LeadProof {
    pub fn new(pk: &ProvingKey, coin: LeadCoin) -> Self {
        let proof = lead_proof::create_lead_proof(pk, coin).unwrap();
        Self { proof }
    }

    pub fn verify(&self, vk: &VerifyingKey, public_inputs: &[DrkCircuitField]) -> VerifyResult<()> {
        lead_proof::verify_lead_proof(vk, &self.proof, public_inputs)
    }
}

impl From<Proof> for LeadProof {
    fn from(proof: Proof) -> Self {
        Self { proof }
    }
}
