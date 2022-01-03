use cosmwasm_std::{Addr, Uint128};
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
    /// Merkle roots used to verify is a terra user is eligible for the airdrop
    pub merkle_roots: Vec<String>,
    /// Timestamp since which ASTRO airdrops can be delegated to bootstrap auction contract
    pub from_timestamp: u64,
    /// Timestamp to which ASTRO airdrops can be claimed
    pub to_timestamp: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct State {
    /// Total ASTRO issuance used as airdrop incentives
    pub total_airdrop_size: Uint128,
    /// Total ASTRO tokens that are yet to be claimed by the users
    pub unclaimed_tokens: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserInfo {
    /// Total ASTRO airdrop tokens claimable by the user
    pub airdrop_amount: Uint128,
}

impl Default for UserInfo {
    fn default() -> Self {
        UserInfo {
            airdrop_amount: Uint128::zero(),
        }
    }
}
