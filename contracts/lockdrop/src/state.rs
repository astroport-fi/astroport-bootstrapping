use astroport_periphery::lockdrop::MigrationInfo;
use cosmwasm_std::{Addr, Decimal, Uint128, Uint256};
use cw_storage_plus::{Item, Map, U64Key};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONFIG: Item<Config> = Item::new("config");
pub const STATE: Item<State> = Item::new("state");

/// Key is an Terraswap LP token address
pub const ASSET_POOLS: Map<&Addr, PoolInfo> = Map::new("LiquidityPools");
/// Key is an user address
pub const USER_INFO: Map<&Addr, UserInfo> = Map::new("users");
/// Key consists of an Terraswap LP token address, an user address, and a duration
pub const LOCKUP_INFO: Map<(&Addr, &Addr, U64Key), LockupInfo> = Map::new("lockup_position");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// Account which can update the config
    pub owner: Addr,
    /// ASTRO Token address
    pub astro_token: Option<Addr>,
    /// Bootstrap Auction contract address
    pub auction_contract: Option<Addr>,
    /// Generator (Staking for dual rewards) contract address
    pub generator: Option<Addr>,
    /// Timestamp when Contract will start accepting LP Token deposits
    pub init_timestamp: u64,
    /// Number of seconds during which lockup deposits will be accepted
    pub deposit_window: u64,
    /// Withdrawal Window Length :: Post the deposit window
    pub withdrawal_window: u64,
    /// Min. no. of weeks allowed for lockup
    pub min_lock_duration: u64,
    /// Max. no. of weeks allowed for lockup
    pub max_lock_duration: u64,
    /// Lockdrop Reward multiplier
    pub weekly_multiplier: u64,
    /// Lockdrop Reward divider
    pub weekly_divider: u64,
    /// Total ASTRO lockdrop incentives to be distributed among the users
    pub lockdrop_incentives: Option<Uint128>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    /// Total ASTRO incentives share
    pub total_incentives_share: u64,
    /// ASTRO Tokens delegated to the bootstrap auction contract
    pub total_astro_delegated: Uint128,
    /// ASTRO returned to forcefully unlock Lockup positions
    pub total_astro_returned_available: Uint128,
    /// Boolean value indicating if the user can withdraw their ASTRO rewards or not
    pub are_claims_allowed: bool,
    pub supported_pairs_list: Vec<Addr>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolInfo {
    pub terraswap_pool: Addr,
    pub terraswap_amount_in_lockups: Uint128,
    pub migration_info: Option<MigrationInfo>,
    /// Share of total ASTRO incentives allocated to this pool
    pub incentives_share: u64,
    /// Weighted LP Token balance used to calculate ASTRO rewards a particular user can claim
    pub weighted_amount: Uint256,
    /// Ratio of Generator ASTRO rewards accured to astroport pool share
    pub generator_astro_per_share: Decimal,
    /// Ratio of Generator Proxy rewards accured to astroport pool share
    pub generator_proxy_per_share: Decimal,
    /// Boolean value indicating if the LP Tokens are staked with the Generator contract or not
    pub is_staked: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct UserInfo {
    /// Total ASTRO tokens user delegated to the LP bootstrap auction pool
    pub delegated_astro_rewards: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LockupInfo {
    /// Terraswap LP units locked by the user
    pub lp_units_locked: Uint128,
    /// Boolean value indicating if the user's has withdrawn funds post the only 1 withdrawal limit cutoff
    pub withdrawal_flag: bool,
    /// ASTRO tokens received as rewards for participation in the lockdrop
    pub astro_rewards: Uint128,
    /// ASTRO tokens transferred to user
    pub astro_transferred: bool,
    /// Generator ASTRO tokens loockup received as generator rewards
    pub generator_astro_debt: Uint128,
    /// Generator Proxy tokens lockup received as generator rewards
    pub generator_proxy_debt: Uint128,
    /// Timestamp beyond which this position can be unlocked
    pub unlock_timestamp: u64,
}
