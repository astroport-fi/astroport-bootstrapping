use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::{to_binary, Addr, CosmosMsg, Decimal, StdResult, WasmMsg};
use cw20::Cw20ReceiveMsg;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub astro_token_address: String,
    pub airdrop_contract_address: String,
    pub lockdrop_contract_address: String,
    pub astroport_lp_pool: Option<String>,
    pub lp_token_address: Option<String>,
    pub generator_contract: Option<String>,
    pub astro_rewards: Uint256,
    pub astro_vesting_duration: u64,
    pub lp_tokens_vesting_duration: u64,
    pub init_timestamp: u64,
    pub deposit_window: u64,
    pub withdrawal_window: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UpdateConfigMsg {
    pub owner: Option<String>,
    pub astroport_lp_pool: Option<String>,
    pub lp_token_address: Option<String>,
    pub generator_contract: Option<String>,
    pub astro_rewards: Option<Uint256>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),
    UpdateConfig { new_config: UpdateConfigMsg },

    DepositUst {},
    WithdrawUst { amount: Uint256 },

    AddLiquidityToAstroportPool { slippage: Option<Decimal> },
    StakeLpTokens {},

    ClaimRewards {},
    WithdrawLpShares {},
    Callback(CallbackMsg),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    DelegateAstroTokens { user_address: Addr },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CallbackMsg {
    UpdateStateOnRewardClaim {
        user_address: Addr,
        prev_astro_balance: Uint256,
    },
    UpdateStateOnLiquidityAdditionToPool {
        prev_lp_balance: Uint256,
    },
}

// Modified from
// https://github.com/CosmWasm/cosmwasm-plus/blob/v0.2.3/packages/cw20/src/receiver.rs#L15
impl CallbackMsg {
    pub fn to_cosmos_msg(&self, contract_addr: &Addr) -> StdResult<CosmosMsg> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: String::from(contract_addr),
            msg: to_binary(&ExecuteMsg::Callback(self.clone()))?,
            funds: vec![],
        }))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    State {},
    UserInfo { address: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub astro_token_address: String,
    pub airdrop_contract_address: String,
    pub lockdrop_contract_address: String,
    pub astroport_lp_pool: String,
    pub lp_token_address: String,
    pub generator_contract: String,
    pub astro_rewards: Uint256,
    pub init_timestamp: u64,
    pub deposit_window: u64,
    pub withdrawal_window: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StateResponse {
    pub total_astro_delegated: Uint256,
    pub total_ust_deposited: Uint256,
    pub lp_shares_minted: Uint256,
    pub lp_shares_claimed: Uint256,
    pub are_staked: bool,
    pub pool_init_timestamp: u64,
    pub global_reward_index: Decimal256,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserInfoResponse {
    pub astro_delegated: Uint256,
    pub ust_deposited: Uint256,
    pub lp_shares: Uint256,
    pub claimed_lp_shares: Uint256,
    pub claimable_lp_shares: Uint256,
    pub total_auction_incentives: Uint256,
    pub claimed_auction_incentives: Uint256,
    pub claimable_auction_incentives: Uint256,
    pub user_reward_index: Decimal256,
    pub claimable_staking_incentives: Uint256,
}
