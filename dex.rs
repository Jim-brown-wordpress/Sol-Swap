#![cfg(feature = "program")]

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::{Entry, EntryPoint, ProgramResult},
    entrypoint::ProgramResult::{InvalidArgument, Success},
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    program_pack::{Pack, IsInitialized},
    sysvar::{Sysvar},
    rent::Rent,
    system_program,
    clock::{self, UnixTimestamp},
    spl_token::{self, instruction::{transfer}, state::{Account}},
};
use std::mem::size_of;
use num_enum::TryFromPrimitive;

// Generate program ID in `Solana-keygen new` format
solana_program::declare_id!("DEXprojBt4Rv7Gh5z623Yf7fyTNzgJ123JzNnmCQ8Fr");

/**
 * Error definitions
 */
#[derive(Debug, TryFromPrimitive)]
#[repr(u8)]
pub enum DexError {
    InvalidInstruction = 0,
    TradeAlreadyExist = 1,
    TradeNotFound = 2,
    InsufficientFunds = 3,
}

impl From<DexError> for ProgramError {
    fn from(e: DexError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

/**
 * Constants
 */
const MAX_TRADES_SIZE: usize = 1024;
const SIGNER_SEED: &[&[u8]] = &[b"solana", b"dex"];
const MINIMUM_TRADE_AMOUNT: u64 = 100;

/**
 * DEX trade data structure
 */
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trade {
    pub maker_pubkey: Pubkey,
    pub taker_amount: u64,
    pub maker_amount: u64,
    pub taker_token_pubkey: Pubkey,
    pub maker_token_pubkey: Pubkey,
    pub trade_timestamp: UnixTimestamp,
}

impl Trade {
    pub fn new(
        maker_pubkey: Pubkey,
        taker_amount: u64,
        maker_amount: u64,
        taker_token_pubkey: Pubkey,
        maker_token_pubkey: Pubkey,
        trade_timestamp: UnixTimestamp,
    ) -> Self {
        Self {
            maker_pubkey,
            taker_amount,
            maker_amount,
            taker_token_pubkey,
            maker_token_pubkey,
            trade_timestamp
        }
    }
}

impl Pack for Trade {
    const LEN: usize = size_of::<Trade>();

    fn pack_into_slice(&self, output: &mut [u8]) {
        let data = self.as_ref();
        output.copy_from_slice(data);
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        if input.len() != size_of::<Trade>() {
            return Err(ProgramError::InvalidArgument);
        }
        let trade = unsafe { &*(input.as_ptr() as *const Trade) };
        Ok(*trade)
    }
}

impl IsInitialized for Trade {
    fn is_initialized(&self) -> bool {
        self.maker_pubkey != Pubkey::default()
    }
}

impl Default for Trade {
    fn default() -> Self {
        Self {
            maker_pubkey: Pubkey::default(),
            taker_amount: 0,
            maker_amount: 0,
            taker_token_pubkey: Pubkey::default(),
            maker_token_pubkey: Pubkey::default(),
            trade_timestamp: 0,
        }
    }
}

/**
 * Program entrypoint and instructions
 */
#[derive(Debug, TryFromPrimitive)]
#[repr(u8)]
pub enum DexInstruction {
    CreateTrade = 0,
    CompleteTrade = 1,
}

struct CreateTradeParams {
    taker_amount: u64,
    maker_amount: u64,
    taker_token_pubkey: Pubkey,
    maker_token_pubkey: Pubkey,
}

fn create_trade(
    accounts: &[AccountInfo],
    params: CreateTradeParams
) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();
    let trade_account = next_account_info(accounts_iter)?;
    let taker_account = next_account_info(accounts_iter)?;
    let maker_account = next_account_info(accounts_iter)?;

    // Verify the rent exemption
    let rent = &Rent::from_account_info(next_account_info(accounts_iter)?)?;
    if !rent.is_exempt(trade_account.lamports(), trade_account.data_len()) {
        return Err(ProgramError::AccountNotRentExempt);
    }

    // Check the trade doesn't already exist
    if trade_account.lamports() > 0 {
        return Err(DexError::TradeAlreadyExist.into());
