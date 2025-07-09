pub mod pumpfun;
pub mod pumpswap;
pub mod bonk;
pub mod swqos;

pub mod trade_type {
  pub const COPY_BUY: &'static str = "copy_buy";
  pub const COPY_SELL: &'static str = "copy_sell";
  pub const SNIPER_BUY: &'static str = "sniper_buy";
  pub const SNIPER_SELL: &'static str = "sniper_sell";
}

pub mod trade_platform {
  pub const PUMPFUN: &'static str = "pumpfun";
  pub const PUMPFUN_SWAP: &'static str = "pumpswap";
  pub const BONK: &'static str = "bonk";
}