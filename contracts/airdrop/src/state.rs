use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::{Addr};
use cw_storage_plus::{Item, Map};

pub const CONFIG: Item<Config> = Item::new("config");
pub const CLAIMEES: Map< &[u8], IsClaimed> = Map::new("claimed");

//----------------------------------------------------------------------------------------
// Storage types
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Config {
    pub owner: Addr,
    pub astro_token_address: Addr,
    pub terra_merkle_roots: Vec<String>,
    pub evm_merkle_roots: Vec<String>,
    pub from_timestamp: u64, 
    pub till_timestamp: u64, 
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct IsClaimed {
    pub is_claimed: bool
}

impl Default for IsClaimed {
    fn default() -> Self {
        IsClaimed {
            is_claimed: false
        }
    }
}
