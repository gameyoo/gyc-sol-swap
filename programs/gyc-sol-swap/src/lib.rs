use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    program::{
        invoke_signed, 
    },
    system_instruction::{
        transfer,
        
    },
    sysvar::{
        rent::Rent
    },
    program_option::COption,


};
use anchor_spl::{
    associated_token::{ AssociatedToken, },
    token::{self, Mint, Token, TokenAccount, Transfer},
};

declare_id!("91cSWaofsRKHtbLM93Qdg2ELMnSaqPzMhk1wu1Jaxpvb");

#[program]
pub mod gyc_sol_swap {
    use super::*;
    pub fn initialize(
        ctx: Context<Initialize>,
        config_nonce: u8,
        vault_nonce: u8,
        authority: Pubkey,
    ) -> ProgramResult {

        ctx.accounts.config.initializer = *ctx.accounts.signer.to_account_info().key;
        ctx.accounts.config.authority = authority;
        ctx.accounts.config.sol_vault = *ctx.accounts.sol_vault.key;
        ctx.accounts.config.token_vault = *ctx.accounts.token_vault.to_account_info().key;
        ctx.accounts.config.mint = *ctx.accounts.mint.to_account_info().key;
        ctx.accounts.config.gyc_price = 0;
        ctx.accounts.config.sol_price = 0;
        ctx.accounts.config.timestamp = ctx.accounts.clock.unix_timestamp;
        ctx.accounts.config.config_nonce = config_nonce;
        ctx.accounts.config.vault_nonce = vault_nonce;

        emit!(InitEvent {
            status: "ok".to_string(),
            initializer: ctx.accounts.config.initializer.to_string(),
            config: ctx.accounts.config.to_account_info().key.to_string(),
            sol_vault: ctx.accounts.config.sol_vault.to_string(),
            token_vault: ctx.accounts.config.token_vault.to_string(),
            mint: ctx.accounts.config.mint.to_string(),
        });
        Ok(())
    }

    pub fn update_price(
        ctx: Context<UpdatePrice>,
        gyc_price: u64,
        sol_price: u64,
    ) -> ProgramResult {

        ctx.accounts.config.gyc_price = gyc_price;
        ctx.accounts.config.sol_price = sol_price;
        ctx.accounts.config.timestamp = ctx.accounts.clock.unix_timestamp;
        emit!(UpdateEvent {
            status: "ok".to_string(),
            gyc_price: gyc_price.to_string(),
            sol_price: sol_price.to_string(),
            timestamp: ctx.accounts.config.timestamp.to_string(),
        });
        Ok(())
    }

    pub fn gyc_to_sol(
        ctx: Context<GYCtoSOL>,
        amount: u64,
    ) -> ProgramResult {

        let config = &ctx.accounts.config;

        let sol_amount = amount.checked_mul(config.gyc_price).unwrap()
                            .checked_div(config.sol_price).unwrap();
        
        if sol_amount > ctx.accounts.sol_vault.lamports() {
            return Err(ErrorCode::InsufficientSolBalance.into());
        }

        if let COption::Some(delegate) = ctx.accounts.recipient_token.delegate {
            if delegate != *ctx.accounts.sol_vault.key {
                return Err(ErrorCode::InvalidPrivileges.into());
            }
        } else {
            return Err(ErrorCode::InvalidPrivileges.into());
        }

        let seeds = &[
            config.to_account_info().key.as_ref(),
            &[config.vault_nonce],
        ];
        let pda_signer = &[&seeds[..]];

        //transfer gyc token
        let cpi_accounts = Transfer {
            from: ctx.accounts.recipient_token.to_account_info(),
            to: ctx.accounts.token_vault.to_account_info(),
            authority: ctx.accounts.sol_vault.clone(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts).with_signer(pda_signer);
        token::transfer(cpi_ctx, amount)?;

        //transfer sol
        invoke_signed(
            &transfer(
                ctx.accounts.sol_vault.key,
                ctx.accounts.recipient.key,
                sol_amount,
            ),
            &[
                ctx.accounts.sol_vault.clone(),
                ctx.accounts.recipient.clone(),
                ctx.accounts.system_program.to_account_info(),
            ],
            pda_signer,
        )?;

        emit!(SwapEvent {
            status: "ok".to_string(),
            recipient_token: ctx.accounts.recipient_token.to_account_info().key.to_string(),
            recipient: ctx.accounts.recipient.key.to_string(),
            mint: ctx.accounts.mint.to_account_info().key.to_string(),
            token_amount: amount.to_string(),
            sol_amount: sol_amount.to_string(),
            sol_vault_amount: ctx.accounts.sol_vault.lamports().to_string(),
            token_vault_amount: ctx.accounts.token_vault.amount.to_string(),
        });


        Ok(())
    }

    pub fn withdraw(
        ctx: Context<Withdrawal>,
        amount: u64
    ) -> ProgramResult {
        let config = &ctx.accounts.config;
        let seeds = &[
            config.to_account_info().key.as_ref(),
            &[config.vault_nonce],
        ];
        let pda_signer = &[&seeds[..]];

        let cpi_accounts = Transfer {
            from: ctx.accounts.token_vault.to_account_info(),
            to: ctx.accounts.recipient_token.to_account_info(),
            authority: ctx.accounts.sol_vault.clone(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts).with_signer(pda_signer);
        token::transfer(cpi_ctx, amount)?;

        emit!(WithdrawEvent {
            status: "ok".to_string(),
            amount: amount.to_string(),
            recipient: ctx.accounts.signer.to_account_info().key.to_string(),
        });

        Ok(())
    }

}


#[derive(Accounts)]
#[instruction(config_nonce: u8, vault_nonce: u8)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        init, payer = signer,
        seeds = [b"gyc-sol-swap".as_ref()],
        bump = config_nonce,
    )]
    pub config: Box<Account<'info, SwapSettings>>,

    #[account(
        mut,
        seeds = [config.to_account_info().key.as_ref()], 
        bump = vault_nonce,
    )]
    pub sol_vault: AccountInfo<'info>,

    #[account(
        init,
        payer = signer,
        associated_token::mint = mint,
        associated_token::authority = sol_vault,
    )]
    pub token_vault: Account<'info, TokenAccount>,

    pub mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
    pub clock: Sysvar<'info, Clock>,
}

#[account]
#[derive(Default)]
pub struct SwapSettings {
    pub initializer: Pubkey,
    ///Permission to update prices
    pub authority: Pubkey,
    ///the vault of sol
    pub sol_vault: Pubkey,
    ///the vault of gyc tokens
    pub token_vault: Pubkey,
    ///Tokens for exchange with sol
    pub mint: Pubkey,
    ///the recent price of sol
    pub gyc_price: u64,
    ///the recent price of sol
    pub sol_price: u64,
    ///Timestamp of the last sol and gyc token price update
    pub timestamp: i64,
    ///pda nonce of configuration
    pub config_nonce: u8,
    ///pda nonce of vault
    pub vault_nonce: u8,
}


#[derive(Accounts)]
pub struct UpdatePrice<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        mut,
        owner = id()  @ErrorCode::InvalidOwner,
        seeds = [b"gyc-sol-swap".as_ref()],
        bump = config.config_nonce,
        constraint = config.authority == signer.key() @ErrorCode::Unauthorized,
    )]
    pub config: Box<Account<'info, SwapSettings>>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct GYCtoSOL<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(mut)]
    pub recipient: AccountInfo<'info>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = recipient,
        constraint = recipient_token.amount >= amount @ErrorCode::InsufficientTokenBalance,
    )]
    pub recipient_token: Account<'info, TokenAccount>,

    pub mint: Account<'info, Mint>,

    #[account(
        mut,
        seeds = [config.to_account_info().key.as_ref()], 
        bump = config.vault_nonce,
    )]
    pub sol_vault: AccountInfo<'info>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = sol_vault,
    )]
    pub token_vault: Account<'info, TokenAccount>,

    #[account(
        owner = id()  @ErrorCode::InvalidOwner,
        seeds = [b"gyc-sol-swap".as_ref()],
        bump = config.config_nonce,
        constraint = config.authority == signer.key() @ErrorCode::Unauthorized,
        constraint = config.sol_vault == sol_vault.key() @ErrorCode::VaultMismatch,
        constraint = config.token_vault == token_vault.key() @ErrorCode::VaultMismatch,
        constraint = config.mint == mint.key() @ErrorCode::InvalidMintMismatch,
    )]
    pub config: Box<Account<'info, SwapSettings>>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}


#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct Withdrawal<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = signer,
    )]
    pub recipient_token: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [config.to_account_info().key.as_ref()], 
        bump = config.vault_nonce,
    )]
    pub sol_vault: AccountInfo<'info>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = sol_vault,
        constraint = token_vault.amount >= amount @ErrorCode::InsufficientTokenBalance,
    )]
    pub token_vault: Account<'info, TokenAccount>,

    #[account(
        owner = id()  @ErrorCode::InvalidOwner,
        seeds = [b"gyc-sol-swap".as_ref()],
        bump = config.config_nonce,
        constraint = config.initializer == signer.key() @ErrorCode::Unauthorized,
        constraint = config.sol_vault == sol_vault.key() @ErrorCode::VaultMismatch,
        constraint = config.token_vault == token_vault.key() @ErrorCode::VaultMismatch,
    )]
    pub config: Box<Account<'info, SwapSettings>>,

    pub mint: Account<'info, Mint>,

    pub token_program: Program<'info, Token>,

}

#[event]
pub struct InitEvent {
    #[index]
    pub status: String,
    pub initializer: String,
    pub config: String,
    pub sol_vault: String,
    pub token_vault: String,
    pub mint: String,
}

#[event]
pub struct UpdateEvent {
    #[index]
    pub status: String,
    pub gyc_price: String,
    pub sol_price: String,
    pub timestamp: String,
}

#[event]
pub struct SwapEvent {
    #[index]
    pub status: String,
    pub recipient_token: String,
    pub recipient: String,
    pub mint: String,
    pub token_amount: String,
    pub sol_amount: String,
    pub sol_vault_amount: String,
    pub token_vault_amount: String,
}

#[event]
pub struct WithdrawEvent {
    #[index]
    status: String,
    amount: String,
    recipient: String,
}

#[error]
pub enum ErrorCode {
    #[msg("Invalid owner.")]
    InvalidOwner,
    #[msg("You do not have sufficient permissions to perform this action.")]
    Unauthorized,
    #[msg("Insufficient balance of gyc token.")]
    InsufficientTokenBalance,
    #[msg("The vault account mismatch.")]
    VaultMismatch,
    #[msg("Insufficient balance of Sol.")]
    InsufficientSolBalance,
    #[msg("No operating privileges.")]
    InvalidPrivileges,
    #[msg("The mint mismatch.")]
    InvalidMintMismatch,
   
}