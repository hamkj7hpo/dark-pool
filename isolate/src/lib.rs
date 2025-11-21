// isolate/src/lib.rs

use anchor_lang::prelude::*;
use anchor_spl::token_interface::{TokenInterface, TokenAccount as InterfaceTokenAccount};

use raydium_cp_swap::cpi::accounts::Swap;
use raydium_cp_swap::cpi::swap_base_input;

use ark_bls12_381::{Bls12_381, G1Affine, G2Affine};
use ark_ec::pairing::Pairing;
use ark_ec::AffineRepr;
use ark_serialize::CanonicalDeserialize;
use sha2::{Digest, Sha256};

declare_id!("7VJ8PT2BA5UacYLyF1AmYMCjZjJijqMthgHzQogFhfSt");

#[program]
pub mod isolate {
    use super::*;

    #[event]
    pub struct DarkSwap {
        pub authority: Pubkey,
        pub amount_in: u64,
        pub is_buy: bool,
        pub classification: String,
        pub timestamp: i64,
    }

    pub fn swap(
        ctx: Context<SwapContext>,
        amount_in: u64,
        is_buy: bool,
        minimum_amount_out: u64,
        bls_sig: [u8; 96],
        bls_pk: [u8; 48],
        nonce: u64,
    ) -> Result<()> {
        let clock = Clock::get()?;

        let mut msg = Vec::with_capacity(65);
        msg.extend_from_slice(&amount_in.to_le_bytes());
        msg.push(is_buy as u8);
        msg.extend_from_slice(&minimum_amount_out.to_le_bytes());
        msg.extend_from_slice(&nonce.to_le_bytes());
        msg.extend_from_slice(&ctx.accounts.user.key().to_bytes());

        require!(verify_bls_sig(bls_sig, bls_pk, &msg), ErrorCode::InvalidBlsSignature);
        require_eq!(ctx.accounts.vault.nonce, nonce, ErrorCode::InvalidNonce);

        if ctx.accounts.global_state.swap_count == 0 {
            require!(
                clock.unix_timestamp - ctx.accounts.global_state.bond_timestamp >= 120,
                ErrorCode::AntiSniperCooldown
            );
        }

        let net_amount = amount_in * 9750 / 10000;

        let cpi_accounts = Swap {
            authority: ctx.accounts.swap_authority.to_account_info(),
            payer: ctx.accounts.user.to_account_info(),
            amm_config: ctx.accounts.amm_config.to_account_info(),
            pool_state: ctx.accounts.pool_state.to_account_info(),
            input_token_account: if is_buy {
                ctx.accounts.user_sol.to_account_info()
            } else {
                ctx.accounts.user_token.to_account_info()
            },
            output_token_account: if is_buy {
                ctx.accounts.user_token.to_account_info()
            } else {
                ctx.accounts.user_sol.to_account_info()
            },
            input_token_mint: ctx.accounts.input_mint.to_account_info(),
            output_token_mint: ctx.accounts.output_mint.to_account_info(),
            input_vault: ctx.accounts.input_vault.to_account_info(),
            output_vault: ctx.accounts.output_vault.to_account_info(),
            input_token_program: ctx.accounts.token_program.to_account_info(),
            output_token_program: ctx.accounts.token_program.to_account_info(),
            observation_state: ctx.accounts.observation_state.to_account_info(),
        };

        let cpi_ctx = CpiContext::new(ctx.accounts.raydium_program.to_account_info(), cpi_accounts);
        swap_base_input(cpi_ctx, if is_buy { net_amount } else { amount_in }, minimum_amount_out)?;

        let classification = if amount_in >= 1_000_000_000_000 {
            "Leviathan".to_string()
        } else if amount_in >= 100_000_000_000 {
            "Kraken".to_string()
        } else if amount_in >= 10_000_000_000 {
            "Whale".to_string()
        } else if amount_in >= 1_000_000_000 {
            "Shark".to_string()
        } else {
            "Crab".to_string()
        };

        emit!(DarkSwap {
            authority: ctx.accounts.swap_authority.key(),
            amount_in,
            is_buy,
            classification,
            timestamp: clock.unix_timestamp,
        });

        let gs = &mut ctx.accounts.global_state;
        gs.swap_count += 1;
        gs.total_swapped = gs.total_swapped.checked_add(amount_in).ok_or(ErrorCode::MathError)?;

        ctx.accounts.vault.nonce = nonce.checked_add(1).ok_or(ErrorCode::MathError)?;
        ctx.accounts.user_state.last_swap = clock.unix_timestamp;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct SwapContext<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(seeds = [b"darkswap", user.key().as_ref()], bump)]
    pub swap_authority: SystemAccount<'info>,

    #[account(mut)]
    pub user_sol: InterfaceAccount<'info, InterfaceTokenAccount>,

    #[account(mut)]
    pub user_token: InterfaceAccount<'info, InterfaceTokenAccount>,

    #[account(mut)]
    pub global_state: Account<'info, GlobalState>,

    #[account(mut)]
    pub vault: Account<'info, Vault>,

    #[account(mut)]
    pub user_state: Account<'info, UserState>,

    /// CHECK: Validated by Raydium
    pub amm_config: UncheckedAccount<'info>,

    /// CHECK: Validated by Raydium
    #[account(mut)]
    pub pool_state: UncheckedAccount<'info>,

    #[account(mut)]
    pub input_vault: InterfaceAccount<'info, InterfaceTokenAccount>,

    #[account(mut)]
    pub output_vault: InterfaceAccount<'info, InterfaceTokenAccount>,

    /// CHECK: Mint validated by vault
    pub input_mint: UncheckedAccount<'info>,

    /// CHECK: Mint validated by vault
    pub output_mint: UncheckedAccount<'info>,

    /// CHECK: Observation validated by Raydium
    #[account(mut)]
    pub observation_state: UncheckedAccount<'info>,

    pub raydium_program: Program<'info, raydium_cp_swap::program::RaydiumCpSwap>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[account] pub struct GlobalState { pub swap_count: u64, pub total_swapped: u64, pub bond_timestamp: i64 }
#[account] pub struct Vault { pub nonce: u64 }
#[account] pub struct UserState { pub last_swap: i64 }

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid BLS signature")] InvalidBlsSignature,
    #[msg("Invalid nonce")] InvalidNonce,
    #[msg("Anti-sniper cooldown")] AntiSniperCooldown,
    #[msg("Math overflow")] MathError,
}

pub fn verify_bls_sig(sig_bytes: [u8; 96], pk_bytes: [u8; 48], msg: &[u8]) -> bool {
    let sig = match G2Affine::deserialize_compressed(&sig_bytes[..]) { Ok(s) => s, Err(_) => return false };
    let pk  = match G1Affine::deserialize_compressed(&pk_bytes[..])  { Ok(p) => p, Err(_) => return false };

    let mut hasher = Sha256::new();
    hasher.update(msg);
    hasher.update(b"SAFE-PUMP-V5");
    let base = hasher.finalize();

    let hash_point = {
        let mut i = 0u8;
        loop {
            let mut h = Sha256::new();
            h.update(&base);
            h.update(&[i]);
            let digest = h.finalize();
            if let Ok(p) = G2Affine::deserialize_compressed(&digest[..]) {
                break p;
            }
            i = i.wrapping_add(1);
            if i == 0 { return false; }
        }
    };

    Bls12_381::pairing(G1Affine::generator(), sig) == Bls12_381::pairing(pk, hash_point)
}
