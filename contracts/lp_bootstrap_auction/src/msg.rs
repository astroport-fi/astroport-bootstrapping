use cosmwasm_std::{Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: Option<String>,
    pub astro_token_address: String,
    pub airdrop_contract_address: String,
    pub lockdrop_contract_address: String,
    pub astroport_lp_pool: Option<String>,
    pub lp_staking_contract: Option<String>,
    pub astro_rewards: Uint128,
    pub init_timestamp: u64,
    pub deposit_window: u64,
    pub withdrawal_window: u64
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    UpdateConfig {
        new_config: InstantiateMsg,
    },

    DelegateAstroTokens { 
        user_address: String, 
        amount: Uint128 
    } 
    DepositUst { },
    WithdrawUst { amount: Uint128 },

    AddLiquidityToAstroportPool { },

    ClaimRewards { },
    WithdrawLpShares { amount: Uint128 },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    State {},
    UserInfo {
        address: String,
     },
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub astro_token_address: String,
    pub terra_merkle_roots: Vec<String>,
    pub evm_merkle_roots: Vec<String>,    
    pub from_timestamp: u64,
    pub till_timestamp: u64,
    pub boostrap_auction_address: String,
    pub are_claims_allowed: bool
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StateResponse {
    pub total_astro_deposited: Uint128,
    pub total_ust_deposited: Uint128,
    pub total_lp_shares_minted: Uint128,
    pub global_reward_index: Decimal
}
}



#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserInfoResponse {
    pub astro_delegated: Uint128,
    pub ust_deposited: Uint128,
    pub lp_shares: Uint128,
    pub total_auction_incentives: Uint128,
    pub unclaimed_auction_incentives: Uint128,
    pub user_reward_index: Decimal,
    pub unclaimed_staking_rewards: Uint128
}




#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct WithdrawalStatus {
    pub max_withdrawal_percent: Decimal,
    pub update_withdrawal_counter: bool,
}

