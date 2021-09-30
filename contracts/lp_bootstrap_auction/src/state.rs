use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::{Addr, Uint128, Decimal};
use cw_storage_plus::{Item, Map};

pub const CONFIG: Item<Config> = Item::new("config");
pub const STATE: Item<State> = Item::new("state");
pub const USERS: Map< &Addr, UserInfo> = Map::new("users");

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
    ///  ASTRO-UST LP Pool address
    pub astroport_lp_pool: Addr,
    ///  ASTRO-UST LP Token address
    pub lp_token_address: Addr,
    ///  ASTRO-UST LP Tokens staking contract address
    pub lp_staking_contract: Addr,
    /// ASTRO token rewards to be used to incentivize boostrap auction participants
    pub astro_rewards: u64, 
    /// Timestamp from which ASTRO / UST can be deposited in the boostrap auction contract 
    pub init_timestamp: u64, 
    /// Number of seconds post init_timestamp during which deposits will be allowed 
    pub deposit_window: u64, 
    /// Number of seconds post deposit_window completion during which withdrawals will be allowed 
    pub withdrawal_window: u64,     
}



#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct State {
    /// Total ASTRO tokens delegated to the contract by lockdrop participants / airdrop recepients
    pub total_astro_deposited: Uint128, 
    /// Total UST deposited in the contract
    pub total_ust_deposited: Uint128, 
    /// Total LP shares minted post liquidity addition to the ASTRO-UST Pool
    pub total_lp_shares_minted: Uint128 
    /// index used to keep track of LP staking rewards and distribute them proportionally among the auction participants
    pub global_reward_index: Decimal 
}


impl Default for State {
    fn default() -> Self {
        State {
            total_astro_deposited: Uint128::zero(),
            total_ust_deposited: Uint128::zero(),
            total_lp_shares_minted: Uint128::zero(),
            global_reward_index: Decimal::zero()
        }
    }
}




#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserInfo {
    pub astro_delegated: Uint128,
    pub ust_deposited: Uint128,
    pub lp_shares: Uint128,
    pub auction_astro_incentives: Uint128,
    pub user_reward_index: Decimal,
}

impl Default for UserInfo {
    fn default() -> Self {
        UserInfo {
            astro_delegated: Uint128::zero(),
            ust_deposited: Uint128::zero(),
            lp_shares: Uint128::zero(),
            auction_astro_incentives: Uint128::zero(),
            user_reward_index: Decimal::zero(),
        }
    }
}


