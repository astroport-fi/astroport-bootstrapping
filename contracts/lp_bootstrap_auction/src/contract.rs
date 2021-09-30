#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{ 
    attr, Binary, Deps, Api, DepsMut, MessageInfo, Env, Response, 
    StdError, StdResult, Uint128, WasmMsg, to_binary, Addr, CosmosMsg, 
};
use crate::msg::{UserInfoResponse, StateResponse, ConfigResponse, WithdrawalStatus, ExecuteMsg, InstantiateMsg, QueryMsg  } ;
use crate::state::{Config, State, CONFIG, STATE, USERS, CLAIMEES};

use cw20_base::msg::{ExecuteMsg as CW20ExecuteMsg };
use sha3::{ Digest, Keccak256 };
use std::cmp::Ordering;
use std::convert::{TryInto};


//----------------------------------------------------------------------------------------
// Entry points
//----------------------------------------------------------------------------------------


#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate( deps: DepsMut, _env: Env, _info: MessageInfo, msg: InstantiateMsg) -> StdResult<Response> {

    let config = Config {
        owner: deps.api.addr_validate(&msg.owner.unwrap())?,
        astro_token_address: deps.api.addr_validate(&msg.astro_token_address).to_string(),
        airdrop_contract_address: deps.api.addr_validate(&msg.airdrop_contract_address).to_string(),
        lockdrop_contract_address: deps.api.addr_validate(&msg.lockdrop_contract_address).to_string(),
        astroport_lp_pool: deps.api.addr_validate(&msg.astroport_lp_pool.unwrap_or(Addr::unchecked("").to_string() ))?,
        lp_staking_contract: deps.api.addr_validate(&msg.lp_staking_contract.unwrap_or(Addr::unchecked("").to_string() ))?,
        astro_rewards: msg.astro_rewards ,
        init_timestamp: msg.init_timestamp.unwrap_or( _env.block.time.seconds()) ,
        deposit_window: msg.deposit_window,
        withdrawal_window: msg.withdrawal_window,
    };

    let state = State {
        total_astro_deposited: Uint128::zero(),
        total_ust_deposited: Uint128::zero(),
        total_lp_shares_minted: Uint128::zero(),
        global_reward_index: Decimal::zero()
    };


    CONFIG.save(deps.storage, &config)?;
    STATE.save( deps.storage, &state )?;
    Ok(Response::default())
}


#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute( deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg)  -> Result<Response, StdError> {
    match msg {
        ExecuteMsg::UpdateConfig { 
            new_config 
        }  => handle_update_config(deps, info,  new_config),

        ExecuteMsg::DelegateAstroTokens { 
            user_address, 
            amount, 
        } => handle_delegate_astro_tokens(deps, env, info,  user_address, amount),
        ExecuteMsg::DepositUst { } => handle_deposit_ust(deps, env, info ),       
        ExecuteMsg::WithdrawUst { 
            amount
        }  => handle_withdraw_ust(deps, env,  info, amount),

        ExecuteMsg::AddLiquidityToAstroportPool {  }  => handle_add_liquidity_to_astroport_pool(deps, env,  info),

        ExecuteMsg::ClaimRewards {  }  => handle_claim_rewards(deps, env,  info),
        ExecuteMsg::WithdrawLpShares { amount  }  => handle_withdraw_lp_shares(deps, env,  info, amount),
    }
}



#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg,) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State {} => to_binary(&query_state(deps)?),
        QueryMsg::UserInfo { 
            address 
        } => to_binary(&query_user_info(deps, address)?),
    }
}


//----------------------------------------------------------------------------------------
// Handle functions
//----------------------------------------------------------------------------------------



/// @dev Admin function to update Configuration parameters
/// @param new_config : Same as InstantiateMsg struct
// pub fn handle_update_config( deps: DepsMut, info: MessageInfo, new_config: InstantiateMsg ) -> StdResult<Response> { 
//     let mut config = CONFIG.load(deps.storage)?;
    
//     // CHECK :: ONLY OWNER CAN CALL THIS FUNCTION
//     if info.sender != config.owner {    
//         return Err(StdError::generic_err("Only owner can update configuration"));
//     }

//     // UPDATE :: ADDRESSES IF PROVIDED
//     config.owner = option_string_to_addr(deps.api, new_config.owner, config.owner)?;
//     config.astro_token_address = option_string_to_addr(deps.api, new_config.astro_token_address, config.astro_token_address)?;
//     config.boostrap_auction_address = option_string_to_addr(deps.api, new_config.boostrap_auction_address, config.boostrap_auction_address)?;

//     // UPDATE :: VALUES IF PROVIDED
//     config.terra_merkle_roots = new_config.terra_merkle_roots.unwrap_or(config.terra_merkle_roots);
//     config.evm_merkle_roots = new_config.evm_merkle_roots.unwrap_or(config.evm_merkle_roots);
//     config.from_timestamp = new_config.from_timestamp.unwrap_or(config.from_timestamp);
//     config.till_timestamp = new_config.till_timestamp.unwrap_or(config.till_timestamp );

//     CONFIG.save(deps.storage, &config)?;
//     Ok(Response::new().add_attribute("action", "Airdrop::ExecuteMsg::UpdateConfig"))
// }



/// @dev Delegates ASTRO tokens to be used for the LP Bootstrapping via auction. Callable only by Airdrop / Lockdrop contracts
/// @param user_address : User address who is delegating the ASTRO tokens for LP Pool bootstrap via auction
/// @param amount : Number of ASTRO Tokens being delegated
pub fn handle_delegate_astro_tokens( 
    deps: DepsMut, 
    _env: Env, 
    info: MessageInfo,
    user_address: String,  
    amount: Uint128
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let user_account =  deps.api.addr_validate(&user_address)?;
    let mut user_info =  USERS.may_load(deps.storage, &user_account.clone() )?.unwrap_or_default();

    // CHECK :: Lockdrop deposit window open
    if !is_deposit_open(env.block.time.seconds(), &config) {
        return Err(StdError::generic_err("Deposit window closed"));
    }

    // CHECK :: Lockdrop / Airdrop contract is the function call origin point
    if info.sender != config.airdrop_contract_address && info.sender != config.lockdrop_contract_address {
        return Err(StdError::generic_err("Unauthorized"));
    }

    // UPDATE STATE
    state.total_astro_deposited += amount;
    user_info.astro_delegated += amount;

    // SAVE UPDATED STATE
    STATE.save(deps.storage, &state)?;
    USERS.save(deps.storage, &user_account, &user_info)?;

    Ok(Response::new()        
    .add_messages(messages_)    
    .add_attributes(vec![
        attr("action", "Auction::ExecuteMsg::DelegateAstroTokens"),
        attr("user", user_account.to_string() ),
        attr("astro_delegated", amount)
    ]))
}




/// @dev Facilitates UST deposits by users to be used for LP Bootstrapping via auction
pub fn handle_deposit_ust( 
    deps: DepsMut,
    _env: Env, 
    info: MessageInfo
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let depositor_address = info.sender.clone();
    let mut user_info =  USERS.may_load(deps.storage, &depositor_address.clone() )?.unwrap_or_default();

    // Retrieve UST sent by the user
    let deposit_amount = get_denom_amount_from_coins(&info.funds,  "uusd".to_string() );

    // CHECK :: Lockdrop deposit window open
    if !is_deposit_open(env.block.time.seconds(), &config) {
        return Err(StdError::generic_err("Deposit window closed"));
    }

    // UPDATE STATE
    state.total_ust_deposited += deposit_amount;
    user_info.deposit_amount += deposit_amount;

    // SAVE UPDATED STATE
    STATE.save(deps.storage, &state)?;
    USERS.save(deps.storage, &user_account, &user_info)?;

    Ok(Response::new()        
    .add_messages(messages_)    
    .add_attributes(vec![
        attr("action", "Auction::ExecuteMsg::DepositUst"),
        attr("user", depositor_address.to_string() ),
        attr("ust_deposited", deposit_amount)
    ]))

}




/// @dev Facilitates UST withdrawals by users from their initial deposited balances
/// @param amount : UST amounf being withdrawn
pub fn handle_withdraw_ust( 
    deps: DepsMut,
    _env: Env, 
    info: MessageInfo,
    amount: Uint128
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let user_address = info.sender.clone();
    let mut user_info =  USERS.may_load(deps.storage, &user_address.clone() )?.unwrap_or_default();

    // Check :: Amount should be withing the allowed withdrawal limit bounds
    let withdrawals_status = calculate_max_withdrawals_allowed(env.block.time.seconds(), &config);
    let max_withdrawal_allowed = user_info.ust_deposited * withdrawals_status.max_withdrawal_percent;
    if amount > max_withdrawal_allowed  {
        return Err(StdError::generic_err("Amount exceeds maximum allowed withdrawal limit"));
    }

    // UPDATE STATE
    state.total_ust_deposited = state.total_ust_deposited - amount;
    user_info.ust_deposited = user_info.ust_deposited - amount;

    // SAVE UPDATED STATE
    STATE.save(deps.storage, &state)?;
    USERS.save(deps.storage, &user_account, &user_info)?;

    Ok(Response::new()        
    .add_messages(messages_)    
    .add_attributes(vec![
        attr("action", "Auction::ExecuteMsg::WithdrawUst"),
        attr("user", user_address.to_string() ),
        attr("ust_withdrawn", amount)
    ]))

}






/// @dev Admin Function to allow users to delegate their ASTRO Tokens to the LP Bootstrap auction contract
/// @param amount_to_delegate Amount of ASTRO to be delegate
pub fn handle_add_liquidity_to_astroport_pool( 
    deps: DepsMut, 
    _env: Env, 
    info: MessageInfo
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    state.tokens_used_for_auction += amount_to_delegate;
    user_info.tokens_used_for_auction += amount_to_delegate;

    // CHECK :: HAS THE BOOTSTRAP AUCTION CONCLUDED ? 
    if config.are_claims_allowed {
        return Err(StdError::generic_err("LP bootstrap auction has concluded"));        
    }

    // CHECK :: TOKENS BEING DELEGATED SHOULD NOT EXCEED USER'S CLAIMABLE AIRDROP AMOUNT
    if user_info.tokens_used_for_auction > user_info.airdrop_amount {
        return Err(StdError::generic_err("Total amount being delegated for boostrap auction cannot exceed your claimable airdrop balance"));        
    }
    
    // COSMOS MSG :: DELEGATE ASTRO TOKENS TO LP BOOTSTRAP AUCTION CONTRACT
    let delegate_msg = build_delegate_tokens_to_auction_contract_msg(deps.api.addr_validate(&info.sender.to_string().clone())? , config.boostrap_auction_address.to_string(), amount_to_delegate.into())?;

    // STATE UPDATE : SAVE UPDATED STATES
    USERS.save(deps.storage, &info.sender , &user_info)?;  
    STATE.save( deps.storage, &state )?;

    Ok(Response::new()
    .add_message(delegate_msg)        
    .add_attributes(vec![
        attr("action", "Airdrop::ExecuteMsg::DelegateAstroToBootstrapAuction"),
        attr("user", info.sender.to_string() ),
        attr("amount_delegated", amount_to_delegate ),
    ]))

}



/// @dev Function to allow users to withdraw their undelegated ASTRO Tokens 
/// @param amount_to_delegate Amount of ASTRO to be delegate
pub fn handle_withdraw_airdrop_rewards( 
    deps: DepsMut, 
    _env: Env, 
    info: MessageInfo
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;
    let mut user_info =  USERS.may_load(deps.storage, &info.sender.clone() )?.unwrap_or_default();

    // CHECK :: HAS THE BOOTSTRAP AUCTION CONCLUDED ? 
    if !config.are_claims_allowed {
        return Err(StdError::generic_err("LP Boostrap auction in progress. Claims not allowed during this period"));        
    }

    // CHECK :: HAS USER ALREADY WITHDRAWN THEIR REWARDS ? 
    if user_info.tokens_claimed > Uint128::zero() {
        return Err(StdError::generic_err("Tokens have already been claimed"));        
    }

    // TRANSFER ASTRO IF CLAIMS ARE ALLOWED (i.e LP Boostrap auction has concluded)
    user_info.tokens_claimed = user_info.airdrop_amount - user_info.tokens_used_for_auction;
    let transfer_msg = build_send_cw20_token_msg(info.sender.clone(), config.astro_token_address.to_string(), user_info.tokens_claimed.into())? ;    

    USERS.save(deps.storage, &info.sender , &user_info)?;  

    Ok(Response::new()
    .add_message(transfer_msg)        
    .add_attributes(vec![
        attr("action", "Airdrop::ExecuteMsg::WithdrawAirdropRewards"),
        attr("user", info.sender.to_string() ),
        attr("amount", user_info.tokens_claimed ),
    ]))
}


 
/// @dev Admin function to transfer ASTRO Tokens to the recepient address
/// @param recepient Recepient receiving the ASTRO tokens
/// @param amount Amount of ASTRO to be transferred
pub fn handle_transfer_unclaimed_tokens( 
    deps: DepsMut, 
    _env: Env, 
    info: MessageInfo,
    recepient: String, 
    amount: Uint128
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // CHECK :: CAN ONLY BE CALLED BY THE OWNER 
    if info.sender != config.owner {
        return Err(StdError::generic_err("Sender not authorized!"));        
    }

    // CHECK :: CAN ONLY BE CALLED AFTER THE CLAIM PERIOD IS OVER 
    if config.till_timestamp > _env.block.time.seconds() {
        return Err(StdError::generic_err("Airdrop claim period has not concluded"));        
    }

    if amount > state.unclaimed_tokens {
        return Err(StdError::generic_err("Amount to transfer cannot exceed unclaimed token balance!"));       
    }



    // COSMOS MSG :: TRANSFER ASTRO TOKENS
    state.unclaimed_tokens = state.unclaimed_tokens - amount;
    let transfer_msg = build_send_cw20_token_msg(deps.api.addr_validate(&recepient.clone())? , config.astro_token_address.to_string(), amount.into())?;

    Ok(Response::new()
    .add_message(transfer_msg)        
    .add_attributes(vec![
        attr("action", "Airdrop::ExecuteMsg::TransferUnclaimedRewards"),
        attr("recepient", recepient.to_string() ),
        attr("amount", amount ),
    ]))

}





//----------------------------------------------------------------------------------------
// Query functions
//----------------------------------------------------------------------------------------


/// @dev Returns the airdrop configuration
fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse { 
        astro_token_address: config.astro_token_address.to_string(),
        owner: config.owner.to_string(),
        terra_merkle_roots: config.terra_merkle_roots, 
        evm_merkle_roots: config.evm_merkle_roots, 
        from_timestamp: config.from_timestamp,
        till_timestamp: config.till_timestamp,
        boostrap_auction_address: config.boostrap_auction_address.to_string(),
        are_claims_allowed: config.are_claims_allowed
    })
}

/// @dev Returns the airdrop contract state
fn query_state(deps: Deps) -> StdResult<StateResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(StateResponse { 
        total_airdrop_size: state.total_airdrop_size,
        tokens_used_for_auction: state.tokens_used_for_auction,
        unclaimed_tokens: state.unclaimed_tokens 
    })
}


/// @dev Returns details around user's ASTRO Airdrop claim
fn query_user_info(deps: Deps, user_address: String) -> StdResult<UserInfoResponse> {
    let user_address = deps.api.addr_validate(&user_address)?;
    let user_info = USERS.may_load(deps.storage, &user_address )?.unwrap_or_default();
    Ok(UserInfoResponse { 
        airdrop_amount: user_info.airdrop_amount,
        tokens_used_for_auction: user_info.tokens_used_for_auction,
        tokens_claimed: user_info.tokens_claimed
    })
}







/// @dev Returns true if the user has claimed the airdrop [EVM addresses to be provided in lower-case without the '0x' prefix]
fn check_user_claimed(deps: Deps, address: String  ) -> StdResult<ClaimResponse> {
    let res = CLAIMEES.may_load(deps.storage, &address.as_bytes() )?.unwrap_or_default();
    Ok(ClaimResponse {  is_claimed: res.is_claimed }) 
}


/// @dev Returns the recovered public key, evm address and a boolean value which is true if the evm address provided was used for signing the message.
/// @param evm_address : EVM address claiming the airdop. Needs to be in lower case without the `0x` prefix
/// @param evm_signature : ECDSA Signature string generated by signing the message (without the `0x` prefix and the last 2 characters which originate from `v`)
/// @param signed_msg_hash : Keccak256 hash of the signed message following the EIP-191 prefix standard.(without the `0x` prefix) 
fn verify_signature(_deps: Deps, evm_address:String, evm_signature: String, signed_msg_hash: String  ) -> StdResult<SignatureResponse> {
    
    let verification_response = handle_verify_signature(_deps.api,  evm_address, evm_signature,  signed_msg_hash);
    
    Ok(SignatureResponse {
        is_valid: verification_response.is_valid,
        public_key: verification_response.public_key,
        recovered_address: verification_response.recovered_address
    }) 

}




//----------------------------------------------------------------------------------------
// HELPERS
//----------------------------------------------------------------------------------------

/// true if deposits are allowed
fn is_deposit_open(current_timestamp: u64, config: &Config) -> bool {
    let deposits_opened_till = config.init_timestamp + config.deposit_window;
    (config.init_timestamp <= current_timestamp) && (current_timestamp <= deposits_opened_till)
}

// native coins
fn get_denom_amount_from_coins(coins: &[Coin], denom: &str) -> Uint256 {
    coins
        .iter()
        .find(|c| c.denom == denom)
        .map(|c| Uint256::from(c.amount))
        .unwrap_or_else(Uint256::zero)
}

/// true if deposits are allowed
fn calculate_max_withdrawals_allowed(current_timestamp: u64, config: &Config) -> WithdrawalStatus {
    let withdrawal_cutoff_init_point = config.init_timestamp + config.deposit_window;
    // 100% withdrawals allowed
    if current_timestamp <= withdrawal_cutoff_init_point {
        return  WithdrawalStatus {
            max_withdrawal_percent: Decimal::from_ratio(100u128, 100u128),
            update_withdrawal_counter: false
        }
    }

    let withdrawal_cutoff_sec_point = withdrawal_cutoff_init_point + (config.withdrawal_window/2u64);
    // 50% withdrawals allowed
    if current_timestamp <= withdrawal_cutoff_sec_point {
        return  WithdrawalStatus {
            max_withdrawal_percent: Decimal::from_ratio(50u128, 100u128),
            update_withdrawal_counter: true
        }
    }

    let withdrawal_cutoff_final = withdrawal_cutoff_sec_point + (config.withdrawal_window/2u64);
    // max withdrawal allowed decreasing linearly from 50% to 0% vs time elapsed
    if current_timestamp < withdrawal_cutoff_final {
        let slope = Decimal::from_ratio( 50u128, (config.withdrawal_window/2u64) );
        let time_elapsed = current_timestamp - withdrawal_cutoff_sec_point;
        return  WithdrawalStatus {
            max_withdrawal_percent: time_elapsed * slope,
            update_withdrawal_counter: true
        }
    }
    // Withdrawals not allowed
    else {
        return  WithdrawalStatus {
            max_withdrawal_percent: Decimal::from_ratio(0u128, 100u128),
            update_withdrawal_counter: true
        }
    }
}











//----------------------------------------------------------------------------------------
// Helper functions
//----------------------------------------------------------------------------------------


/// @dev Verify whether a claim is valid
/// @param account Account on behalf of which the airdrop is to be claimed (etherum addresses without `0x` prefix)
/// @param amount Airdrop amount to be claimed by the user
/// @param merkle_proof Array of hashes to prove the input is a leaf of the Merkle Tree
/// @param merkle_root Hash of Merkle tree's root
fn verify_claim( account: String, amount: Uint128, merkle_proof: Vec<String>, merkle_root: String) -> bool {

    let leaf = account.clone() + &amount.to_string();
    let mut hash_buf  = Keccak256::digest(leaf.as_bytes()).as_slice().try_into().expect("Wrong length");
    let mut hash_str: String; 

    for p in merkle_proof {
        let mut proof_buf: [u8; 32] = [0; 32];
        hex::decode_to_slice(p, &mut proof_buf).unwrap();
        let proof_buf_str = hex::encode(proof_buf);
        hash_str = hex::encode(hash_buf);

        if proof_buf_str.cmp(&hash_str.clone()) == Ordering::Greater {
            hash_buf = Keccak256::digest(&[hash_buf, proof_buf].concat()).as_slice().try_into().expect("Wrong length")
        } else {
            hash_buf = Keccak256::digest(&[proof_buf, hash_buf].concat()).as_slice().try_into().expect("Wrong length")
        }
    }
        
    hash_str = hex::encode(hash_buf);
    
    merkle_root == hash_str
}


/// @dev Verify whether Signature provided is valid
/// @param evm_address : EVM address claiming to have signed the message. Needs to be in lower case without the `0x` prefix
/// @param evm_signature : ECDSA Signature string generated by signing the message (without the `0x` prefix and the last 2 characters which originate from `v`)
/// @param signed_msg_hash : Keccak256 hash of the signed message following the EIP-191 prefix standard.(without the `0x` prefix) 
pub fn handle_verify_signature( api: &dyn Api, evm_address:String, evm_signature: String, signed_msg_hash: String ) -> SignatureResponse {
    let msg_hash = hex::decode(signed_msg_hash).unwrap();
    let signature = hex::decode(evm_signature).unwrap();
    let recovery_param = normalize_recovery_id(signature[63]);

    let mut recovered_public_key = vec![0; 64];
    recovered_public_key = api.secp256k1_recover_pubkey(&msg_hash, &signature, recovery_param ).unwrap_or([].to_vec()) ;
    let recovered_public_key_string = hex::encode(&recovered_public_key );

    let recovered_address = evm_address_raw(&recovered_public_key ).unwrap_or([0;20] ) ;
    let recovered_address_string = hex::encode(recovered_address);
  
    SignatureResponse {
        is_valid: evm_address == recovered_address_string,
        public_key: recovered_public_key_string,
        recovered_address: recovered_address_string 
    }
}


/// Returns a raw 20 byte Ethereum address
/// Copied from https://github.com/CosmWasm/cosmwasm/blob/96a1f888f0cdb7446e29f60054165e635b258e39/contracts/crypto-verify/src/ethereum.rs#L99
pub fn evm_address_raw(pubkey: &[u8]) -> StdResult<[u8; 20]> {
    let (tag, data) = match pubkey.split_first() {
        Some(pair) => pair,
        None => return Err(StdError::generic_err("Public key must not be empty")),
    };
    if *tag != 0x04 {
        return Err(StdError::generic_err("Public key must start with 0x04"));
    }
    if data.len() != 64 {
        return Err(StdError::generic_err("Public key must be 65 bytes long"));
    }

    let hash = Keccak256::digest(data);
    Ok(hash[hash.len() - 20..].try_into().unwrap())
}


/// Normalizes recovery id for recoverable signature while getting the recovery param from the value `v`
/// See [EIP-155] for how `v` is composed.
/// [EIP-155]: https://github.com/ethereum/EIPs/blob/master/EIPS/eip-155.md
/// Copied from https://github.com/gakonst/ethers-rs/blob/01cc80769c291fc80f5b1e9173b7b580ae6b6413/ethers-core/src/types/signature.rs#L142
pub fn normalize_recovery_id(v: u8) -> u8 {
    match v {
        0 => 0,
        1 => 1,
        27 => 0,
        28 => 1,
        v if v >= 35 => ((v - 1) % 2) as _,
        _ => 4,
    }
}


/// @dev Helper function which returns a cosmos wasm msg to transfer cw20 tokens to a recepient address 
/// @param recipient : Address to be transferred cw20 tokens to
/// @param token_contract_address : Contract address of the cw20 token to transfer
/// @param amount : Number of tokens to transfer
fn build_send_cw20_token_msg(recipient: Addr, token_contract_address: String, amount: Uint128) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: token_contract_address,
        msg: to_binary(&CW20ExecuteMsg::Transfer {
            recipient: recipient.into(),
            amount: amount.into(),
        })?,
        funds: vec![],
    }))
}


/// Used when unwrapping an optional address sent in a contract call by a user.
/// Validates addreess if present, otherwise uses a given default value.
pub fn option_string_to_addr( api: &dyn Api, option_string: Option<String>, default: Addr) -> StdResult<Addr> {
    match option_string {
        Some(input_addr) => api.addr_validate(&input_addr),
        None => Ok(default),
    }
}



/// TO DO
fn build_delegate_tokens_to_auction_contract_msg(user_address: Addr, boostrap_auction_address: String, amount_to_delegate: Uint128) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: boostrap_auction_address,
        msg: to_binary(&CW20ExecuteMsg::Transfer {
            recipient: user_address.into(),
            amount: amount_to_delegate.into(),
        })?,
        funds: vec![],
    }))
}



//----------------------------------------------------------------------------------------
// TESTS 
//----------------------------------------------------------------------------------------


// #[cfg(test)]
// mod tests {
//     use super::*;
//     use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR};
//     use cosmwasm_std::{Timestamp,BlockInfo, ContractInfo, attr, Coin, from_binary, OwnedDeps, SubMsg};
//     use crate::state::{CONFIG};
//     use cw20_base::msg::{ExecuteMsg as CW20ExecuteMsg };
//     use crate::msg::{ConfigResponse, InstantiateMsg, QueryMsg  } ;
//     use crate::msg::ExecuteMsg::{UpdateConfig, ClaimByTerraUser , ClaimByEvmUser, TransferUnclaimedTokens};



//     #[test]
//     fn test_proper_initialization() {
//         let mut deps = mock_dependencies(&[]);

//         let terra_merkle_roots = vec!["terra_merkle_roots".to_string()];
//         let evm_merkle_roots = vec![ "evm_merkle_roots".to_string() ];
//         let till_timestamp = 1_000_000_00000;
//         let from_timestamp = 1_000_000_000;

//         // Config with valid base params 
//         let base_config = InstantiateMsg {
//             owner: Some("owner_address".to_string()),
//             astro_token_address: Some("astro_token_contract".to_string()),
//             terra_merkle_roots: Some(terra_merkle_roots.clone()),
//             evm_merkle_roots: Some(evm_merkle_roots.clone()), 
//             from_timestamp: Some(from_timestamp),
//             till_timestamp: Some(till_timestamp)
//         };

//         let info = mock_info("creator");
//         let env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(from_timestamp),
//             ..Default::default()
//         });

//         // we can just call .unwrap() to assert this was a success
//         let res = instantiate(deps.as_mut(), env.clone(), info, base_config).unwrap();
//         assert_eq!(0, res.messages.len());

//         // it worked, let's query the state
//         let res = query(deps.as_ref(), env.clone(), QueryMsg::Config {}).unwrap();
//         let value: ConfigResponse = from_binary(&res).unwrap();

//         assert_eq!("astro_token_contract".to_string(), value.astro_token_address);
//         assert_eq!("owner_address".to_string(), value.owner);
//         assert_eq!(terra_merkle_roots.clone(), value.terra_merkle_roots);
//         assert_eq!(evm_merkle_roots.clone(), value.evm_merkle_roots);
//         assert_eq!(from_timestamp.clone(), value.from_timestamp);
//         assert_eq!(till_timestamp.clone(), value.till_timestamp);
//     }



//     #[test]
//     fn test_update_config() {
//         let mut deps = mock_dependencies(&[]);
//         let env = mock_env(MockEnvParams::default());
//         let not_admin_info = mock_info("not_owner");
//         let mut admin_info = mock_info("owner");
        

//         // Config with valid base params 
//         let base_config = InstantiateMsg {
//             owner: Some("owner".to_string()),
//             astro_token_address: Some("astro_token_contract".to_string()),
//             terra_merkle_roots: Some(vec!["".to_string()]),
//             evm_merkle_roots: Some(vec!["".to_string()]),
//             from_timestamp: Some(1000000000),
//             till_timestamp: Some(1000000000000)
//         };

//         // we can just call .unwrap() to assert this was a success
//         let res = instantiate(deps.as_mut(), env.clone(), admin_info.clone(), base_config).unwrap();
//         assert_eq!(0, res.messages.len());

//         // *** Test updating the owner and the astro token address ***
//         let msg = InstantiateMsg {
//             owner: Some("new_owner".to_string()),
//             astro_token_address:  Some("new_astro_token".to_string()),
//             terra_merkle_roots: None,
//             evm_merkle_roots: None,
//             from_timestamp: None,
//             till_timestamp: None
//         };
//         let mut ex_msg = UpdateConfig {
//             new_config: msg.clone(),
//         };        
        
//         // should fail as only owner can update config
//         let mut res_f = execute(deps.as_mut(), env.clone(), not_admin_info.clone(), ex_msg.clone() );
//         assert_generic_error_message(res_f,"Only owner can update configuration");

//         // should be a success
//         let mut res_s = execute(deps.as_mut(), env.clone(), admin_info.clone(), ex_msg.clone()).unwrap();
//         assert_eq!(0, res_s.messages.len());
//         let mut new_config = CONFIG.load(&deps.storage).unwrap();
//         assert_eq!(new_config.owner, Addr::unchecked("new_owner"));
//         assert_eq!(new_config.astro_token_address, Addr::unchecked("new_astro_token"));
//         assert_eq!( vec!["".to_string()] , new_config.terra_merkle_roots);
//         assert_eq!( vec!["".to_string()] , new_config.evm_merkle_roots);
//         assert_eq!(1000000000, new_config.from_timestamp);
//         assert_eq!(1000000000000, new_config.till_timestamp);


//         // update admin_info to new_owner 
//         admin_info = mock_info("new_owner");

//         // // *** Test updating the merkle roots ***
//         let update_roots_msg = InstantiateMsg {
//             owner: None,
//             astro_token_address: None,
//             terra_merkle_roots: Some( vec!["new_terra_merkle_roots".to_string()] ),
//             evm_merkle_roots: Some( vec!["new_evm_merkle_roots".to_string()] ),
//             from_timestamp: None,
//             till_timestamp: None
//         };
//         ex_msg = UpdateConfig {
//             new_config: update_roots_msg.clone(),
//         };        

//         // should fail as only owner can update config
//         res_f = execute(deps.as_mut(), env.clone(), not_admin_info.clone(), ex_msg.clone() );
//         assert_generic_error_message(res_f,"Only owner can update configuration");

//         // should be a success
//         res_s = execute(deps.as_mut(), env.clone(), admin_info.clone(), ex_msg.clone()).unwrap();
//         assert_eq!(0, res_s.messages.len());
//         new_config = CONFIG.load(&deps.storage).unwrap();
//         assert_eq!(new_config.terra_merkle_roots, vec!["new_terra_merkle_roots".to_string()] );
//         assert_eq!(new_config.evm_merkle_roots, vec!["new_evm_merkle_roots".to_string()] );
//         assert_eq!(new_config.owner, Addr::unchecked("new_owner"));
//         assert_eq!(new_config.astro_token_address, Addr::unchecked("new_astro_token"));
//         assert_eq!(1000000000, new_config.from_timestamp);
//         assert_eq!(1000000000000, new_config.till_timestamp);

//         // *** Test updating timestamps ***
//         let update_timestamps_msg = InstantiateMsg {
//             owner: None,
//             astro_token_address: None,
//             terra_merkle_roots: None,
//             evm_merkle_roots: None,
//             from_timestamp: Some(1_040_000_00000),
//             till_timestamp: Some(1_940_000_00000)
//         };
//         ex_msg = UpdateConfig {
//             new_config: update_timestamps_msg.clone(),
//         };        
        
//         // should fail as only owner can update config
//         res_f = execute(deps.as_mut(), env.clone(), not_admin_info, ex_msg.clone() );
//         assert_generic_error_message(res_f,"Only owner can update configuration");

//         // should be a success
//         res_s = execute(deps.as_mut(), env, admin_info, ex_msg.clone() ).unwrap();
//         assert_eq!(0, res_s.messages.len());
//         new_config = CONFIG.load(&deps.storage).unwrap();
//         assert_eq!(new_config.owner, Addr::unchecked("new_owner"));
//         assert_eq!(new_config.astro_token_address, Addr::unchecked("new_astro_token"));
//         assert_eq!(new_config.from_timestamp, 1_040_000_00000 );
//         assert_eq!(new_config.till_timestamp, 1_940_000_00000 );
//         assert_eq!(new_config.terra_merkle_roots, vec!["new_terra_merkle_roots".to_string()] );
//         assert_eq!(new_config.evm_merkle_roots, vec!["new_evm_merkle_roots".to_string()] );
//     }


//     #[test]
//     fn test_transfer_astro_tokens() {
//         let mut deps = mock_dependencies(&[]);
//         let env = mock_env(MockEnvParams::default());
//         let not_admin_info = mock_info("not_owner");
//         let admin_info = mock_info("owner");
        
//         // Config with valid base params 
//         let base_config = InstantiateMsg {
//             owner: Some("owner".to_string()),
//             astro_token_address: Some("astro_token_contract".to_string()),
//             terra_merkle_roots: Some(vec!["".to_string()]),
//             evm_merkle_roots: Some(vec!["".to_string()]),
//             from_timestamp: Some(1000000000),
//             till_timestamp: Some(1000000000000)
//         };

//         // we can just call .unwrap() to assert this was a success
//         let res = instantiate(deps.as_mut(), env.clone(), admin_info.clone(), base_config).unwrap();
//         assert_eq!(0, res.messages.len());

//         // *** Test Transfer Msg ***
//         let transfer_msg = TransferUnclaimedTokens {
//             recepient: "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string(),
//             amount:  Uint128::from(1000 as u64)
//         };

//         // should fail as only owner can Execute Transfer
//         let res_f = execute(deps.as_mut(), env.clone(), not_admin_info.clone(), transfer_msg.clone() );
//         assert_generic_error_message(res_f,"Sender not authorized!");

//         // should be a success
//         let res_s = execute(deps.as_mut(), env.clone(), admin_info.clone(), transfer_msg.clone()).unwrap();
//         assert_eq!(
//             res_s.attributes,
//             vec![
//                 attr("action", "Airdrop::ExecuteMsg::TransferUnclaimedTokens"),
//                 attr("recepient", "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp"),
//                 attr("amount", "1000"),
//             ]
//         );
//         assert_eq!(
//             res_s.messages,
//             vec![ SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
//                     contract_addr: "astro_token_contract".to_string(),
//                     funds: vec![],
//                     msg: to_binary(&CW20ExecuteMsg::Transfer {
//                         recipient: "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string(),
//                         amount: Uint128::from(1000 as u64),
//                     }).unwrap(),
//             }))]
//         );

//     }




//     #[test]
//     fn test_claim_by_terra_user() { 
//         let mut deps = mock_dependencies(&[]);
//         let admin_info = mock_info("admin");
//         let user_info_1 = mock_info("terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp");
//         let user_info_2 = mock_info("terra1757tkx08n0cqrw7p86ny9lnxsqeth0wgp0em95");
        
//         let mut init_mock_env_params = MockEnvParams {
//                                         block_time: Timestamp::from_seconds(1_571_797),
//                                         block_height: 1,
//                                     };
//         let mut env = mock_env(init_mock_env_params);
        
//         // Config with valid base params 
//         let base_config = InstantiateMsg {
//             owner: Some("owner".to_string()),
//             astro_token_address: Some("astro_token_contract".to_string()),
//             terra_merkle_roots: Some(vec!["cdcdfad1c342f5f55a2639dcae7321a64cd000807fa24c2c4ddaa944fd52d34e".to_string()]),
//             evm_merkle_roots: Some(vec!["".to_string()]),
//             from_timestamp: Some(1_575_797),
//             till_timestamp: Some(1_771_797)
//         };

//         // we can just call .unwrap() to assert this was a success
//         let res = instantiate(deps.as_mut(), env.clone(), admin_info.clone(), base_config).unwrap();
//         assert_eq!(0, res.messages.len());
//         let config = CONFIG.load(&deps.storage).unwrap();
//         assert_eq!(config.from_timestamp, 1_575_797 );
//         assert_eq!(config.till_timestamp, 1_771_797 );

//         let mut claim_msg = ClaimByTerraUser {
//                                             claim_amount : Uint128::from(250000000 as u64),
//                                             merkle_proof : vec!["7719b79a65e5aa0bbfd144cf5373138402ab1c374d9049e490b5b61c23d90065".to_string(),
//                                                                 "60368f2058e0fb961a7721a241f9b973c3dd6c57e10a627071cd81abca6aa490".to_string()],
//                                             root_index : 0
//                                         };
//         let mut claim_msg_wrong_amount = ClaimByTerraUser {
//                                             claim_amount : Uint128::from(210000000 as u64),
//                                             merkle_proof : vec!["7719b79a65e5aa0bbfd144cf5373138402ab1c374d9049e490b5b61c23d90065".to_string(),
//                                                                 "60368f2058e0fb961a7721a241f9b973c3dd6c57e10a627071cd81abca6aa490".to_string()],
//                                             root_index : 0
//                                         };
//         let mut claim_msg_incorrect_proof = ClaimByTerraUser {
//                                                         claim_amount : Uint128::from(250000000 as u64),
//                                                         merkle_proof : vec!["7719b79a65e4aa0bbfd144cf5373138402ab1c374d9049e490b5b61c23d90065".to_string(),
//                                                                             "60368f2058e0fb961a7721a241f9b973c3dd6c57e10a627071cd81abca6aa490".to_string()],
//                                                         root_index : 0
//                                                     };

//         // **** "Claim not allowed" Error should be returned ****
//         let mut claim_f = execute(deps.as_mut(), env.clone(), user_info_1.clone(), claim_msg.clone() );
//         assert_generic_error_message(claim_f,"Claim not allowed");
        
//         // Update MockEnv to test concluded error
//         init_mock_env_params = MockEnvParams {
//             block_time: Timestamp::from_seconds(1_771_798),
//             block_height: 1,
//         };
//         env = mock_env(init_mock_env_params);

//         // **** "Claim period has concluded" Error should be returned **** 
//         claim_f = execute(deps.as_mut(), env.clone(), user_info_1.clone(), claim_msg.clone() );
//         assert_generic_error_message(claim_f,"Claim period has concluded");
        

//         // Update MockEnv to test successful claim
//         init_mock_env_params = MockEnvParams {
//             block_time: Timestamp::from_seconds(1_771_098),
//             block_height: 1,
//         };
//         env = mock_env(init_mock_env_params);

//         // **** "Incorrect Merkle Proof" Error should be returned **** 
//         claim_f = execute(deps.as_mut(), env.clone(), user_info_1.clone(), claim_msg_incorrect_proof.clone() );
//         assert_generic_error_message(claim_f,"Incorrect Merkle Proof");

//         // **** "Incorrect Merkle Proof" Error should be returned **** 
//         claim_f = execute(deps.as_mut(), env.clone(), user_info_1.clone(), claim_msg_wrong_amount.clone() );
//         assert_generic_error_message(claim_f,"Incorrect Merkle Proof");

//         let mut is_claimed = check_user_claimed(deps.as_ref(), "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string() ).unwrap();
//         assert_eq!(is_claimed.is_claimed, false );

//         // **** Should process the airdrop successfully **** 
//         let claim_s = execute(deps.as_mut(), env.clone(), user_info_1.clone(), claim_msg.clone() ).unwrap();
//         assert_eq!(
//             claim_s.attributes,
//             vec![
//                 attr("action", "Airdrop::ExecuteMsg::ClaimByTerraUser"),
//                 attr("claimee", "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp"),
//                 attr("airdrop", "250000000"),
//             ]
//         );
//         assert_eq!(
//             claim_s.messages,
//             vec![ SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
//                     contract_addr: "astro_token_contract".to_string(),
//                     funds: vec![],
//                     msg: to_binary(&CW20ExecuteMsg::Transfer {
//                         recipient: "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string(),
//                         amount: Uint128::from(250000000 as u64),
//                     }).unwrap(),
//             }))]
//         );

//         is_claimed = check_user_claimed(deps.as_ref(), "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string() ).unwrap();
//         assert_eq!(is_claimed.is_claimed, true );

//         // ** "Already claimed" Error should be returned **
//         claim_f = execute(deps.as_mut(), env.clone(), user_info_1.clone(), claim_msg.clone() );
//         assert_generic_error_message(claim_f,"Already claimed");

//         claim_msg = ClaimByTerraUser {
//                                             claim_amount : Uint128::from(1 as u64),
//                                             merkle_proof : vec!["7fd0f6ac4074cef9f89eedcf72459ad7b0891855f8084b54dc7de7569849d1c8".to_string(),
//                                                                 "4fab6b0ef8d988835ad968d03d61de408772d033e9ce734394bb623309c5d7fc".to_string()],
//                                             root_index : 0
//                                         };
//         claim_msg_wrong_amount = ClaimByTerraUser {
//                                             claim_amount : Uint128::from(2 as u64),
//                                             merkle_proof : vec!["7fd0f6ac4074cef9f89eedcf72459ad7b0891855f8084b54dc7de7569849d1c8".to_string(),
//                                                                 "4fab6b0ef8d988835ad968d03d61de408772d033e9ce734394bb623309c5d7fc".to_string()],
//                                             root_index : 0
//                                         };
//         claim_msg_incorrect_proof = ClaimByTerraUser {
//                                             claim_amount : Uint128::from(1 as u64),
//                                             merkle_proof : vec!["7fd0f6ac4074cef1f89eedcf72459ad7b0891855f8084b54dc7de7569849d1c8".to_string(),
//                                                                 "4fab6b0ef8d988835ad968d03d61de408772d033e9ce734394bb623309c5d7fc".to_string()],
//                                             root_index : 0
//                                         };

//         // **** "Incorrect Merkle Proof" Error should be returned **** 
//         claim_f = execute(deps.as_mut(), env.clone(), user_info_2.clone(), claim_msg_incorrect_proof.clone() );
//         assert_generic_error_message(claim_f,"Incorrect Merkle Proof");

//         // **** "Incorrect Merkle Proof" Error should be returned **** 
//         claim_f = execute(deps.as_mut(), env.clone(), user_info_2.clone(), claim_msg_wrong_amount.clone() );
//         assert_generic_error_message(claim_f,"Incorrect Merkle Proof");

//         is_claimed = check_user_claimed(deps.as_ref(), "terra1757tkx08n0cqrw7p86ny9lnxsqeth0wgp0em95".to_string() ).unwrap();
//         assert_eq!(is_claimed.is_claimed, false );

//         // **** Should process the airdrop successfully **** 
//         let claim_s = execute(deps.as_mut(), env.clone(), user_info_2.clone(), claim_msg.clone() ).unwrap();
//         assert_eq!(
//             claim_s.attributes,
//             vec![
//                 attr("action", "Airdrop::ExecuteMsg::ClaimByTerraUser"),
//                 attr("claimee", "terra1757tkx08n0cqrw7p86ny9lnxsqeth0wgp0em95"),
//                 attr("airdrop", "1"),
//             ]
//         );
//         assert_eq!(
//             claim_s.messages,
//             vec![ SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
//                     contract_addr: "astro_token_contract".to_string(),
//                     funds: vec![],
//                     msg: to_binary(&CW20ExecuteMsg::Transfer {
//                         recipient: "terra1757tkx08n0cqrw7p86ny9lnxsqeth0wgp0em95".to_string(),
//                         amount: Uint128::from(1 as u64),
//                     }).unwrap(),
//             }))]
//         );

//         is_claimed = check_user_claimed(deps.as_ref(), "terra1757tkx08n0cqrw7p86ny9lnxsqeth0wgp0em95".to_string() ).unwrap();
//         assert_eq!(is_claimed.is_claimed, true );

//         // ** "Already claimed" Error should be returned **
//         claim_f = execute(deps.as_mut(), env.clone(), user_info_2.clone(), claim_msg.clone() );
//         assert_generic_error_message(claim_f,"Already claimed");
//     }




//     #[test]
//     fn test_claim_by_evm_user() { 
//         let mut deps = mock_dependencies(&[]);
//         let admin_info = mock_info("admin");

//         let user_info_recepient_ = mock_info("terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp");
//         let user_info_evm_address ="2c21b6fa9f82892d9853d8ee2351dc3c3e8e176d";
//         let user_info_claim_amount = 50000000;
//         let user_info_signed_msg_hash = "91f879f53729f18888d74aa10ea7737d629e36a1675bce35e1fb1be9065501df";
//         let user_info_signature = "ca6c32751cf2b46429b98a1649d5b115c8836328427080335d6c401ed8ac3a9030cc13f4793d05cc68162f1231122eba6ea9d1dda1c13b7e327efbaf0c024d7d";
        
//         let mut init_mock_env_params = MockEnvParams {
//                                         block_time: Timestamp::from_seconds(1_571_797),
//                                         block_height: 1,
//                                     };
//         let mut env = mock_env(init_mock_env_params);
        
//         // Config with valid base params 
//         let base_config = InstantiateMsg {
//             owner: Some("owner".to_string()),
//             astro_token_address: Some("astro_token_contract".to_string()),
//             terra_merkle_roots: Some(vec!["".to_string()]),
//             evm_merkle_roots: Some(vec!["1680ce46cb2c916f103afb54006b53dc751edccb8c0ba668fe1311ee7592c232".to_string()]),
//             from_timestamp: Some(1_575_797),
//             till_timestamp: Some(1_771_797)
//         };

//         // we can just call .unwrap() to assert this was a success
//         let res = instantiate(deps.as_mut(), env.clone(), admin_info.clone(), base_config).unwrap();
//         assert_eq!(0, res.messages.len());
//         let config = CONFIG.load(&deps.storage).unwrap();
//         assert_eq!(config.from_timestamp, 1_575_797 );
//         assert_eq!(config.till_timestamp, 1_771_797 );

//         let claim_msg = ClaimByEvmUser {
//                                             eth_address : user_info_evm_address.to_string() ,
//                                             claim_amount : Uint128::from(user_info_claim_amount as u64),
//                                             merkle_proof : vec!["0a3419fc5fa4cb0ecb878dc3aaf01fa00782e5d79b02fbb4097dc8df8f191c60".to_string(),
//                                                                 "45cc757ac5eda8bcd1a45a7bd2cb23f4af5147683f120fa287b99617834b83aa".to_string()],
//                                             root_index : 0,
//                                             signature : user_info_signature.to_string(),
//                                             signed_msg_hash : user_info_signed_msg_hash.to_string()
//                                         };
//         let claim_msg_wrong_amount = ClaimByEvmUser {
//                                             eth_address : user_info_evm_address.to_string() ,
//                                             claim_amount : Uint128::from(150000000 as u64),
//                                             merkle_proof : vec!["0a3419fc5fa4cb0ecb878dc3aaf01fa00782e5d79b02fbb4097dc8df8f191c60".to_string(),
//                                                                 "45cc757ac5eda8bcd1a45a7bd2cb23f4af5147683f120fa287b99617834b83aa".to_string()],
//                                             root_index : 0,
//                                             signature : user_info_signature.to_string(),
//                                             signed_msg_hash : user_info_signed_msg_hash.to_string()
//                                         };
//         let claim_msg_incorrect_proof = ClaimByEvmUser {
//                                             eth_address : user_info_evm_address.to_string() ,
//                                             claim_amount : Uint128::from(user_info_claim_amount as u64),
//                                             merkle_proof : vec!["0b3419fc5fa4cb0ecb878dc3aaf01fa00782e5d79b02fbb4097dc8df8f191c60".to_string(),
//                                                                 "45cc757ac5eda8bcd1a45a7bd2cb23f4af5147683f120fa287b99617834b83aa".to_string()],
//                                             root_index : 0,
//                                             signature : user_info_signature.to_string(),
//                                             signed_msg_hash : user_info_signed_msg_hash.to_string()
//                                         };
//         let claim_msg_incorrect_msg_hash = ClaimByEvmUser {
//                                             eth_address : user_info_evm_address.to_string() ,
//                                             claim_amount : Uint128::from(user_info_claim_amount as u64),
//                                             merkle_proof : vec!["0a3419fc5fa4cb0ecb878dc3aaf01fa00782e5d79b02fbb4097dc8df8f191c60".to_string(),
//                                                                 "45cc757ac5eda8bcd1a45a7bd2cb23f4af5147683f120fa287b99617834b83aa".to_string()],
//                                             root_index : 0,
//                                             signature : user_info_signature.to_string(),
//                                             signed_msg_hash : "11f879f53729f18888d74aa10ea7737d629e36a1675bce35e1fb1be9065501df".to_string()
//                                         };
//         let claim_msg_incorrect_signature = ClaimByEvmUser {
//                                             eth_address : user_info_evm_address.to_string() ,
//                                             claim_amount : Uint128::from(user_info_claim_amount as u64),
//                                             merkle_proof : vec!["0a3419fc5fa4cb0ecb878dc3aaf01fa00782e5d79b02fbb4097dc8df8f191c60".to_string(),
//                                                                 "45cc757ac5eda8bcd1a45a7bd2cb23f4af5147683f120fa287b99617834b83aa".to_string()],
//                                             root_index : 0,
//                                             signature : user_info_signature.to_string(),
//                                             signed_msg_hash : "ca7c32751cf2b46429b98a1649d5b115c8836328427080335d6c401ed8ac3a9030cc13f4793d05cc68162f1231122eba6ea9d1dda1c13b7e327efbaf0c024d7d1b".to_string()
//                                         };

//         // **** "Claim not allowed" Error should be returned ****
//         let mut claim_f = execute(deps.as_mut(), env.clone(), user_info_recepient_.clone(), claim_msg.clone() );
//         assert_generic_error_message(claim_f,"Claim not allowed");
        
//         // Update MockEnv to test concluded error
//         init_mock_env_params = MockEnvParams {
//             block_time: Timestamp::from_seconds(1_771_798),
//             block_height: 1,
//         };
//         env = mock_env(init_mock_env_params);

//         // **** "Claim period has concluded" Error should be returned **** 
//         claim_f = execute(deps.as_mut(), env.clone(), user_info_recepient_.clone(), claim_msg.clone() );
//         assert_generic_error_message(claim_f,"Claim period has concluded");
        

//         // Update MockEnv to test successful claim
//         init_mock_env_params = MockEnvParams {
//             block_time: Timestamp::from_seconds(1_771_098),
//             block_height: 1,
//         };
//         env = mock_env(init_mock_env_params);

//         // **** "Incorrect Merkle Proof" Error should be returned **** 
//         claim_f = execute(deps.as_mut(), env.clone(), user_info_recepient_.clone(), claim_msg_incorrect_proof.clone() );
//         assert_generic_error_message(claim_f,"Incorrect Merkle Proof");

//         // **** "Incorrect Merkle Proof" Error should be returned **** 
//         claim_f = execute(deps.as_mut(), env.clone(), user_info_recepient_.clone(), claim_msg_wrong_amount.clone() );
//         assert_generic_error_message(claim_f,"Incorrect Merkle Proof");

//         // **** "Invalid Signature" Error should be returned **** 
//         claim_f = execute(deps.as_mut(), env.clone(), user_info_recepient_.clone(), claim_msg_incorrect_msg_hash.clone() );
//         assert_generic_error_message(claim_f,"Invalid Signature");

//         // **** "Invalid Signature" Error should be returned **** 
//         claim_f = execute(deps.as_mut(), env.clone(), user_info_recepient_.clone(), claim_msg_incorrect_signature.clone() );
//         assert_generic_error_message(claim_f,"Invalid Signature");

//         let signature_response = verify_signature(deps.as_ref(), user_info_evm_address.to_string(), user_info_signature.to_string(), user_info_signed_msg_hash.to_string() ).unwrap();
//         assert_eq!(signature_response.is_valid, true );
//         assert_eq!(signature_response.recovered_address, user_info_evm_address.to_string() );

//         let mut is_claimed = check_user_claimed(deps.as_ref(), user_info_evm_address.to_string() ).unwrap();
//         assert_eq!(is_claimed.is_claimed, false );

//         // **** Should process the airdrop successfully **** 
//         let claim_s = execute(deps.as_mut(), env.clone(), user_info_recepient_.clone(), claim_msg.clone() ).unwrap();
//         assert_eq!(
//             claim_s.attributes,
//             vec![
//                 attr("action", "Airdrop::ExecuteMsg::ClaimByEvmUser"),
//                 attr("claimee", user_info_evm_address.to_string() ),
//                 attr("recepient", "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string() ),
//                 attr("airdrop", user_info_claim_amount.to_string() ),
//             ]
//         );
//         assert_eq!(
//             claim_s.messages,
//             vec![ SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
//                     contract_addr: "astro_token_contract".to_string(),
//                     funds: vec![],
//                     msg: to_binary(&CW20ExecuteMsg::Transfer {
//                         recipient: "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string(),
//                         amount: Uint128::from(user_info_claim_amount as u64),
//                     }).unwrap(),
//             }))]
//         );
        
//         is_claimed = check_user_claimed(deps.as_ref(), user_info_evm_address.to_string() ).unwrap();
//         assert_eq!(is_claimed.is_claimed, true );

//         // ** "Already claimed" Error should be returned **
//         claim_f = execute(deps.as_mut(), env.clone(), user_info_recepient_.clone(), claim_msg.clone() );
//         assert_generic_error_message(claim_f,"Already claimed");

//     }





//     pub struct MockEnvParams {
//         pub block_time: Timestamp,
//         pub block_height: u64,
//     }
    
//     impl Default for MockEnvParams {
//         fn default() -> Self {
//             MockEnvParams {
//                 block_time: Timestamp::from_nanos(1_571_797_419_879_305_533),
//                 block_height: 1,
//             }
//         }
//     }
    
//     /// mock_env replacement for cosmwasm_std::testing::mock_env
//     pub fn mock_env(mock_env_params: MockEnvParams) -> Env {
//         Env {
//             block: BlockInfo {
//                 height: mock_env_params.block_height,
//                 time: mock_env_params.block_time,
//                 chain_id: "cosmos-testnet-14002".to_string(),
//             },
//             contract: ContractInfo {
//                 address: Addr::unchecked(MOCK_CONTRACT_ADDR),
//             },
//         }
//     }

//     // quick mock info with just the sender
//     // TODO: Maybe this one does not make sense given there's a very smilar helper in cosmwasm_std
//     pub fn mock_info(sender: &str) -> MessageInfo {
//         MessageInfo {
//             sender: Addr::unchecked(sender),
//             funds: vec![],
//         }
//     }

//     /// mock_dependencies replacement for cosmwasm_std::testing::mock_dependencies
//     pub fn mock_dependencies(
//         contract_balance: &[Coin],
//     ) -> OwnedDeps<MockStorage, MockApi, MockQuerier> {
//         let contract_addr = Addr::unchecked(MOCK_CONTRACT_ADDR);
//         let custom_querier: MockQuerier = MockQuerier::new(&[(
//             &contract_addr.to_string(),
//             contract_balance,
//         )]);

//         OwnedDeps {
//             storage: MockStorage::default(),
//             api: MockApi::default(),
//             querier: custom_querier,
//         }
//     }

//     /// Assert StdError::GenericErr message with expected_msg
//     pub fn assert_generic_error_message<T>(response: StdResult<T>, expected_msg: &str) {
//         match response {
//             Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, expected_msg),
//             Err(other_err) => panic!("Unexpected error: {:?}", other_err),
//             Ok(_) => panic!("SHOULD NOT ENTER HERE!"),
//         }
//     }

// }