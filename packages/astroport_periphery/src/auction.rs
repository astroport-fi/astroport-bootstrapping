use cosmwasm_std::{to_binary, Addr, CosmosMsg, Decimal, Env, StdResult, Uint128, WasmMsg};
use cw20::Cw20ReceiveMsg;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: Option<String>,
    pub astro_token_address: String,
    pub airdrop_contract_address: String,
    pub lockdrop_contract_address: String,
    pub lp_tokens_vesting_duration: u64,
    pub init_timestamp: u64,
    pub deposit_window: u64,
    pub withdrawal_window: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UpdateConfigMsg {
    pub owner: Option<String>,
    pub astro_ust_pair_address: Option<String>,
    pub generator_contract: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct PoolInfo {
    ///  ASTRO-UST LP Pool address
    pub astro_ust_pool_address: Addr,
    ///  ASTRO-UST LP Token address
    pub astro_ust_lp_token_address: Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),
    UpdateConfig { new_config: UpdateConfigMsg },

    DepositUst {},
    WithdrawUst { amount: Uint128 },

    InitPool { slippage: Option<Decimal> },
    StakeLpTokens {},

    ClaimRewards { withdraw_lp_shares: Option<Uint128> },
    Callback(CallbackMsg),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    DelegateAstroTokens { user_address: String },
    IncreaseAstroIncentives {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CallbackMsg {
    UpdateStateOnRewardClaim {
        prev_astro_balance: Uint128,
    },
    UpdateStateOnLiquidityAdditionToPool {
        prev_lp_balance: Uint128,
    },
    WithdrawUserRewardsCallback {
        user_address: Addr,
        withdraw_lp_shares: Option<Uint128>,
    },
}

// Modified from
// https://github.com/CosmWasm/cosmwasm-plus/blob/v0.2.3/packages/cw20/src/receiver.rs#L15
impl CallbackMsg {
    pub fn to_cosmos_msg(&self, env: &Env) -> StdResult<CosmosMsg> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
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
    pub owner: Addr,
    pub astro_token_address: Addr,
    pub airdrop_contract_address: Addr,
    pub lockdrop_contract_address: Addr,
    pub pool_info: Option<PoolInfo>,
    pub generator_contract: Option<Addr>,
    pub astro_incentive_amount: Option<Uint128>,
    pub lp_tokens_vesting_duration: u64,
    pub init_timestamp: u64,
    pub deposit_window: u64,
    pub withdrawal_window: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StateResponse {
    pub total_astro_delegated: Uint128,
    pub total_ust_delegated: Uint128,
    pub is_lp_staked: bool,
    pub lp_shares_minted: Option<Uint128>,
    pub pool_init_timestamp: u64,
    pub generator_astro_per_share: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserInfoResponse {
    pub astro_delegated: Uint128,
    pub ust_delegated: Uint128,
    pub ust_withdrawn: bool,
    pub lp_shares: Option<Uint128>,
    pub claimed_lp_shares: Uint128,
    pub withdrawable_lp_shares: Option<Uint128>,
    pub auction_incentive_amount: Option<Uint128>,
    pub astro_incentive_transferred: bool,
    pub claimable_generator_astro: Uint128,
    pub generator_astro_debt: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}
