use anchor_lang::prelude::*;
use tfhe::prelude::*;
declare_id!("2thXcRahx1gAmXeMAWWy3B81WJcSEKbPYTrgfxCJBfhj");

#[program]
pub mod solfhe {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
