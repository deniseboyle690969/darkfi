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

use std::{
    process::exit,
    sync::{mpsc::channel, Arc},
};

use clap::Parser;
use darkfi::util::cli::ProgressInc;
use darkfi_sdk::crypto::{ContractId, PublicKey, SecretKey, TokenId};
use rand::rngs::OsRng;
use rayon::prelude::*;

use darkfi::cli_desc;

#[derive(Parser)]
#[clap(name = "vanityaddr", about = cli_desc!(), version)]
#[clap(arg_required_else_help(true))]
struct Args {
    /// Prefixes to search
    prefix: Vec<String>,

    /// Should the search be case-sensitive
    #[clap(short)]
    case_sensitive: bool,

    /// Search for an Address
    #[clap(long)]
    address: bool,

    /// Search for a Token ID
    #[clap(long)]
    token_id: bool,

    /// Search for a Contract ID
    #[clap(long)]
    contract_id: bool,

    /// Number of threads to use (defaults to number of available CPUs)
    #[clap(short)]
    threads: Option<usize>,
}

struct DrkAddr {
    pub public: PublicKey,
    pub secret: SecretKey,
}

struct DrkToken {
    pub token_id: TokenId,
    pub secret: SecretKey,
}

struct DrkContract {
    pub contract_id: ContractId,
    pub secret: SecretKey,
}

trait Prefixable {
    fn new() -> Self;
    fn to_string(&self) -> String;
    fn get_secret(&self) -> SecretKey;

    fn starts_with(&self, prefix: &str, case_sensitive: bool) -> bool {
        if case_sensitive {
            self.to_string().starts_with(prefix)
        } else {
            self.to_string().to_lowercase().starts_with(prefix.to_lowercase().as_str())
        }
    }

    fn starts_with_any(&self, prefixes: &[String], case_sensitive: bool) -> bool {
        prefixes.iter().any(|prefix| self.starts_with(prefix, case_sensitive))
    }
}

impl Prefixable for DrkAddr {
    fn new() -> Self {
        let secret = SecretKey::random(&mut OsRng);
        let public = PublicKey::from_secret(secret);
        Self { public, secret }
    }

    fn to_string(&self) -> String {
        self.public.to_string()
    }

    fn get_secret(&self) -> SecretKey {
        self.secret
    }
}

impl Prefixable for DrkToken {
    fn new() -> Self {
        let secret = SecretKey::random(&mut OsRng);
        let token_id = TokenId::derive(secret);
        Self { token_id, secret }
    }

    fn to_string(&self) -> String {
        self.token_id.to_string()
    }

    fn get_secret(&self) -> SecretKey {
        self.secret
    }
}

impl Prefixable for DrkContract {
    fn new() -> Self {
        let secret = SecretKey::random(&mut OsRng);
        let contract_id = ContractId::derive(secret);
        Self { contract_id, secret }
    }

    fn to_string(&self) -> String {
        self.contract_id.to_string()
    }

    fn get_secret(&self) -> SecretKey {
        self.secret
    }
}

fn main() {
    let args = Args::parse();

    if !((args.address ^ args.contract_id ^ args.token_id) &&
        !(args.address && args.contract_id && args.token_id))
    {
        eprintln!("Error: Can only search for one of Address/ContractId/TokenId");
        exit(1);
    }

    if args.prefix.is_empty() {
        eprintln!("Error: No prefix given to search.");
        exit(1);
    }

    // Check if prefixes are valid base58
    for (idx, prefix) in args.prefix.iter().enumerate() {
        match bs58::decode(prefix).into_vec() {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error: Invalid base58 for prefix {}: {}", idx, e);
                exit(1);
            }
        };
    }

    // Threadpool
    let num_threads = if args.threads.is_some() {
        args.threads.unwrap()
    } else {
        std::thread::available_parallelism().unwrap().get()
    };
    let rayon_pool = rayon::ThreadPoolBuilder::new().num_threads(num_threads).build().unwrap();

    // Handle SIGINT
    let (tx, rx) = channel();
    ctrlc::set_handler(move || tx.send(()).expect("Could not send signal on channel"))
        .expect("Error setting SIGINT handler");

    // Something fancy
    let progress = Arc::new(ProgressInc::new());
    /*
    let progress = ProgressBar::new_spinner();
    let template =
        ProgressStyle::default_bar().template("[{elapsed_precise}] {pos} attempts").unwrap();
    progress.set_style(template);
    */

    // Fire off the threadpool
    let progress_ = progress.clone();
    rayon_pool.spawn(move || {
        if args.token_id {
            let tid = rayon::iter::repeat(DrkToken::new)
                .inspect(|_| progress_.inc(1))
                .map(|create| create())
                .find_any(|token_id| token_id.starts_with_any(&args.prefix, args.case_sensitive))
                .expect("Failed to find a token ID match");

            // The above will keep running until it finds a match or until the
            // program terminates. Only if a match is found shall the following
            // code be executed and the program exit successfully:
            let attempts = progress_.position();
            progress_.finish_and_clear();

            println!(
                "{{\"token_id\":\"{}\",\"attempts\":{},\"secret\":\"{}\"}}",
                tid.token_id, attempts, tid.secret,
            );
        } else if args.address {
            let addr = rayon::iter::repeat(DrkAddr::new)
                .inspect(|_| progress_.inc(1))
                .map(|create| create())
                .find_any(|address| address.starts_with_any(&args.prefix, args.case_sensitive))
                .expect("Failed to find an address match");

            let attempts = progress_.position();
            progress_.finish_and_clear();

            println!(
                "{{\"address\":\"{}\",\"attempts\":{},\"secret\":\"{}\"}}",
                addr.public, attempts, addr.secret,
            );
        } else if args.contract_id {
            let cid = rayon::iter::repeat(DrkContract::new)
                .inspect(|_| progress_.inc(1))
                .map(|create| create())
                .find_any(|contract_id| {
                    contract_id.starts_with_any(&args.prefix, args.case_sensitive)
                })
                .expect("Failed to find a contract ID match");

            let attempts = progress_.position();
            progress_.finish_and_clear();

            println!(
                "{{\"contract_id\":\"{}\",\"attempts\":{},\"secret\":\"{}\"}}",
                cid.contract_id, attempts, cid.secret,
            );
        }

        exit(0);
    });

    // This now blocks and lets our threadpool execute in the background.
    rx.recv().expect("Could not receive from channel");
    progress.finish_and_clear();
    eprintln!("\r\x1b[2KCaught SIGINT, exiting...");
    exit(127);
}
