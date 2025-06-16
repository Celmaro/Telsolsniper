//! Constants used by the crate.
//!
//! This module contains various constants used throughout the crate, including:
//!
//! - Seeds for deriving Program Derived Addresses (PDAs)
//! - Program account addresses and public keys
//!
//! The constants are organized into submodules for better organization:
//!
//! - `seeds`: Contains seed values used for PDA derivation
//! - `accounts`: Contains important program account addresses

/// Constants used as seeds for deriving PDAs (Program Derived Addresses)
pub mod seeds {
    /// Seed for the global state PDA
    pub const GLOBAL_SEED: &[u8] = b"global";
}

/// Constants related to program accounts and authorities
pub mod accounts {
    use solana_sdk::{pubkey, pubkey::Pubkey};

    pub const JITO_TIP_ACCOUNTS: &[&str] = &[
        "96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5",
        "HFqU5x63VTqvQss8hp11i4wVV8bD44PvwucfZ2bU7gRe",
        "Cw8CFyM9FkoMi7K7Crf6HNQqf4uEMzpKw6QNghXLvLkY",
        "ADaUMid9yfUytqMBgopwjb2DTLSokTSzL1zt6iGPaS49",
        "DfXygSm4jCyNCybVYYK6DwvWqjKee8pbDmJGcLWNDXjh",
        "ADuUkR4vqLUMWXxW9gh6D6L8pMSawimctcNZ5pGwDcEt",
        "DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL",
        "3AVi9Tg9Uo68tJfuvoKvqKNWKkC5wPdSSdeBnizKZ6jT",
    ];

    /// Tip accounts
    pub const NEXTBLOCK_TIP_ACCOUNTS: &[&str] = &[
        "NextbLoCkVtMGcV47JzewQdvBpLqT9TxQFozQkN98pE",
        "NexTbLoCkWykbLuB1NkjXgFWkX9oAtcoagQegygXXA2",
        "NeXTBLoCKs9F1y5PJS9CKrFNNLU1keHW71rfh7KgA1X",
        "NexTBLockJYZ7QD7p2byrUa6df8ndV2WSd8GkbWqfbb",
        "neXtBLock1LeC67jYd1QdAa32kbVeubsfPNTJC1V5At",
        "nEXTBLockYgngeRmRrjDV31mGSekVPqZoMGhQEZtPVG",
        "NEXTbLoCkB51HpLBLojQfpyVAMorm3zzKg7w9NFdqid",
        "nextBLoCkPMgmG8ZgJtABeScP35qLa2AMCNKntAP7Xc",
    ];

    pub const ZEROSLOT_TIP_ACCOUNTS: &[&str] = &[
        "Eb2KpSC8uMt9GmzyAEm5Eb1AAAgTjRaXWFjKyFXHZxF3",
        "FCjUJZ1qozm1e8romw216qyfQMaaWKxWsuySnumVCCNe",
        "ENxTEjSQ1YabmUpXAdCgevnHQ9MHdLv8tzFiuiYJqa13",
        "6rYLG55Q9RpsPGvqdPNJs4z5WTxJVatMB8zV3WJhs5EK",
        "Cix2bHfqPcKcM233mzxbLk14kSggUUiz2A87fJtGivXr",
    ];

    pub const NOZOMI_TIP_ACCOUNTS: &[&str] = &[
        "TEMPaMeCRFAS9EKF53Jd6KpHxgL47uWLcpFArU1Fanq",
        "noz3jAjPiHuBPqiSPkkugaJDkJscPuRhYnSpbi8UvC4",
        "noz3str9KXfpKknefHji8L1mPgimezaiUyCHYMDv1GE",
        "noz6uoYCDijhu1V7cutCpwxNiSovEwLdRHPwmgCGDNo",
        "noz9EPNcT7WH6Sou3sr3GGjHQYVkN3DNirpbvDkv9YJ",
        "nozc5yT15LazbLTFVZzoNZCwjh3yUtW86LoUyqsBu4L",
        "nozFrhfnNGoyqwVuwPAW4aaGqempx4PU6g6D9CJMv7Z",
        "nozievPk7HyK1Rqy1MPJwVQ7qQg2QoJGyP71oeDwbsu",
        "noznbgwYnBLDHu8wcQVCEw6kDrXkPdKkydGJGNXGvL7",
        "nozNVWs5N8mgzuD3qigrCG2UoKxZttxzZ85pvAQVrbP",
        "nozpEGbwx4BcGp6pvEdAh1JoC2CQGZdU6HbNP1v2p6P",
        "nozrhjhkCr3zXT3BiT4WCodYCUFeQvcdUkM7MqhKqge",
        "nozrwQtWhEdrA6W8dkbt9gnUaMs52PdAv5byipnadq3",
        "nozUacTVWub3cL4mJmGCYjKZTnE9RbdY5AP46iQgbPJ",
        "nozWCyTPppJjRuw2fpzDhhWbW355fzosWSzrrMYB1Qk",
        "nozWNju6dY353eMkMqURqwQEoM3SFgEKC6psLCSfUne",
        "nozxNBgWohjR75vdspfxR5H9ceC7XXH99xpxhVGt3Bb",
    ];

    pub const AMMV4_PROGRAM: Pubkey = pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");
    pub const CPMM_PROGRAM: Pubkey = pubkey!("CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C");
}

pub mod trade {
    pub const TRADER_TIP_AMOUNT: u64 = 100000; // 0.0001 SOL in lamports
    pub const DEFAULT_SLIPPAGE: u64 = 1000; // 10%
    pub const DEFAULT_COMPUTE_UNIT_LIMIT: u32 = 78000;
    pub const DEFAULT_COMPUTE_UNIT_PRICE: u64 = 500000;
    pub const DEFAULT_BUY_TIP_FEE: u64 = 600000; // 0.0006 SOL in lamports
    pub const DEFAULT_SELL_TIP_FEE: u64 = 100000; // 0.0001 SOL in lamports
}
