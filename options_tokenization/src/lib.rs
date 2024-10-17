use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, TokenAccount, Token, MintTo, Transfer, Burn};
use anchor_lang::solana_program::system_program;

declare_id!("AxtDraLXGvmwwjUN34wYd8D1RsXDCZpvyC8FN7cHjBAG");

/// Constants for option types
const OPTION_TYPE_CALL: u8 = 0;
const OPTION_TYPE_PUT: u8 = 1;

/// Program entrypoint
#[program]
pub mod options_tokenization {
    use super::*;

    // Mint a new call or put option with optional fee
    pub fn mint_option(
        ctx: Context<MintOption>,
        strike_price: u64,
        expiration: i64,
        option_type: u8, // 0 for call, 1 for put
        amount_underlying: u64, // Amount of underlying asset (e.g., 1 USDC or 100 USDC)
        fee: u64, // Optional fee for minting
        is_american: bool // Option style: true for American, false for European
    ) -> Result<()> {
        let option = &mut ctx.accounts.option;
        
        // Ensure the option type is valid
        require!(option_type == OPTION_TYPE_CALL || option_type == OPTION_TYPE_PUT, MyError::InvalidOptionType);

        // Initialize the option state
        option.strike_price = strike_price;
        option.expiration = expiration;
        option.option_type = option_type;
        option.amount_underlying = amount_underlying;
        option.mint_authority = *ctx.accounts.user.key;
        option.is_exercised = false;
        option.underlying_mint = ctx.accounts.underlying_mint.key();
        option.is_american = is_american;

        // Mint the SPL token to represent the option
        let cpi_accounts = MintTo {
            mint: ctx.accounts.mint.to_account_info(),
            to: ctx.accounts.token_account.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        token::mint_to(cpi_ctx, 1)?;

        // Lock the underlying asset in escrow
        let transfer_underlying_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.underlying_asset_account.to_account_info(),
                to: ctx.accounts.escrow.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        );
        token::transfer(transfer_underlying_ctx, amount_underlying)?;

        // Collect minting fee (if any)
        if fee > 0 {
            let transfer_fee_ctx = CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user.to_account_info(),
                    to: ctx.accounts.fee_receiver.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            );
            token::transfer(transfer_fee_ctx, fee)?;
        }

        emit!(OptionMinted {
            minter: *ctx.accounts.user.key,
            option_type,
            strike_price,
            expiration,
            amount_underlying,
        });

        Ok(())
    }

    // Transfer option token
    pub fn transfer_option(ctx: Context<TransferOption>, amount: u64) -> Result<()> {
        let option = &ctx.accounts.option;
        // Ensure the option has not expired
        let clock = Clock::get()?;
        require!(clock.unix_timestamp < option.expiration, MyError::OptionExpired);

        let cpi_accounts = Transfer {
            from: ctx.accounts.from.to_account_info(),
            to: ctx.accounts.to.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        token::transfer(cpi_ctx, amount)?;

        emit!(OptionTransferred {
            from: ctx.accounts.from.owner, 
            to: ctx.accounts.to.owner,    
            amount,
        });

        Ok(())
    }

    // Exercise the option (Call or Put), with support for partial exercise
    pub fn exercise_option(ctx: Context<ExerciseOption>, amount: u64) -> Result<()> {
        let option = &mut ctx.accounts.option;

        // Ensure the option has not expired
        let clock = Clock::get()?;
        require!(clock.unix_timestamp < option.expiration, MyError::OptionExpired);

        // Ensure the option is not already exercised
        require!(!option.is_exercised, MyError::OptionAlreadyExercised);

        // Ensure the amount being exercised is valid
        require!(amount <= option.amount_underlying, MyError::InvalidAmount);

        // Ensure early exercise is allowed for American options, or it's the expiration date for European options
        if !option.is_american {
            require!(clock.unix_timestamp >= option.expiration, MyError::EarlyExerciseNotAllowed);
        }

        let user_key = *ctx.accounts.user.key;

        match option.option_type {
            OPTION_TYPE_CALL => {
                // Call Option: User (option holder) buys the underlying asset at the strike price.
                let proportional_strike_price = (option.strike_price * amount) / option.amount_underlying;

                // Transfer proportional strike price from the option holder to the escrow
                let transfer_strike_price_ctx = CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.option_holder.to_account_info(),
                        to: ctx.accounts.escrow.to_account_info(),
                        authority: ctx.accounts.user.to_account_info(),
                    },
                );
                token::transfer(transfer_strike_price_ctx, proportional_strike_price)?;

                // Transfer the proportional underlying asset from escrow to the option holder
                let transfer_underlying_ctx = CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.escrow.to_account_info(),
                        to: ctx.accounts.option_holder.to_account_info(),
                        authority: ctx.accounts.escrow_authority.to_account_info(),
                    },
                );
                token::transfer(transfer_underlying_ctx, amount)?;
            }

            OPTION_TYPE_PUT => {
                // Put Option: User (option holder) sells the underlying asset at the strike price.
                let proportional_strike_price = (option.strike_price * amount) / option.amount_underlying;

                // Transfer the proportional underlying asset from the option holder to the escrow
                let transfer_underlying_ctx = CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.option_holder.to_account_info(),
                        to: ctx.accounts.escrow.to_account_info(),
                        authority: ctx.accounts.user.to_account_info(),
                    },
                );
                token::transfer(transfer_underlying_ctx, amount)?;

                // Transfer the proportional strike price from escrow to the option holder
                let transfer_strike_price_ctx = CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.escrow.to_account_info(),
                        to: ctx.accounts.option_holder.to_account_info(),
                        authority: ctx.accounts.escrow_authority.to_account_info(),
                    },
                );
                token::transfer(transfer_strike_price_ctx, proportional_strike_price)?;
            }

            _ => return Err(MyError::InvalidOptionType.into()),
        }

        // Update the amount of the underlying asset remaining
        option.amount_underlying -= amount;

        // If all underlying has been exercised, mark the option as fully exercised and burn it
        if option.amount_underlying == 0 {
            option.is_exercised = true;

            // Option is now burned after exercising
            let cpi_accounts = Burn {
                mint: ctx.accounts.mint.to_account_info(),
                from: ctx.accounts.option_token_account.to_account_info(),  // Fixed: from, not to
                authority: ctx.accounts.user.to_account_info(),
            };
            let cpi_program = ctx.accounts.token_program.to_account_info();
            let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
            token::burn(cpi_ctx, 1)?;  // Burn 1 option token
        }

        emit!(OptionExercised {
            exerciser: user_key,
            option_type: option.option_type,
            strike_price: option.strike_price,
            expiration: option.expiration,
        });

        Ok(())
    }

    // Cancel an option before expiry
    pub fn cancel_option(ctx: Context<CancelOption>) -> Result<()> {
        let option = &ctx.accounts.option;
        require!(!option.is_exercised, MyError::OptionAlreadyExercised);

        // Return underlying assets to the option creator
        let transfer_underlying_back_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.escrow.to_account_info(),
                to: ctx.accounts.user.to_account_info(),  // Return to option creator
                authority: ctx.accounts.escrow_authority.to_account_info(),
            },
        );
        token::transfer(transfer_underlying_back_ctx, option.amount_underlying)?;

        emit!(OptionCancelled {
            creator: ctx.accounts.user.key(),
            option_type: option.option_type,
            amount_returned: option.amount_underlying,
        });

        Ok(())
    }

    // Function to expire the option and return funds from escrow
    pub fn expire_option(ctx: Context<ExpireOption>) -> Result<()> {
        let option = &ctx.accounts.option;

        // Ensure the option has expired
        let clock = Clock::get()?;
        require!(clock.unix_timestamp >= option.expiration, MyError::OptionNotExpired);

        // Return the underlying asset or strike price from escrow to the original minter (or another designated recipient)
        let return_funds_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.escrow.to_account_info(),
                to: ctx.accounts.mint_authority.to_account_info(),
                authority: ctx.accounts.escrow_authority.to_account_info(),
            },
        );

        // If it is a call, return the underlying asset; if it is a put, return the strike price
        if option.option_type == OPTION_TYPE_CALL {
            token::transfer(return_funds_ctx, option.amount_underlying)?;
        } else if option.option_type == OPTION_TYPE_PUT {
            token::transfer(return_funds_ctx, option.strike_price)?;
        } else {
            return Err(MyError::InvalidOptionType.into());
        }

        emit!(OptionExpired {
            option_type: option.option_type,
            strike_price: option.strike_price,
            expiration: option.expiration,
        });

        Ok(())
    }
}

#[derive(Accounts)]
pub struct MintOption<'info> {
    #[account(init, payer = user, space = 8 + OptionState::LEN)]
    pub option: Account<'info, OptionState>,
    #[account(mut)]
    pub mint: Account<'info, Mint>,  // The SPL token mint for the option
    #[account(mut)]
    pub token_account: Account<'info, TokenAccount>,  // The recipient's token account
    #[account(mut)]
    pub user: Signer<'info>,  // The person minting the option
    pub underlying_mint: Account<'info, Mint>,  // The underlying asset's mint (e.g., USDC, SOL)
    #[account(mut)]
    pub underlying_asset_account: Account<'info, TokenAccount>,  // The user's token account for the underlying asset
    #[account(mut)]
    pub escrow: Account<'info, TokenAccount>,  // The escrow account to lock underlying assets
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    #[account(mut)]
    pub fee_receiver: Account<'info, TokenAccount>, // Receiver of minting fees
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct TransferOption<'info> {
    #[account(mut)]
    pub from: Account<'info, TokenAccount>,
    #[account(mut)]
    pub to: Account<'info, TokenAccount>,
    #[account(signer)]
    pub authority: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
    #[account(mut)]
    pub option: Account<'info, OptionState>,
}

#[derive(Accounts)]
pub struct ExerciseOption<'info> {
    #[account(mut)]
    pub option: Account<'info, OptionState>,  // The option state being exercised
    #[account(mut)]
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub option_token_account: Account<'info, TokenAccount>,  // Option holder's account holding the option token
    #[account(mut)]
    pub option_holder: Account<'info, TokenAccount>,  // The account holding the user's token (strike price or underlying asset)
    #[account(mut)]
    pub escrow: Account<'info, TokenAccount>,  // The escrow holding underlying or strike price
    #[account(signer)]
    pub escrow_authority: AccountInfo<'info>,  // Authority to manage escrow (could be a program-derived address)
    #[account(mut)]
    pub user: Signer<'info>,  // The person exercising the option
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct ExpireOption<'info> {
    #[account(mut)]
    pub option: Account<'info, OptionState>,  // The option to expire
    #[account(mut)]
    pub escrow: Account<'info, TokenAccount>,  // Escrow account holding the assets
    #[account(signer)]
    pub escrow_authority: AccountInfo<'info>,  // Authority for managing the escrow
    #[account(mut)]
    pub mint_authority: AccountInfo<'info>,  // The original minter or the designated recipient of returned funds
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct CancelOption<'info> {
    #[account(mut)]
    pub option: Account<'info, OptionState>,
    #[account(mut)]
    pub user: Signer<'info>, // Option creator
    #[account(mut)]
    pub escrow: Account<'info, TokenAccount>, // Escrow account holding assets
    #[account(signer)]
    pub escrow_authority: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
}

#[account]
pub struct OptionState {
    pub strike_price: u64,        // Strike price for the option (in terms of underlying asset)
    pub expiration: i64,          // Expiration timestamp (Unix)
    pub option_type: u8,          // Call (0) or Put (1)
    pub underlying_mint: Pubkey,  // Mint of the underlying SPL token (or SOL)
    pub amount_underlying: u64,   // Amount of the underlying asset to be traded
    pub mint_authority: Pubkey,   // The original minter (could be used for validation)
    pub is_exercised: bool,       // Whether the option has been exercised
    pub is_american: bool,        // True for American option, false for European option
}

impl OptionState {
    const LEN: usize = 8 + 8 + 1 + 32 + 8 + 32 + 1 + 1;  // Size of the OptionState
}

/// Custom errors
#[error_code]
pub enum MyError {
    #[msg("Invalid option type. Must be 0 (Call) or 1 (Put).")]
    InvalidOptionType,
    #[msg("Option has expired.")]
    OptionExpired,
    #[msg("Option has already been exercised.")]
    OptionAlreadyExercised,
    #[msg("Option is not eligible for early exercise.")]
    EarlyExerciseNotAllowed,
    #[msg("Invalid amount specified for partial exercise.")]
    InvalidAmount,
    #[msg("Option has not expired yet.")]
    OptionNotExpired,
}

/// Events for tracking on-chain actions
#[event]
pub struct OptionMinted {
    pub minter: Pubkey,
    pub option_type: u8,
    pub strike_price: u64,
    pub expiration: i64,
    pub amount_underlying: u64,
}

#[event]
pub struct OptionTransferred {
    pub from: Pubkey,
    pub to: Pubkey,
    pub amount: u64,
}

#[event]
pub struct OptionExercised {
    pub exerciser: Pubkey,
    pub option_type: u8,
    pub strike_price: u64,
    pub expiration: i64,
}

#[event]
pub struct OptionExpired {
    pub option_type: u8,
    pub strike_price: u64,
    pub expiration: i64,
}

#[event]
pub struct OptionCancelled {
    pub creator: Pubkey,
    pub option_type: u8,
    pub amount_returned: u64,
}
