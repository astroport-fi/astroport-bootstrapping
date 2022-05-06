use astroport::asset::AssetInfo;
use astroport::common::OwnershipProposal;
use astroport::generator::PoolInfoResponse;
use astroport::generator::QueryMsg as GenQueryMsg;
use astroport::restricted_vector::RestrictedVector;
use astroport_periphery::lockdrop::MigrationInfo;
use cosmwasm_std::{Addr, Decimal, Decimal256, Deps, StdError, StdResult, Uint128, Uint256};
use cw_storage_plus::{Item, Map, U64Key};

use crate::raw_queries::raw_proxy_asset;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONFIG: Item<Config> = Item::new("config");
pub const STATE: Item<State> = Item::new("state");

/// Key is an Terraswap LP token address
pub const ASSET_POOLS: Map<&Addr, PoolInfo> = Map::new("LiquidityPools");
/// Key is an user address
pub const USER_INFO: Map<&Addr, UserInfo> = Map::new("users");
/// Key consists of an Terraswap LP token address, an user address, and a duration
pub const LOCKUP_INFO: Map<(&Addr, &Addr, U64Key), LockupInfoV2> = Map::new("lockup_position");
/// Old LOCKUP_INFO storage interface for backward compatibility
pub const OLD_LOCKUP_INFO: Map<(&Addr, &Addr, U64Key), LockupInfoV1> = Map::new("lockup_position");
/// Total received asset reward by lockdrop contract per lp token share
pub const TOTAL_ASSET_REWARD_INDEX: Map<&Addr, Decimal256> = Map::new("total_asset_reward_index");
/// Last used total asset reward index for user claim ( lp_addr -> user -> duration )
pub const USERS_ASSET_REWARD_INDEX: Map<(&Addr, &Addr, U64Key), Decimal256> =
    Map::new("users_asset_reward_index");

pub trait CompatibleLoader<K, R> {
    fn compatible_load(&self, deps: Deps, key: K, generator: &Option<Addr>) -> StdResult<R>;

    fn compatible_may_load(
        &self,
        deps: Deps,
        key: K,
        generator: &Option<Addr>,
    ) -> StdResult<Option<R>>;
}

impl CompatibleLoader<(&Addr, &Addr, U64Key), LockupInfoV2>
    for Map<'_, (&Addr, &Addr, U64Key), LockupInfoV2>
{
    fn compatible_load(
        &self,
        deps: Deps,
        key: (&Addr, &Addr, U64Key),
        generator: &Option<Addr>,
    ) -> StdResult<LockupInfoV2> {
        self.load(deps.storage, key.clone()).or_else(|_| {
            let old_lockup_info = OLD_LOCKUP_INFO.load(deps.storage, key.clone())?;
            let mut generator_proxy_debt = RestrictedVector::default();
            let generator = generator.as_ref().expect("Generator should be set!");

            if !old_lockup_info.generator_proxy_debt.is_zero() {
                let asset = ASSET_POOLS.load(deps.storage, key.0)?;
                let astro_lp = asset
                    .migration_info
                    .expect("Pool should be migrated!")
                    .astroport_lp_token;
                let pool_info: PoolInfoResponse = deps.querier.query_wasm_smart(
                    generator,
                    &GenQueryMsg::PoolInfo {
                        lp_token: astro_lp.to_string(),
                    },
                )?;
                let (proxy, _) = pool_info
                    .accumulated_proxy_rewards_per_share
                    .first()
                    .ok_or_else(|| {
                        StdError::generic_err(format!("Proxy rewards not found: {}", astro_lp))
                    })?;
                let reward_asset = raw_proxy_asset(deps.querier, generator, proxy.as_bytes())?;

                generator_proxy_debt.update(&reward_asset, old_lockup_info.generator_proxy_debt)?;
            }

            let lockup_info = LockupInfoV2 {
                lp_units_locked: old_lockup_info.lp_units_locked,
                astroport_lp_transferred: old_lockup_info.astroport_lp_transferred,
                withdrawal_flag: old_lockup_info.withdrawal_flag,
                astro_rewards: old_lockup_info.astro_rewards,
                generator_astro_debt: old_lockup_info.generator_astro_debt,
                generator_proxy_debt,
                unlock_timestamp: old_lockup_info.unlock_timestamp,
            };

            Ok(lockup_info)
        })
    }

    fn compatible_may_load(
        &self,
        deps: Deps,
        key: (&Addr, &Addr, U64Key),
        generator: &Option<Addr>,
    ) -> StdResult<Option<LockupInfoV2>> {
        if !OLD_LOCKUP_INFO.has(deps.storage, key.clone()) {
            return Ok(None);
        }
        Some(self.compatible_load(deps, key, generator)).transpose()
    }
}

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
    pub lockdrop_incentives: Uint128,
    /// Max lockup positions a user can have
    pub max_positions_per_user: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    /// Total ASTRO incentives share
    pub total_incentives_share: u64,
    /// ASTRO Tokens delegated to the bootstrap auction contract
    pub total_astro_delegated: Uint128,
    /// Boolean value indicating if the user can withdraw their ASTRO rewards or not
    pub are_claims_allowed: bool,
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
    pub generator_proxy_per_share: RestrictedVector<AssetInfo, Decimal>,
    /// Boolean value indicating if the LP Tokens are staked with the Generator contract or not
    pub is_staked: bool,
    /// Flag defines whether the asset has rewards or not
    pub has_asset_rewards: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct UserInfo {
    /// Total ASTRO tokens user received as rewards for participation in the lockdrop
    pub total_astro_rewards: Uint128,
    /// Total ASTRO tokens user delegated to the LP bootstrap auction pool
    pub delegated_astro_rewards: Uint128,
    /// ASTRO tokens transferred to user
    pub astro_transferred: bool,
    /// Number of lockup positions the user is having
    pub lockup_positions_index: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LockupInfoV1 {
    /// Terraswap LP units locked by the user
    pub lp_units_locked: Uint128,
    pub astroport_lp_transferred: Option<Uint128>,
    /// Boolean value indicating if the user's has withdrawn funds post the only 1 withdrawal limit cutoff
    pub withdrawal_flag: bool,
    /// ASTRO tokens received as rewards for participation in the lockdrop
    pub astro_rewards: Uint128,
    /// Generator ASTRO tokens loockup received as generator rewards
    pub generator_astro_debt: Uint128,
    /// Generator Proxy tokens lockup received as generator rewards
    pub generator_proxy_debt: Uint128,
    /// Timestamp beyond which this position can be unlocked
    pub unlock_timestamp: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LockupInfoV2 {
    /// Terraswap LP units locked by the user
    pub lp_units_locked: Uint128,
    pub astroport_lp_transferred: Option<Uint128>,
    /// Boolean value indicating if the user's has withdrawn funds post the only 1 withdrawal limit cutoff
    pub withdrawal_flag: bool,
    /// ASTRO tokens received as rewards for participation in the lockdrop
    pub astro_rewards: Uint128,
    /// Generator ASTRO tokens loockup received as generator rewards
    pub generator_astro_debt: Uint128,
    /// Generator Proxy tokens lockup received as generator rewards
    pub generator_proxy_debt: RestrictedVector<AssetInfo, Uint128>,
    /// Timestamp beyond which this position can be unlocked
    pub unlock_timestamp: u64,
}

pub const OWNERSHIP_PROPOSAL: Item<OwnershipProposal> = Item::new("ownership_proposal");
