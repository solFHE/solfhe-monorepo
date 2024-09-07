use anchor_lang::prelude::*;
use tfhe::prelude::*;
declare_id!("2thXcRahx1gAmXeMAWWy3B81WJcSEKbPYTrgfxCJBfhj");

#[program]
pub mod fhe_solana_ad {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let ad_account = &mut ctx.accounts.ad_account;
        ad_account.authority = *ctx.accounts.authority.key;
        Ok(())
    }

    pub fn store_encrypted_data(ctx: Context<StoreEncryptedData>, encrypted_data: Vec<u8>) -> Result<()> {
        let ad_account = &mut ctx.accounts.ad_account;
        ad_account.encrypted_data = encrypted_data;
        Ok(())
    }

    pub fn get_ad(ctx: Context<GetAd>) -> Result<()> {
        let ad_account = &ctx.accounts.ad_account;
        msg!("Encrypted data: {:?}", ad_account.encrypted_data);
        
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = authority, space = 8 + 32 + 1000)]
    pub ad_account: Account<'info, AdAccount>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct StoreEncryptedData<'info> {
    #[account(mut, has_one = authority)]
    pub ad_account: Account<'info, AdAccount>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct GetAd<'info> {
    #[account(mut)]
    pub ad_account: Account<'info, AdAccount>,
}

#[account]
pub struct AdAccount {
    pub authority: Pubkey,
    pub encrypted_data: Vec<u8>,
}
