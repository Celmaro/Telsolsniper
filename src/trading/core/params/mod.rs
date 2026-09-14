//! DEX protocol parameter types and [`SwapParams`].

mod bonk;
mod dex_swap;
mod meteora_damm_v2;
mod pumpfun;
mod pumpswap;
mod raydium_amm_v4;
mod raydium_cpmm;

pub use bonk::{BonkParams, LaunchLabParams, StonkFunParams};
pub use dex_swap::{DexParamEnum, SenderConcurrencyConfig, SwapParams};
pub use meteora_damm_v2::MeteoraDammV2Params;
pub use pumpfun::PumpFunParams;
pub use pumpswap::PumpSwapParams;
pub use raydium_amm_v4::RaydiumAmmV4Params;
pub use raydium_cpmm::{RaydiumCpmmParams, TokenTransferFee};
/// User-facing parameters for a graduated StonkFun pool on the external CPMM venue.
pub type StonkFunSwapParams = RaydiumCpmmParams;
