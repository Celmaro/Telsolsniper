use solana_sdk::{
    pubkey::Pubkey, signature::Keypair, signer::Signer, system_instruction,
    transaction::Transaction,
};
use spl_associated_token_account::get_associated_token_address;
use spl_token::instruction::close_account;

use crate::common::SolanaRpcClient;
use anyhow::anyhow;

#[inline]
pub async fn get_token_balance(
    rpc: &SolanaRpcClient,
    payer: &Pubkey,
    mint: &Pubkey,
) -> Result<u64, anyhow::Error> {
    println!("payer: {:?}", payer);
    println!("mint: {:?}", mint);
    let ata = get_associated_token_address(payer, mint);
    let balance = rpc.get_token_account_balance(&ata).await?;
    let balance_u64 = balance
        .amount
        .parse::<u64>()
        .map_err(|_| anyhow!("Failed to parse token balance"))?;
    Ok(balance_u64)
}

#[inline]
pub async fn get_sol_balance(
    rpc: &SolanaRpcClient,
    account: &Pubkey,
) -> Result<u64, anyhow::Error> {
    let balance = rpc.get_balance(account).await?;
    Ok(balance)
}

// Calculate slippage for buy operations
#[inline]
pub fn calculate_with_slippage_buy(amount: u64, basis_points: u64) -> u64 {
    amount + (amount * basis_points / 10000)
}

// Calculate slippage for sell operations
#[inline]
pub fn calculate_with_slippage_sell(amount: u64, basis_points: u64) -> u64 {
    if amount <= basis_points / 10000 {
        1
    } else {
        amount - (amount * basis_points / 10000)
    }
}

pub async fn transfer_sol(
    rpc: &SolanaRpcClient,
    payer: &Keypair,
    receive_wallet: &Pubkey,
    amount: u64,
) -> Result<(), anyhow::Error> {
    if amount == 0 {
        return Err(anyhow!("transfer_sol: Amount cannot be zero"));
    }

    let balance = get_sol_balance(rpc, &payer.pubkey()).await?;
    if balance < amount {
        return Err(anyhow!("Insufficient balance"));
    }

    let transfer_instruction =
        system_instruction::transfer(&payer.pubkey(), receive_wallet, amount);

    let recent_blockhash = rpc.get_latest_blockhash().await?;

    let transaction = Transaction::new_signed_with_payer(
        &[transfer_instruction],
        Some(&payer.pubkey()),
        &[payer],
        recent_blockhash,
    );

    rpc.send_and_confirm_transaction(&transaction).await?;

    Ok(())
}

/// 关闭代币账户
///
/// 此函数用于关闭指定代币的关联代币账户，将账户中的代币余额转移给账户所有者。
///
/// # 参数
///
/// * `rpc` - Solana RPC客户端
/// * `payer` - 支付交易费用的账户
/// * `mint` - 代币的Mint地址
///
/// # 返回值
///
/// 返回一个Result，成功时返回()，失败时返回错误
pub async fn close_token_account(
    rpc: &SolanaRpcClient,
    payer: &Keypair,
    mint: &Pubkey,
) -> Result<(), anyhow::Error> {
    // 获取关联代币账户地址
    let ata = get_associated_token_address(&payer.pubkey(), mint);

    // 检查账户是否存在
    let account_exists = rpc.get_account(&ata).await.is_ok();
    if !account_exists {
        return Ok(()); // 如果账户不存在，直接返回成功
    }

    // 构建关闭账户指令
    let close_account_ix = close_account(
        &spl_token::ID,
        &ata,
        &payer.pubkey(),
        &payer.pubkey(),
        &[&payer.pubkey()],
    )?;

    // 构建交易
    let recent_blockhash = rpc.get_latest_blockhash().await?;
    let transaction = Transaction::new_signed_with_payer(
        &[close_account_ix],
        Some(&payer.pubkey()),
        &[payer],
        recent_blockhash,
    );

    // 发送交易
    rpc.send_and_confirm_transaction(&transaction).await?;

    Ok(())
}
