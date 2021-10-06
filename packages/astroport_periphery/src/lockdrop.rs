use cosmwasm_std::{Uint128, to_binary, Addr, CosmosMsg, StdResult, WasmMsg};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_bignumber::{Decimal256, Uint256};
use cw20::Cw20ReceiveMsg;
use crate::asset::{Asset, AssetInfo, NativeAsset, Cw20Asset, LiquidityPool};


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    /// Account who can update config
    pub owner: String,
    /// Bootstrap Auction contract address
    pub auction_contract_address: Option<String>,
    /// Generator (Staking for dual rewards) contract address
    pub generator_address: Option<String>,
    /// Astroport token address
    pub astro_token_address: Option<String>,
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
    pub weekly_multiplier: Decimal256,
    /// Total MARS lockdrop incentives to be distributed among the users
    pub lockdrop_incentives: Uint256,
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UpdateConfigMsg {
    /// Account who can update config
    pub owner: Option<String>,
    /// Bootstrap Auction contract address
    pub auction_contract_address: Option<String>,
    /// Generator (Staking for dual rewards) contract address
    pub generator_address: Option<String>,
    /// Astroport token address
    pub astro_token_address: Option<String>,
    /// Total MARS lockdrop incentives to be distributed among the users
    pub lockdrop_incentives: Option<Uint256>,
}




#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    // Receive hook used to accept LP Token deposits
    Receive(Cw20ReceiveMsg),
    // ADMIN Function ::: To update configuration
    UpdateConfig {
        new_config: UpdateConfigMsg,
    },
    EnableClaims {},
    // ADMIN Function ::: Add new Pool (Only Terraswap Pools)
    InitializePool {
        terraswap_pool: LiquidityPool,
        incentives_percent: Option<Decimal256>
    },

    // Function to facilitate LP Token withdrawals from lockups
    WithdrawFromLockup {
        pool_identifer: String,
        duration: u64,
        amount: Uint256
    },

    // ADMIN Function ::: To Migrate liquidity from terraswap to astroport
    MigrateLiquidity {
        pool_identifer: String,
        astroport_pool_address: String,
        astroport_lp_address: String
    },
    // // ADMIN Function ::: To stake LP Tokens with the guage generator contract
    StakeLpTokens { 
        pool_identifer: String,
    },
    // Delegate ASTRO to Bootstrap via auction contract
    DelegateAstroToAuction {
        amount: Uint256
    },
    // Facilitates ASTRO reward withdrawal which have not been delegated to bootstrap auction
    ClaimRewardsForLockup { 
        pool_identifer: String,
        duration: u64
    },
    // // Unlocks a lockup position whose lockup duration has concluded
    // UnlockPosition { 
    //     pool_identifer: String,
    //     duration: u64
    //  },
    // // Unlocks a lockup position whose lockup duration has not concluded. user needs to approve ASTRO Token to
    // // be transferred by the lockdrop contract before calling this function
    // ForceUnlockPosition { 
    //     pool_identifer: String,
    //     duration: u64
    //  },
    /// Callbacks; only callable by the contract itself.
    Callback(CallbackMsg)
}




#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    /// Open a new user position or add to an existing position (Cw20ReceiveMsg)
    IncreaseLockup {   duration: u64  }
}
 


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CallbackMsg {
    UpdatePoolOnDualRewardsClaim {
        pool_identifer: String,
        prev_astro_balance: Uint256,
        prev_dual_reward_balance: Uint256,
    },
    WithdrawUserLockupRewardsCallback {
        user_address: Addr,
        pool_identifer: String,
        duration: u64,
        withdraw_lp_stake: bool
    },
    WithdrawLiquidityFromTerraswapCallback {
        pool_identifer: String,
        native_asset: NativeAsset,
        native_asset_balance: Uint128,
        cw20_asset: Cw20Asset,
        cw20_asset_balance: Uint128,
        astroport_pool: LiquidityPool,
    },
    UpdateStateLiquidityMigrationCallback {
        pool_identifer: String,
        astroport_pool: LiquidityPool,
        astroport_lp_balance: Uint128 
    }

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




// #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
// #[serde(rename_all = "snake_case")]
// pub enum QueryMsg {
//     Config {},
//     State {},
//     Pool { pool_identifier: String },
//     UserInfo { address: String },
//     LockUpInfo { user_address: String, pool_identifier: String, duration: u64 },
//     LockUpInfoWithId { lockup_id: String },
// }

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    /// Account who can update config
    pub owner: String,
    /// Bootstrap Auction contract address
    pub auction_contract_address: String,
    /// Generator (ASTRO-UST Staking) contract address
    pub generator_address: String,
    /// ASTRO Token address
    pub astro_token_address: String,
    /// Timestamp when Contract will start accepting LP Token deposits
    pub init_timestamp: u64,
    /// Deposit Window Length
    pub deposit_window: u64,
    /// Withdrawal Window Length :: Post the deposit window
    pub withdrawal_window: u64,
    /// Min. no. of weeks allowed for lockup
    pub min_lock_duration: u64,
    /// Max. no. of weeks allowed for lockup
    pub max_lock_duration: u64,
    /// Number of seconds per week
    pub seconds_per_week: u64,
    /// Lockdrop Reward multiplier
    pub weekly_multiplier: Decimal256,
    /// Total ASTRO lockdrop incentives to be distributed among the users
    pub lockdrop_incentives: Uint256,

}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StateResponse {
    /// ASTRO Tokens delegated to the bootstrap auction contract
    pub total_astro_delegated: Uint256,
    /// ASTRO returned to forcefully unlock Lockup positions
    pub total_astro_returned: Uint256,
    /// Boolean value indicating if the user can withdraw thier ASTRO rewards or not
    pub are_claims_allowed: bool,
    /// Vector containing LP addresses for all the supported LP Pools
    pub supported_pairs_list: Vec<String>,
}



#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolResponse {
    /// Terraswap Pool Details
    pub terraswap_pair: LiquidityPool,
    /// Astroport Pool Details
    pub astroport_pair: LiquidityPool,
    /// Pair's cw20 token 
    pub cw20_asset: Cw20Asset,
    /// Pair's Native token (uusd/uluna)
    pub native_asset: NativeAsset,    
    /// % of total ASTRO incentives allocated to this pool
    pub incentives_percent: Decimal256,
    /// Boolean value indicating if the LP Tokens are staked with the Generator contract or not
    pub is_staked: bool,
    /// Boolean value indicating if the liquidity has been migrated or not 
    pub is_migrated: bool,
    /// Weighted LP Token balance used to calculate ASTRO rewards a particular user can claim
    pub weighted_amount: Uint256,
    /// Ratio of ASTRO rewards accured to total_lp_deposited. Used to calculate ASTRO incentives accured by each user
    pub astro_global_reward_index: Decimal256,
    /// Ratio of ASSET rewards accured to total_lp_deposited. Used to calculate ASSET incentives accured by each user
    pub asset_global_reward_index: Decimal256,
}




#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserInfoResponse {
    /// Total ASTRO tokens user received as rewards for participation in the lockdrop
    pub total_astro_rewards: Uint256,
    /// Total ASTRO tokens user delegated to the LP bootstrap auction pool 
    pub delegated_astro_rewards: Uint256,
    /// Contains lockup Ids of the User's lockup positions with different pools having different durations / deposit amounts
    pub lockup_positions: Vec<String>,
}



#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LockUpInfoResponse {
    /// LP Pool identifer whose LP tokens this Lockdrop position accounts for 
    pub pool_identifier: String,
    /// Lockup Duration
    pub duration: u64,
    /// UST locked as part of this lockup position
    pub lp_units_locked: Uint256,
    /// UST locked as part of this lockup position
    pub astro_rewards: Uint256,
    /// Boolean value indicating if the user's LP units have been updated post liquidity migration
    pub is_migrated: bool,
    /// Boolean value indicating if the user's has withdrawn funds post the only 1 withdrawal limit cutoff
    pub withdrawal_counter: bool,
    /// Timestamp beyond which this position can be unlocked
    pub unlock_timestamp: u64,
    /// Used to calculate user's pending ASTRO rewards from the generator (staking) contract 
    pub astro_reward_index: Decimal256,
    /// Used to calculate user's pending DUAL rewards from the generator (staking) contract 
    pub dual_reward_index: Decimal256,
}











#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct WithdrawalStatus {
    pub max_withdrawal_percent: Decimal256,
    pub update_withdrawal_counter: bool,
}



