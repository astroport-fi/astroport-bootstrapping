use astroport_periphery::lockdrop::MigrationInfo;
use cosmwasm_std::{Addr, Decimal, Uint128, Uint256};
use cw_storage_plus::Map;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolInfoV101 {
    pub terraswap_pool: Addr,
    pub terraswap_amount_in_lockups: Uint128,
    pub migration_info: Option<MigrationInfo>,
    pub incentives_share: u64,
    pub weighted_amount: Uint256,
    pub generator_astro_per_share: Decimal,
    pub generator_proxy_per_share: Decimal,
    pub is_staked: bool,
}

pub const ASSET_POOLS_V101: Map<&Addr, PoolInfoV101> = Map::new("LiquidityPools");
