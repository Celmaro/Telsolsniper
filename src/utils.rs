use crate::streaming::event_parser::protocols::pumpfun::PumpFunTradeEvent;
use crate::trading;
use crate::SolanaTrade;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;

impl SolanaTrade {
    #[inline]
    pub async fn get_sol_balance(&self, payer: &Pubkey) -> Result<u64, anyhow::Error> {
        trading::common::utils::get_sol_balance(&self.rpc, payer).await
    }

    #[inline]
    pub async fn get_payer_sol_balance(&self) -> Result<u64, anyhow::Error> {
        trading::common::utils::get_sol_balance(&self.rpc, &self.payer.pubkey()).await
    }

    #[inline]
    pub async fn get_token_balance(
        &self,
        payer: &Pubkey,
        mint: &Pubkey,
    ) -> Result<u64, anyhow::Error> {
        println!(
            "get_token_balance payer: {}, mint: {}, rpc_url: {}",
            payer, mint, self.trade_config.rpc_url
        );
        trading::common::utils::get_token_balance(&self.rpc, payer, mint).await
    }

    #[inline]
    pub async fn get_payer_token_balance(&self, mint: &Pubkey) -> Result<u64, anyhow::Error> {
        trading::common::utils::get_token_balance(&self.rpc, &self.payer.pubkey(), mint).await
    }

    #[inline]
    pub fn get_payer_pubkey(&self) -> Pubkey {
        self.payer.pubkey()
    }

    #[inline]
    pub fn get_payer(&self) -> &Keypair {
        self.payer.as_ref()
    }

    #[inline]
    pub async fn transfer_sol(
        &self,
        payer: &Keypair,
        receive_wallet: &Pubkey,
        amount: u64,
    ) -> Result<(), anyhow::Error> {
        trading::common::utils::transfer_sol(&self.rpc, payer, receive_wallet, amount).await
    }

    #[inline]
    pub async fn close_token_account(&self, mint: &Pubkey) -> Result<(), anyhow::Error> {
        trading::common::utils::close_token_account(&self.rpc, self.payer.as_ref(), mint).await
    }

    // -------------------------------- PumpFun --------------------------------

    #[inline]
    pub fn get_pumpfun_token_price(
        &self,
        virtual_sol_reserves: u64,
        virtual_token_reserves: u64,
    ) -> f64 {
        trading::pumpfun::common::get_token_price(virtual_sol_reserves, virtual_token_reserves)
    }

    #[inline]
    pub fn get_pumpfun_token_buy_price(&self, amount: u64, trade_info: &PumpFunTradeEvent) -> u64 {
        trading::pumpfun::common::get_buy_price(amount, trade_info)
    }

    #[inline]
    pub async fn get_pumpfun_token_current_price(
        &self,
        mint: &Pubkey,
    ) -> Result<f64, anyhow::Error> {
        let (bonding_curve, _) =
            trading::pumpfun::common::get_bonding_curve_account_v2(&self.rpc, mint).await?;

        let virtual_sol_reserves = bonding_curve.virtual_sol_reserves;
        let virtual_token_reserves = bonding_curve.virtual_token_reserves;

        Ok(trading::pumpfun::common::get_token_price(
            virtual_sol_reserves,
            virtual_token_reserves,
        ))
    }

    #[inline]
    pub async fn get_pumpfun_token_real_sol_reserves(
        &self,
        mint: &Pubkey,
    ) -> Result<u64, anyhow::Error> {
        let (bonding_curve, _) =
            trading::pumpfun::common::get_bonding_curve_account_v2(&self.rpc, mint).await?;

        let actual_sol_reserves = bonding_curve.real_sol_reserves;

        Ok(actual_sol_reserves)
    }

    #[inline]
    pub async fn get_pumpfun_token_creator(&self, mint: &Pubkey) -> Result<Pubkey, anyhow::Error> {
        let (bonding_curve, _) =
            trading::pumpfun::common::get_bonding_curve_account_v2(&self.rpc, mint).await?;

        let creator = bonding_curve.creator;

        Ok(creator)
    }

    // -------------------------------- PumpSwap --------------------------------

    #[inline]
    pub async fn get_pumpswap_token_current_price(
        &self,
        pool_address: &Pubkey,
    ) -> Result<f64, anyhow::Error> {
        let pool = trading::pumpswap::pool::Pool::fetch(&self.rpc, pool_address).await?;

        let (base_amount, quote_amount) = pool.get_token_balances(&self.rpc).await?;

        // Calculate price using constant product formula (x * y = k)
        // Price = quote_amount / base_amount
        if base_amount == 0 {
            return Err(anyhow::anyhow!(
                "Base amount is zero, cannot calculate price"
            ));
        }

        let price = quote_amount as f64 / base_amount as f64;

        Ok(price)
    }

    #[inline]
    pub async fn get_pumpswap_token_real_sol_reserves(
        &self,
        pool_address: &Pubkey,
    ) -> Result<u64, anyhow::Error> {
        let pool = trading::pumpswap::pool::Pool::fetch(&self.rpc, pool_address).await?;

        let (_, quote_amount) = pool.get_token_balances(&self.rpc).await?;

        Ok(quote_amount)
    }

    #[inline]
    pub async fn get_pumpswap_payer_token_balance(
        &self,
        pool_address: &Pubkey,
    ) -> Result<u64, anyhow::Error> {
        let pool = trading::pumpswap::pool::Pool::fetch(&self.rpc, pool_address).await?;

        let (base_amount, _) = pool.get_token_balances(&self.rpc).await?;

        Ok(base_amount)
    }
}
