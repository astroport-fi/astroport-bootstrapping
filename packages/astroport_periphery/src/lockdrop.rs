use cosmwasm_std::{Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};



#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    // /// Admin function to update the configuration parameters
    // UpdateConfig {
    //     new_config: InstantiateMsg,
    // },
    EnableClaims {},
    /// Allows Terra users to claim their ASTRO Airdrop 
    ClaimByTerraUser {
        claim_amount: Uint128,
        merkle_proof: Vec<String>,
        root_index: u32
    },
    /// Allows EVM users to claim their ASTRO Airdrop 
    ClaimByEvmUser {
        eth_address: String,
        claim_amount: Uint128,
        merkle_proof: Vec<String>,
        root_index: u32,
        signature: String,
        signed_msg_hash: String
        
    },
    /// Allows users to delegate their ASTRO tokens to the LP Bootstrap auction contract 
    DelegateAstroToBootstrapAuction {
        amount_to_delegate: Uint128
    },
    /// Allows users to withdraw their ASTRO tokens 
    WithdrawAirdropReward { },
    /// Admin function to facilitate transfer of the unclaimed ASTRO Tokens
    TransferUnclaimedTokens {
        recepient: String,
        amount: Uint128,
    },
}
