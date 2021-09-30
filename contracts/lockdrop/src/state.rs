use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

use cosmwasm_bignumber::{Decimal256, Uint256};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONFIG: Item<Config> = Item::new("config");
pub const STATE: Item<State> = Item::new("state");

pub const ASSET_POOLS: Map<&Addr, PoolInfo> = Item::new("lp_assets");

pub const USER_INFO: Map<&Addr, UserInfo> = Map::new("users");
pub const LOCKUP_INFO: Map<&[u8], LockupInfo> = Map::new("lockup_position");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// Account who can update config
    pub owner: Addr,
    /// Bootstrap Auction contract address
    pub auction_contract_address: Addr,
    /// Timestamp when Contract will start accepting LP Token deposits
    pub init_timestamp: u64,
    /// Deposit Window Length
    pub deposit_window: u64,
    /// Withdrawal Window Length :: Post the deposit window
    pub withdrawal_window: u64,
    /// Min. no. of weeks allowed for lockup
    pub min_lock_duration: u64,
    /// Max. no. of weeks allowed for lockup
    pub max_lock_duration: u64,
    /// Number of seconds per week
    pub seconds_per_week: u64,
    /// Lockdrop Reward multiplier
    pub weekly_multiplier: Decimal256,
    /// Total ASTRO lockdrop incentives to be distributed among the users
    pub lockdrop_incentives: Uint256,
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolInfo {
    /// LP Token Address
    pub lp_token_addr: Uint256,
    /// Pool Address
    pub pool_addr: Uint256,
    /// 
    pub lockdrop_incentives_percent: Decimal256,
    /// 
    pub total_lp_units_before_migration: Uint256,
    /// 
    pub total_lp_units_after_migration: Uint256,
    /// 
    pub total_lp_units_staked_with_generator: Uint256,
    /// Total weighted deposits
    pub pool_type: String,
    /// Total weighted deposits
    pub is_migrated: bool,
    /// Ratio of ASTRO rewards accured to total_lp_deposited. Used to calculate ASTRO incentives accured by each user
    pub astro_global_reward_index: Decimal256,
    /// Ratio of ASSET rewards accured to total_lp_deposited. Used to calculate ASSET incentives accured by each user
    pub asset_global_reward_index: Decimal256,
}





#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    /// 
    pub total_astro_delegated: Uint256,
    /// 
    pub total_astro_unclaimed: Uint256,
    /// Boolean value indicating if the user can withdraw his ASTRO rewards or not
    pub are_claims_allowed: bool,
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserInfo {
    /// Total ASTRO tokens user received as rewards for participation in the lockdrop
    pub total_astro_reward: Uint256,
    /// Total ASTRO tokens user can still withdraw 
    pub unclaimed_astro_reward: Uint256,
    /// Total ASTRO tokens user delegated to the LP bootstrap auction pool 
    pub delegated_astro_reward: Uint256,
    /// Contains lockup Ids of the User's lockup positions with different pools having different durations / deposit amounts
    pub lockup_positions: Vec<String>,
}


impl Default for UserInfo {
    fn default() -> Self {
        UserInfo {
            total_astro_reward: Uint256::zero(),
            unclaimed_astro_reward: Uint256::zero(),
            delegated_astro_reward: Uint256::zero(),
            lockup_positions: vec![]
        }
    }
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LockupInfo {
    /// Lockup Duration
    pub duration: u64,
    /// UST locked as part of this lockup position
    pub lp_units_locked: Uint256,
    /// Timestamp beyond which this position can be unlocked
    pub unlock_timestamp: u64,
}


impl Default for LockupInfo {
    fn default() -> Self {
        LockupInfo {
            duration: 0 as u64,
            lp_units_locked: Uint256::zero(),
            unlock_timestamp: 0 as u64,
        }
    }
}
