use cosmwasm_std::{to_binary, Addr, CosmosMsg, StdResult, WasmMsg};

use cosmwasm_bignumber::{Decimal256, Uint256};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    /// Account who can update config
    pub owner: String,
    /// Bootstrap Auction contract address
    pub auction_contract_address: String,
    /// Timestamp when Contract will start accepting LP Token deposits
    pub init_timestamp: u64,
    /// Number of seconds for which lockup deposits will be accepted
    pub deposit_window: u64,
    /// Withdrawal Window Length :: Post the deposit window
    pub withdrawal_window: u64,
    /// Min. no. of days allowed for lockup
    pub min_duration: u64,
    /// Max. no. of days allowed for lockup
    pub max_duration: u64,
    /// Number of seconds per week 
    pub seconds_per_week: u64,
    /// Lockdrop Reward multiplier
    pub weekly_multiplier: Option<Decimal256>,
    /// Total MARS lockdrop incentives to be distributed among the users
    pub lockdrop_incentives: Option<Uint256>,
}

// #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
// pub struct UpdateConfigMsg {
//     /// Account who can update config
//     pub owner: Option<String>,
//     /// Contract used to query addresses related to red-bank (MARS Token)
//     pub address_provider: Option<String>,
//     ///  maUST token address - Minted upon UST deposits into red bank
//     pub ma_ust_token: Option<String>,
//     /// Timestamp till when deposits can be made
//     pub init_timestamp: Option<u64>,
//     /// Number of seconds for which lockup deposits will be accepted
//     pub deposit_window: Option<u64>,
//     /// Number of seconds for which lockup withdrawals will be allowed
//     pub withdrawal_window: Option<u64>,
//     /// Min. no. of days allowed for lockup
//     pub min_duration: Option<u64>,
//     /// Max. no. of days allowed for lockup
//     pub max_duration: Option<u64>,
//     /// Lockdrop Reward multiplier
//     pub weekly_multiplier: Option<Decimal256>,
//     /// Total MARS lockdrop incentives to be distributed among the users
//     pub lockdrop_incentives: Option<Uint256>,
// }

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    // Receive hook used to accept LP Token deposits
    ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
    // ADMIN Function ::: To update configuration
    UpdateConfig {
        new_config: UpdateConfigMsg,
    },
    // ADMIN Function ::: Add new Pool
    InitializePool {
        lp_token_addr: String,
        pool_addr: String,
        incentives_percent: Decimal256,
        pool_type: PoolType
    }

    // Function to facilitate LP Token withdrawals from lockups
    WithdrawFromLockup {
        duration: u64,
        amount: Uint256,
    },

    // ADMIN Function ::: To Migrate liquidity from terraswap to astroport
    MigrateLiquidity {
        lp_token_address: String,
        astroport_pool_address: String,
        astroport_lp_address: String
    }
    // ADMIN Function ::: To stake LP Tokens with the guage generator contract
    StakeLpTokens { 
        lp_token_address: String,
    }
    // ADMIN Function ::: To unstake LP Tokens with the guage generator contract
    UnstakeLpTokens { 
        lp_token_address: String,
    }

    // Delegate ASTRO to Bootstrap via auction contract
    DelegateAstroToAuction {
        amount: Uint256
    },
    // Facilitates ASTRO reward withdrawal which have not been delegated to bootstrap auction
    WithdrawAstroRewards {},
    // Facilitates ASSET reward withdrawal which have not been delegated to bootstrap auction
    WithdrawAssetRewards {},
    // Unlocks a lockup position whose lockup duration has not concluded. user needs to approve ASTRO Token to
    // be transferred by the lockdrop contract before calling this function
    ForceUnlockPosition { 
        lp_token_address: String,
        duration: u64
     },
    // Unlocks a lockup position whose lockup duration has concluded
    UnlockPosition { 
        lp_token_address: String,
        duration: u64
     },
    /// Callbacks; only callable by the contract itself.
    Callback(CallbackMsg),
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    /// Open a new user position or add to an existing position (Cw20ReceiveMsg)
    IncreaseLockup { user_address: String,
                    lp_token_addr: String,
                    duration: u64 
                },
}
 




#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CallbackMsg {
    // UpdateStateOnRedBankDeposit {
    //     prev_ma_ust_balance: Uint256,
    // },
    // UpdateStateOnClaim {
    //     user: Addr,
    //     prev_xmars_balance: Uint256,
    // },
    // DissolvePosition {
    //     user: Addr,
    //     duration: u64,
    // },
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
    LockUpInfo { user_address: String, lp_token_address: String, duration: u64 },
    LockUpInfoWithId { lockup_id: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    /// Account who can update config
    pub owner: String,
    /// Contract used to query addresses related to red-bank (MARS Token)
    pub address_provider: String,
    ///  maUST token address - Minted upon UST deposits into red bank
    pub ma_ust_token: String,
    /// Timestamp till when deposits can be made
    pub init_timestamp: u64,
    /// Number of seconds for which lockup deposits will be accepted
    pub deposit_window: u64,
    /// Number of seconds for which lockup withdrawals will be allowed
    pub withdrawal_window: u64,
    /// Min. no. of weeks allowed for lockup
    pub min_duration: u64,
    /// Max. no. of weeks allowed for lockup
    pub max_duration: u64,
    /// Lockdrop Reward multiplier
    pub multiplier: Decimal256,
    /// Total MARS lockdrop incentives to be distributed among the users
    pub lockdrop_incentives: Uint256,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GlobalStateResponse {
    /// Total UST deposited at the end of Lockdrop window. This value remains unchanged post the lockdrop window
    pub final_ust_locked: Uint256,
    /// maUST minted at the end of Lockdrop window upon UST deposit in red bank. This value remains unchanged post the lockdrop window
    pub final_maust_locked: Uint256,
    /// UST deposited in the contract. This value is updated real-time upon each UST deposit / unlock
    pub total_ust_locked: Uint256,
    /// maUST held by the contract. This value is updated real-time upon each maUST withdrawal from red bank
    pub total_maust_locked: Uint256,
    /// Total weighted deposits
    pub total_deposits_weight: Uint256,
    /// Ratio of MARS rewards accured to total_maust_locked. Used to calculate MARS incentives accured by each user
    pub global_reward_index: Decimal256,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserInfoResponse {
    pub total_ust_locked: Uint256,
    pub total_maust_locked: Uint256,
    pub lockup_position_ids: Vec<String>,
    pub is_lockdrop_claimed: bool,
    pub reward_index: Decimal256,
    pub pending_xmars: Uint256,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LockUpInfoResponse {
    /// Lockup Duration
    pub duration: u64,
    /// UST locked as part of this lockup position
    pub ust_locked: Uint256,
    /// MA-UST share
    pub maust_balance: Uint256,
    /// Lockdrop incentive distributed to this position
    pub lockdrop_reward: Uint256,
    /// Timestamp beyond which this position can be unlocked
    pub unlock_timestamp: u64,
}





#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PoolType {
    Terraswap { },
    Astroport { },
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct WithdrawalStatus {
    pub max_withdrawal_percent: Decimal,
    pub update_withdrawal_counter: bool,
}
