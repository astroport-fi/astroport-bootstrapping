use cosmwasm_std::{Addr, Decimal256, Uint128};
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONFIG: Item<Config> = Item::new("config");
pub const STATE: Item<State> = Item::new("state");
pub const USERS: Map<&Addr, UserInfo> = Map::new("users");

//----------------------------------------------------------------------------------------
// Storage types
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Config {
    /// Account who can update config
    pub owner: Addr,
    ///  ASTRO token address
    pub astro_token_address: Addr,
    /// Airdrop Contract address
    pub airdrop_contract_address: Addr,
    /// Lockdrop Contract address
    pub lockdrop_contract_address: Addr,
    ///  ASTRO-UST LP Pool address
    pub astro_ust_pool_address: Addr,
    ///  ASTRO-UST LP Token address
    pub astro_ust_lp_token_address: Addr,
    ///  Astroport Generator contract with which ASTRO-UST LP Tokens are staked
    pub generator_contract: Addr,
    /// Total ASTRO token rewards to be used to incentivize boostrap auction participants
    pub astro_rewards: Uint128,
    /// Number of seconds over which ASTRO incentives are vested
    pub astro_vesting_duration: u64,
    ///  Number of seconds over which LP Tokens are vested
    pub lp_tokens_vesting_duration: u64,
    /// Timestamp since which ASTRO / UST deposits will be allowrd
    pub init_timestamp: u64,
    /// Number of seconds post init_timestamp during which deposits / withdrawals will be allowed
    pub deposit_window: u64,
    /// Number of seconds post deposit_window completion during which only withdrawals are allowed
    pub withdrawal_window: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct State {
    /// Total ASTRO tokens delegated to the contract by lockdrop participants / airdrop recepients
    pub total_astro_delegated: Uint128,
    /// Total UST delegated to the contract
    pub total_ust_delegated: Uint128,

    pub is_pool_created: bool,
    /// ASTRO--UST LP Shares currently staked with the Staking contract
    pub is_lp_staked: bool,

    /// Total LP shares minted post liquidity addition to the ASTRO-UST Pool
    pub lp_shares_minted: Uint128,
    /// Number of LP shares that have been withdrawn as they unvest
    pub lp_shares_claimed: Uint128,

    /// Timestamp at which liquidity was added to the ASTRO-UST LP Pool
    pub pool_init_timestamp: u64,
    /// index used to keep track of LP staking rewards and distribute them proportionally among the auction participants
    pub global_reward_index: Decimal256,
}

impl Default for State {
    fn default() -> Self {
        State {
            total_astro_delegated: Uint128::zero(),
            total_ust_delegated: Uint128::zero(),
            lp_shares_minted: Uint128::zero(),
            lp_shares_claimed: Uint128::zero(),
            pool_init_timestamp: 0u64,
            is_lp_staked: false,
            global_reward_index: Decimal256::zero(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserInfo {
    // Total ASTRO Tokens delegated by the user
    pub astro_delegated: Uint128,
    // Total UST deposited by the user
    pub ust_deposited: Uint128,
    // Withdrawal counter to capture if the user already withdrew UST during the "only withdrawals" window
    pub ust_withdrawn: bool,
    // User's LP share balance
    pub lp_shares: Uint128,
    // LP shares withdrawn by the user
    pub claimed_lp_shares: Uint128,
    // User's ASTRO rewards for participating in the auction
    pub total_auction_incentives: Uint128,
    // ASTRO rewards withdrawn by the user
    pub claimed_auction_incentives: Uint128,
    // Index used to calculate user's staking rewards
    pub user_reward_index: Decimal256,
}

impl Default for UserInfo {
    fn default() -> Self {
        UserInfo {
            astro_delegated: Uint128::zero(),
            ust_deposited: Uint128::zero(),
            ust_withdrawn: false,
            lp_shares: Uint128::zero(),
            claimed_lp_shares: Uint128::zero(),
            total_auction_incentives: Uint128::zero(),
            claimed_auction_incentives: Uint128::zero(),
            user_reward_index: Decimal256::zero(),
        }
    }
}
