use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, Mint, MintTo, Token, TokenAccount};

declare_id!("3J9w5Aof5M4CZtWCmQpEVAQTT1Z26AViYR8mxMnwvdq6");

#[program]
pub mod ksha {
    use super::*;

    // ─────────────────────────────────────────────────────────
    // ONBOARDING — stub, fill in later
    // ─────────────────────────────────────────────────────────
    //
    // pub fn register_plant(...) -> Result<()> { ... }
    // pub fn submit_verification(...) -> Result<()> { ... }

    // ─────────────────────────────────────────────────────────
    // TOKEN INSTRUCTIONS — single global mint, MVP version
    // ─────────────────────────────────────────────────────────

    /// Runs ONCE for the whole platform. Creates the one and only
    /// KSHA-CARBON mint and sets its authority to a program-derived
    /// address (the "platform_authority" PDA) that only this program
    /// controls. Unlike the per-batch design, this authority is never
    /// revoked — it has to stay live so future batches can mint.
    ///
    /// Integrity in this model lives entirely in program logic (the
    /// BatchAccount checks below), not in an on-chain revoked-authority
    /// guarantee. Acceptable for MVP since you control the only signer
    /// path into mint_batch; revisit before trusting this with buyers
    /// who can't audit your program logic themselves.
    pub fn init_platform(ctx: Context<InitPlatform>) -> Result<()> {
        let platform = &mut ctx.accounts.platform_state;
        platform.mint = ctx.accounts.mint.key();
        platform.authority_bump = ctx.bumps.platform_authority;
        platform.bump = ctx.bumps.platform_state;
        Ok(())
    }

    /// Creates the on-chain record for a verified batch, before any
    /// tokens exist. In your real flow this would likely be called
    /// from inside the onboarding/verification instruction once you
    /// build it — kept separate here since onboarding is stubbed.
    pub fn create_batch(
        ctx: Context<CreateBatch>,
        csi_project_id: String,
        verified_amount: u64,
    ) -> Result<()> {
        require!(csi_project_id.len() <= 32, ErrorCode::ProjectIdTooLong);
        require!(verified_amount > 0, ErrorCode::ZeroAmount);

        let batch = &mut ctx.accounts.batch_account;
        batch.owner = ctx.accounts.owner.key();
        batch.csi_project_id = csi_project_id;
        batch.verified_amount = verified_amount;
        batch.minted_amount = 0;
        batch.retired_amount = 0;
        batch.minted = false;
        batch.bump = ctx.bumps.batch_account;

        Ok(())
    }

    /// Mints tokens 1:1 against a verified batch, into the ONE global
    /// mint set up in init_platform. The cap on this batch is enforced
    /// here in program logic (minted flag + amount match) rather than
    /// by revoking mint authority, since authority must stay live for
    /// future batches.
    pub fn mint_batch(ctx: Context<MintBatch>, amount: u64) -> Result<()> {
        let batch = &mut ctx.accounts.batch_account;

        require!(!batch.minted, ErrorCode::AlreadyMinted);
        require!(amount == batch.verified_amount, ErrorCode::AmountMismatch);
        require_keys_eq!(
            ctx.accounts.mint.key(),
            ctx.accounts.platform_state.mint,
            ErrorCode::MintMismatch
        );

        let bump = ctx.accounts.platform_state.authority_bump;
        let seeds = &[b"platform_authority".as_ref(), &[bump]];
        let signer_seeds = &[&seeds[..]];

        // CPI into SPL Token: mint_to
        // (Anchor 1.0.0: CpiContext no longer takes the program AccountInfo —
        // pass the program ID directly via Token::id())
        token::mint_to(
            CpiContext::new_with_signer(
                Token::id(),
                MintTo {
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.owner_token_account.to_account_info(),
                    authority: ctx.accounts.platform_authority.to_account_info(),
                },
                signer_seeds,
            ),
            amount,
        )?;

        batch.minted = true;
        batch.minted_amount = amount;

        Ok(())
    }

    /// Burns tokens when a buyer retires them against an offset claim,
    /// and records the retirement on-chain so a certificate can be
    /// generated from this account later.
    pub fn retire_batch(ctx: Context<RetireBatch>, amount: u64) -> Result<()> {
        let batch = &mut ctx.accounts.batch_account;

        require!(
            batch.retired_amount.checked_add(amount).unwrap() <= batch.minted_amount,
            ErrorCode::RetireExceedsMinted
        );

        token::burn(
            CpiContext::new(
                Token::id(),
                Burn {
                    mint: ctx.accounts.mint.to_account_info(),
                    from: ctx.accounts.holder_token_account.to_account_info(),
                    authority: ctx.accounts.holder.to_account_info(),
                },
            ),
            amount,
        )?;

        batch.retired_amount = batch.retired_amount.checked_add(amount).unwrap();

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────
// ACCOUNT STATE
// ─────────────────────────────────────────────────────────

/// Platform-wide singleton. Exactly one of these ever exists.
/// Records the one global mint and the bump for the authority PDA
/// so mint_batch can re-derive the signer seeds without re-deriving
/// from scratch each time.
#[account]
pub struct PlatformState {
    pub mint: Pubkey,
    pub authority_bump: u8,
    pub bump: u8,
}

impl PlatformState {
    // discriminator(8) + mint(32) + authority_bump(1) + bump(1)
    pub const LEN: usize = 8 + 32 + 1 + 1;
}

#[account]
pub struct BatchAccount {
    pub owner: Pubkey,            // plant owner's wallet
    pub csi_project_id: String,   // e.g. "GCSP1084" — max 32 chars
    pub verified_amount: u64,     // tonnes CO2e verified by CSI
    pub minted_amount: u64,       // should equal verified_amount once minted
    pub retired_amount: u64,      // running total burned/retired
    pub minted: bool,
    pub bump: u8,
}

impl BatchAccount {
    // discriminator(8) + owner(32) + csi_project_id(4+32) + verified(8)
    // + minted_amount(8) + retired_amount(8) + minted(1) + bump(1)
    pub const LEN: usize = 8 + 32 + (4 + 32) + 8 + 8 + 8 + 1 + 1;
}

// ─────────────────────────────────────────────────────────
// CONTEXTS
// ─────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct InitPlatform<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        init,
        payer = payer,
        space = PlatformState::LEN,
        seeds = [b"platform_state"],
        bump
    )]
    pub platform_state: Account<'info, PlatformState>,

    /// CHECK: PDA, signer-only via seeds, never read as data.
    #[account(
        seeds = [b"platform_authority"],
        bump
    )]
    pub platform_authority: UncheckedAccount<'info>,

    /// The single global mint, created here, once, for the whole
    /// platform. Authority is set to the platform_authority PDA at
    /// creation time via the mint::authority constraint below.
    #[account(
        init,
        payer = payer,
        mint::decimals = 0,
        mint::authority = platform_authority,
    )]
    pub mint: Account<'info, Mint>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(csi_project_id: String, verified_amount: u64)]
pub struct CreateBatch<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        init,
        payer = owner,
        space = BatchAccount::LEN,
        seeds = [b"batch", owner.key().as_ref(), csi_project_id.as_bytes()],
        bump
    )]
    pub batch_account: Account<'info, BatchAccount>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct MintBatch<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [b"platform_state"],
        bump = platform_state.bump
    )]
    pub platform_state: Account<'info, PlatformState>,

    /// CHECK: PDA, signer-only via seeds, never read as data.
    #[account(
        seeds = [b"platform_authority"],
        bump = platform_state.authority_bump
    )]
    pub platform_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [b"batch", batch_account.owner.as_ref(), batch_account.csi_project_id.as_bytes()],
        bump = batch_account.bump
    )]
    pub batch_account: Account<'info, BatchAccount>,

    #[account(mut)]
    pub mint: Account<'info, Mint>,

    #[account(mut)]
    pub owner_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct RetireBatch<'info> {
    #[account(mut)]
    pub holder: Signer<'info>,

    #[account(
        mut,
        seeds = [b"batch", batch_account.owner.as_ref(), batch_account.csi_project_id.as_bytes()],
        bump = batch_account.bump
    )]
    pub batch_account: Account<'info, BatchAccount>,

    #[account(mut)]
    pub mint: Account<'info, Mint>,

    #[account(mut)]
    pub holder_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

// ─────────────────────────────────────────────────────────
// ERRORS
// ─────────────────────────────────────────────────────────

#[error_code]
pub enum ErrorCode {
    #[msg("CSI project ID exceeds 32 characters")]
    ProjectIdTooLong,
    #[msg("Amount must be greater than zero")]
    ZeroAmount,
    #[msg("This batch has already been minted")]
    AlreadyMinted,
    #[msg("Mint amount does not match verified amount")]
    AmountMismatch,
    #[msg("Mint account does not match the platform's global mint")]
    MintMismatch,
    #[msg("Retire amount would exceed total minted amount")]
    RetireExceedsMinted,
}