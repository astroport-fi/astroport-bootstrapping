use cosmwasm_std::{Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: Option<String>,
    pub astro_token_address: Option<String>,
    pub terra_merkle_roots: Option<Vec<String>>,
    pub evm_merkle_roots: Option<Vec<String>>,    
    pub from_timestamp: Option<u64>,
    pub till_timestamp: Option<u64>
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    UpdateConfig {
        new_config: InstantiateMsg,
    },
    ClaimByTerraUser {
        claim_amount: Uint128,
        merkle_proof: Vec<String>,
        root_index: u32
    },
    ClaimByEvmUser {
        eth_address: String,
        claim_amount: Uint128,
        merkle_proof: Vec<String>,
        root_index: u32,
        signature: String,
        signed_msg_hash: String
        
    },
    TransferAstroTokens {
        recepient: String,
        amount: Uint128,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    IsClaimed {
        address: String,
     },
     IsValidSignature {
        evm_address: String,
        evm_signature: String,
        signed_msg_hash: String,                
     },
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub astro_token_address: String,
    pub terra_merkle_roots: Vec<String>,
    pub evm_merkle_roots: Vec<String>,    
    pub from_timestamp: u64,
    pub till_timestamp: u64
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ClaimResponse {
    pub is_claimed: bool,
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct SignatureResponse {
    pub is_valid: bool,
    pub public_key: String,
    pub recovered_address: String
}
