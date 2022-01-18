use cosmwasm_std::Uint128;
use cw20::Cw20ReceiveMsg;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: Option<String>,
    pub astro_token_address: String,
    pub merkle_roots: Option<Vec<String>>,
    pub from_timestamp: Option<u64>,
    pub to_timestamp: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),
    /// Admin function to update the configuration parameters
    UpdateConfig {
        owner: Option<String>,
        merkle_roots: Option<Vec<String>>,
        from_timestamp: Option<u64>,
        to_timestamp: Option<u64>,
    },
    /// Allows Terra users to claim their ASTRO Airdrop
    Claim {
        claim_amount: Uint128,
        merkle_proof: Vec<String>,
        root_index: u32,
    },
    TransferUnclaimedTokens {
        recipient: String,
        amount: Uint128,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    IncreaseAstroIncentives {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    State {},
    UserInfo { address: String },
    HasUserClaimed { address: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub astro_token_address: String,
    pub merkle_roots: Vec<String>,
    pub from_timestamp: u64,
    pub to_timestamp: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StateResponse {
    pub total_airdrop_size: Uint128,
    pub unclaimed_tokens: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserInfoResponse {
    pub airdrop_amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ClaimResponse {
    pub is_claimed: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}
