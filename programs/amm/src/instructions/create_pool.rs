use crate::error::ErrorCode;
use crate::states::*;
use crate::util::create_token_vault_account;
use crate::{libraries::tick_math, util};
use anchor_lang::{prelude::*, solana_program};
use anchor_spl::token_interface::{Mint, TokenInterface};
// use solana_program::{program::invoke_signed, system_instruction};
#[derive(Accounts)]
pub struct CreatePool<'info> {
    /// Address paying to create the pool. Can be anyone
    #[account(mut)]
    pub pool_creator: Signer<'info>,

    /// Which config the pool belongs to.
    pub amm_config: Box<Account<'info, AmmConfig>>,

    /// Initialize an account to store the pool state
    #[account(
        init,
        seeds = [
            POOL_SEED.as_bytes(),
            amm_config.key().as_ref(),
            token_mint_0.key().as_ref(),
            token_mint_1.key().as_ref(),
        ],
        bump,
        payer = pool_creator,
        space = PoolState::LEN
    )]
    pub pool_state: AccountLoader<'info, PoolState>,

    /// Token_0 mint, the key must be smaller then token_1 mint.
    #[account(
        constraint = token_mint_0.key() < token_mint_1.key(),
        mint::token_program = token_program_0
    )]
    pub token_mint_0: Box<InterfaceAccount<'info, Mint>>,

    /// Token_1 mint
    #[account(
        mint::token_program = token_program_1
    )]
    pub token_mint_1: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: Token_0 vault for the pool, initialized in contract
    #[account(
        mut,
        seeds =[
            POOL_VAULT_SEED.as_bytes(),
            pool_state.key().as_ref(),
            token_mint_0.key().as_ref(),
        ],
        bump,
    )]
    pub token_vault_0: UncheckedAccount<'info>,

    /// CHECK: Token_1 vault for the pool, initialized in contract
    #[account(
        mut,
        seeds =[
            POOL_VAULT_SEED.as_bytes(),
            pool_state.key().as_ref(),
            token_mint_1.key().as_ref(),
        ],
        bump,
    )]
    pub token_vault_1: UncheckedAccount<'info>,

    /// Initialize an account to store oracle observations
    #[account(
        init,
        seeds = [
            OBSERVATION_SEED.as_bytes(),
            pool_state.key().as_ref(),
        ],
        bump,
        payer = pool_creator,
        space = ObservationState::LEN
    )]
    pub observation_state: AccountLoader<'info, ObservationState>,

    /// Initialize an account to store if a tick array is initialized.
    #[account(
        init,
        seeds = [
            POOL_TICK_ARRAY_BITMAP_SEED.as_bytes(),
            pool_state.key().as_ref(),
        ],
        bump,
        payer = pool_creator,
        space = TickArrayBitmapExtension::LEN
    )]
    pub tick_array_bitmap: AccountLoader<'info, TickArrayBitmapExtension>,

    /// Spl token program or token program 2022
    pub token_program_0: Interface<'info, TokenInterface>,
    /// Spl token program or token program 2022
    pub token_program_1: Interface<'info, TokenInterface>,
    /// To create a new program account
    pub system_program: Program<'info, System>,
    /// Sysvar for program account
    pub rent: Sysvar<'info, Rent>,
    // remaining account
    // #[account(
    //     seeds = [
    //     SUPPORT_MINT_SEED.as_bytes(),
    //     token_mint_0.key().as_ref(),
    // ],
    //     bump
    // )]
    // pub support_mint0_associated: Account<'info, SupportMintAssociated>,

    // #[account(
    //     seeds = [
    //     SUPPORT_MINT_SEED.as_bytes(),
    //     token_mint_1.key().as_ref(),
    // ],
    //     bump
    // )]
    // pub support_mint1_associated: Account<'info, SupportMintAssociated>,
}

pub fn create_pool(ctx: Context<CreatePool>, sqrt_price_x64: u128, open_time: u64) -> Result<()> {
    let mint0_associated_is_initialized = util::support_mint_associated_is_initialized(
        &ctx.remaining_accounts,
        &ctx.accounts.token_mint_0,
    )?;
    let mint1_associated_is_initialized = util::support_mint_associated_is_initialized(
        &ctx.remaining_accounts,
        &ctx.accounts.token_mint_1,
    )?;
    if !(util::is_supported_mint(&ctx.accounts.token_mint_0, mint0_associated_is_initialized)
        .unwrap()
        && util::is_supported_mint(&ctx.accounts.token_mint_1, mint1_associated_is_initialized)
            .unwrap())
    {
        return err!(ErrorCode::NotSupportMint);
    }
    let block_timestamp = solana_program::clock::Clock::get()?.unix_timestamp;
    require_gt!(block_timestamp as u64, open_time);
    let pool_id = ctx.accounts.pool_state.key();
    let mut pool_state = ctx.accounts.pool_state.load_init()?;

    let tick = tick_math::get_tick_at_sqrt_price(sqrt_price_x64)?;
    #[cfg(feature = "enable-log")]
    msg!(
        "create pool, init_price: {}, init_tick:{}",
        sqrt_price_x64,
        tick
    );

    // init token vault accounts
    create_token_vault_account(
        &ctx.accounts.pool_creator,
        &ctx.accounts.pool_state.to_account_info(),
        &ctx.accounts.token_vault_0,
        &ctx.accounts.token_mint_0,
        &ctx.accounts.system_program,
        &ctx.accounts.token_program_0,
        &[
            POOL_VAULT_SEED.as_bytes(),
            ctx.accounts.pool_state.key().as_ref(),
            ctx.accounts.token_mint_0.key().as_ref(),
            &[ctx.bumps.token_vault_0][..],
        ],
    )?;

    create_token_vault_account(
        &ctx.accounts.pool_creator,
        &ctx.accounts.pool_state.to_account_info(),
        &ctx.accounts.token_vault_1,
        &ctx.accounts.token_mint_1,
        &ctx.accounts.system_program,
        &ctx.accounts.token_program_1,
        &[
            POOL_VAULT_SEED.as_bytes(),
            ctx.accounts.pool_state.key().as_ref(),
            ctx.accounts.token_mint_1.key().as_ref(),
            &[ctx.bumps.token_vault_1][..],
        ],
    )?;

    // init observation
    ctx.accounts
        .observation_state
        .load_init()?
        .initialize(pool_id)?;

    let bump = ctx.bumps.pool_state;
    pool_state.initialize(
        bump,
        sqrt_price_x64,
        block_timestamp,
        tick,
        ctx.accounts.pool_creator.key(),
        ctx.accounts.token_vault_0.key(),
        ctx.accounts.token_vault_1.key(),
        ctx.accounts.amm_config.as_ref(),
        ctx.accounts.token_mint_0.as_ref(),
        ctx.accounts.token_mint_1.as_ref(),
        ctx.accounts.observation_state.key(),
    )?;

    ctx.accounts
        .tick_array_bitmap
        .load_init()?
        .initialize(pool_id);

    emit!(PoolCreatedEvent {
        token_mint_0: ctx.accounts.token_mint_0.key(),
        token_mint_1: ctx.accounts.token_mint_1.key(),
        tick_spacing: ctx.accounts.amm_config.tick_spacing,
        pool_state: ctx.accounts.pool_state.key(),
        sqrt_price_x64,
        tick,
        token_vault_0: ctx.accounts.token_vault_0.key(),
        token_vault_1: ctx.accounts.token_vault_1.key(),
    });
    Ok(())
}
