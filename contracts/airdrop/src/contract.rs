use astroport_periphery::airdrop::{
    ClaimResponse, ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg, SignatureResponse,
    StateResponse, UserInfoResponse,
};
use astroport_periphery::helpers::{build_send_cw20_token_msg, build_transfer_cw20_token_msg};
use astroport_periphery::lp_bootstrap_auction::Cw20HookMsg::DelegateAstroTokens;
use cosmwasm_std::{
    attr, entry_point, to_binary, Addr, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo,
    Response, StdError, StdResult, Uint128,
};

use crate::crypto::{handle_verify_signature, verify_claim};
use crate::state::{Config, State, CLAIMS, CONFIG, STATE, USERS};

//----------------------------------------------------------------------------------------
// Entry points
//----------------------------------------------------------------------------------------

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let from_timestamp = msg
        .from_timestamp
        .unwrap_or_else(|| env.block.time.seconds());
    if msg.till_timestamp.unwrap() <= from_timestamp {
        return Err(StdError::generic_err(
            "Invalid airdrop claim window closure timestamp",
        ));
    }

    let owner = if let Some(owner) = msg.owner {
        deps.api.addr_validate(owner.as_str())?
    } else {
        info.sender
    };

    if msg.total_airdrop_size.is_zero() {
        return Err(StdError::generic_err("Invalid total airdrop amount"));
    }

    let config = Config {
        owner,
        astro_token_address: deps
            .api
            .addr_validate(msg.astro_token_address.unwrap().as_str())?,
        terra_merkle_roots: msg.terra_merkle_roots.unwrap_or_default(),
        evm_merkle_roots: msg.evm_merkle_roots.unwrap_or_default(),
        from_timestamp,
        till_timestamp: msg.till_timestamp.unwrap(),
        boostrap_auction_address: deps
            .api
            .addr_validate(msg.boostrap_auction_address.unwrap().as_str())?,
        are_claims_enabled: false,
    };

    let state = State {
        total_airdrop_size: msg.total_airdrop_size,
        total_delegated_amount: Uint128::zero(),
        unclaimed_tokens: msg.total_airdrop_size,
    };

    CONFIG.save(deps.storage, &config)?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, StdError> {
    match msg {
        ExecuteMsg::UpdateConfig { new_config } => handle_update_config(deps, info, new_config),
        ExecuteMsg::ClaimByTerraUser {
            claim_amount,
            merkle_proof,
            root_index,
        } => handle_terra_user_claim(deps, env, info, claim_amount, merkle_proof, root_index),
        ExecuteMsg::ClaimByEvmUser {
            eth_address,
            claim_amount,
            merkle_proof,
            root_index,
            signature,
            signed_msg_hash,
        } => handle_evm_user_claim(
            deps,
            env,
            info,
            eth_address,
            claim_amount,
            merkle_proof,
            root_index,
            signature,
            signed_msg_hash,
        ),
        ExecuteMsg::DelegateAstroToBootstrapAuction { amount_to_delegate } => {
            handle_delegate_astro_to_bootstrap_auction(deps, env, info, amount_to_delegate)
        }
        ExecuteMsg::EnableClaims {} => handle_enable_claims(deps, info),
        ExecuteMsg::WithdrawAirdropReward {} => handle_withdraw_airdrop_rewards(deps, env, info),
        ExecuteMsg::TransferUnclaimedTokens { recepient, amount } => {
            handle_transfer_unclaimed_tokens(deps, env, info, recepient, amount)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State {} => to_binary(&query_state(deps)?),
        QueryMsg::HasUserClaimed { address } => to_binary(&query_user_claimed(deps, address)?),
        QueryMsg::UserInfo { address } => to_binary(&query_user_info(deps, address)?),
        QueryMsg::IsValidSignature {
            evm_address,
            evm_signature,
            signed_msg_hash,
        } => to_binary(&verify_signature(
            deps,
            evm_address,
            evm_signature,
            signed_msg_hash,
        )?),
    }
}

//----------------------------------------------------------------------------------------
// Handle functions
//----------------------------------------------------------------------------------------

/// @dev Admin function to update Configuration parameters
/// @param new_config : Same as InstantiateMsg struct
pub fn handle_update_config(
    deps: DepsMut,
    info: MessageInfo,
    new_config: InstantiateMsg,
) -> StdResult<Response> {
    let mut config = CONFIG.load(deps.storage)?;

    // CHECK :: ONLY OWNER CAN CALL THIS FUNCTION
    if info.sender != config.owner {
        return Err(StdError::generic_err("Only owner can update configuration"));
    }

    if let Some(owner) = new_config.owner {
        config.owner = deps.api.addr_validate(owner.as_str())?;
    }

    if let Some(astro_token_address) = new_config.astro_token_address {
        config.astro_token_address = deps.api.addr_validate(astro_token_address.as_str())?;
    }

    if let Some(boostrap_auction_address) = new_config.boostrap_auction_address {
        config.boostrap_auction_address =
            deps.api.addr_validate(boostrap_auction_address.as_str())?;
    }

    if let Some(terra_merkle_roots) = new_config.terra_merkle_roots {
        config.terra_merkle_roots = terra_merkle_roots
    }

    if let Some(evm_merkle_roots) = new_config.evm_merkle_roots {
        config.evm_merkle_roots = evm_merkle_roots
    }

    if let Some(from_timestamp) = new_config.from_timestamp {
        config.from_timestamp = from_timestamp
    }

    if let Some(till_timestamp) = new_config.till_timestamp {
        if till_timestamp <= config.from_timestamp {
            return Err(StdError::generic_err(
                "Invalid airdrop claim window closure timestamp",
            ));
        }

        config.till_timestamp = till_timestamp
    }

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "Airdrop::ExecuteMsg::UpdateConfig"))
}

/// @dev Function to enable ASTRO Claims by users. Called along-with Bootstrap Auction contract's LP Pool provide liquidity tx
pub fn handle_enable_claims(deps: DepsMut, info: MessageInfo) -> StdResult<Response> {
    let mut config = CONFIG.load(deps.storage)?;

    // CHECK :: ONLY AUCTION CONTRACT CAN CALL THIS FUNCTION
    if info.sender != config.boostrap_auction_address {
        return Err(StdError::generic_err("Unauthorized"));
    }

    if config.are_claims_enabled {
        return Err(StdError::generic_err("Already allowed"));
    }

    config.are_claims_enabled = true;

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "Airdrop::ExecuteMsg::EnableClaims"))
}

pub enum ClaimNetwork {
    Terra {},
    Eth {
        signature: String,
        signed_msg_hash: String,
    },
}

pub fn handle_claim(
    deps: DepsMut,
    env: Env,
    claim_addr: String,
    claim_network: ClaimNetwork,
    claim_amount: Uint128,
    merkle_proof: Vec<String>,
    root_index: u32,
    recipient: Addr,
) -> Result<Vec<CosmosMsg>, StdError> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // CHECK :: IS AIRDROP CLAIM WINDOW OPEN ?
    if config.from_timestamp > env.block.time.seconds() {
        return Err(StdError::generic_err("Claim not allowed"));
    }

    // CHECK :: IS AIRDROP CLAIM WINDOW OPEN ?
    if config.till_timestamp < env.block.time.seconds() {
        return Err(StdError::generic_err("Claim period has concluded"));
    }

    // CHECK :: HAS USER ALREADY CLAIMED THEIR AIRDROP ?
    let claim_exists = CLAIMS
        .load(deps.storage, claim_addr.clone())
        .unwrap_or(false);
    if claim_exists {
        return Err(StdError::generic_err("Already claimed"));
    }

    // MERKLE PROOF VERIFICATION
    match claim_network {
        ClaimNetwork::Terra {} => {
            let merkle_root = config.terra_merkle_roots.get(root_index as usize);
            if merkle_root.is_none() {
                return Err(StdError::generic_err("Incorrect Merkle Root Index"));
            }

            if !verify_claim(
                claim_addr.clone(),
                claim_amount,
                merkle_proof,
                merkle_root.unwrap(),
            ) {
                return Err(StdError::generic_err("Incorrect Merkle Proof"));
            }
        }
        ClaimNetwork::Eth {
            signature,
            signed_msg_hash,
        } => {
            let merkle_root = config.evm_merkle_roots.get(root_index as usize);
            if merkle_root.is_none() {
                return Err(StdError::generic_err("Incorrect Merkle Root Index"));
            }

            if !verify_claim(
                claim_addr.clone(),
                claim_amount,
                merkle_proof,
                merkle_root.unwrap(),
            ) {
                return Err(StdError::generic_err("Incorrect Merkle Proof"));
            }

            // SIGNATURE VERIFICATION
            let sig =
                handle_verify_signature(deps.api, claim_addr.clone(), signature, signed_msg_hash);
            if !sig.is_valid {
                return Err(StdError::generic_err("Invalid Signature"));
            }
        }
    }

    let mut user_info = USERS.load(deps.storage, &recipient).unwrap_or_default();

    state.unclaimed_tokens -= claim_amount;
    user_info.airdrop_amount += claim_amount;

    let mut messages = vec![];

    // TRANSFER ASTRO IF CLAIMS ARE ALLOWED (i.e LP Boostrap auction has concluded)
    if config.are_claims_enabled {
        let amount_to_withdraw = user_info.airdrop_amount - user_info.delegated_amount;
        messages.push(build_transfer_cw20_token_msg(
            recipient.clone(),
            config.astro_token_address.to_string(),
            amount_to_withdraw,
        )?);
    }

    USERS.save(deps.storage, &recipient, &user_info)?;
    STATE.save(deps.storage, &state)?;
    CLAIMS.save(deps.storage, claim_addr, &true)?;

    Ok(messages)
}

/// @dev Executes an airdrop claim for a Terra User
/// @param claim_amount : Airdrop to be claimed by the user
/// @param merkle_proof : Array of hashes to prove the input is a leaf of the Merkle Tree
/// @param root_index : Merkle Tree root identifier to be used for verification
pub fn handle_terra_user_claim(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    claim_amount: Uint128,
    merkle_proof: Vec<String>,
    root_index: u32,
) -> Result<Response, StdError> {
    let claimer = info.sender;

    let messages = handle_claim(
        deps,
        env,
        claimer.clone().into_string(),
        ClaimNetwork::Terra {},
        claim_amount,
        merkle_proof,
        root_index,
        claimer.clone(),
    )?;

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "Airdrop::ExecuteMsg::ClaimByTerraUser"),
        attr("claimer", claimer),
        attr("airdrop", claim_amount),
    ]))
}

/// @dev Executes an airdrop claim by an EVM User
/// @param eth_address : EVM address claiming the airdop. Needs to be in lower case without the `0x` prefix
/// @param claim_amount : Airdrop amount claimed by the user
/// @param merkle_proof : Array of hashes to prove the input is a leaf of the Merkle Tree
/// @param root_index : Merkle Tree root identifier to be used for verification
/// @param signature : ECDSA Signature string generated by signing the message (without the `0x` prefix and the last 2 characters which originate from `v`)
/// @param signed_msg_hash : Keccak256 hash of the signed message following the ethereum prefix standard.(without the `0x` prefix)
/// https://web3js.readthedocs.io/en/v1.2.2/web3-eth-accounts.html#hashmessage
pub fn handle_evm_user_claim(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    eth_address: String,
    claim_amount: Uint128,
    merkle_proof: Vec<String>,
    root_index: u32,
    signature: String,
    signed_msg_hash: String,
) -> Result<Response, StdError> {
    let claimer = eth_address;
    let recipient = info.sender;

    let messages = handle_claim(
        deps,
        env,
        claimer.clone(),
        ClaimNetwork::Eth {
            signature,
            signed_msg_hash,
        },
        claim_amount,
        merkle_proof,
        root_index,
        recipient.clone(),
    )?;

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "Airdrop::ExecuteMsg::ClaimByEvmUser"),
        attr("claimer", claimer),
        attr("recipient", recipient.to_string()),
        attr("airdrop", claim_amount.to_string()),
    ]))
}

/// @dev Function to allow users to delegate their ASTRO Tokens to the LP Bootstrap auction contract
/// @param amount_to_delegate Amount of ASTRO to be delegate
pub fn handle_delegate_astro_to_bootstrap_auction(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    amount_to_delegate: Uint128,
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;

    // CHECK :: HAS THE BOOTSTRAP AUCTION CONCLUDED ?
    if config.are_claims_enabled {
        return Err(StdError::generic_err("LP bootstrap auction has concluded"));
    }

    let mut state = STATE.load(deps.storage)?;
    let mut user_info = USERS.load(deps.storage, &info.sender.clone())?;

    // CHECK :: HAS USER ALREADY WITHDRAWN THEIR REWARDS ?
    if user_info.tokens_withdrawn {
        return Err(StdError::generic_err("Tokens have already been claimed"));
    }

    state.total_delegated_amount += amount_to_delegate;
    user_info.delegated_amount += amount_to_delegate;

    // CHECK :: TOKENS BEING DELEGATED SHOULD NOT EXCEED USER'S CLAIMABLE AIRDROP AMOUNT
    if user_info.delegated_amount > user_info.airdrop_amount {
        return Err(StdError::generic_err("Total amount being delegated for boostrap auction cannot exceed your claimable airdrop balance"));
    }

    // COSMOS MSG :: DELEGATE ASTRO TOKENS TO LP BOOTSTRAP AUCTION CONTRACT
    let msg = to_binary(&DelegateAstroTokens {
        user_address: info.sender.clone(),
    })?;

    let delegate_msg = build_send_cw20_token_msg(
        config.boostrap_auction_address.to_string(),
        config.astro_token_address.to_string(),
        amount_to_delegate,
        msg,
    )?;

    // STATE UPDATE : SAVE UPDATED STATES
    USERS.save(deps.storage, &info.sender, &user_info)?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_messages(vec![delegate_msg])
        .add_attributes(vec![
            attr(
                "action",
                "Airdrop::ExecuteMsg::DelegateAstroToBootstrapAuction",
            ),
            attr("user", info.sender.to_string()),
            attr("amount_delegated", amount_to_delegate),
        ]))
}

/// @dev Function to allow users to withdraw their undelegated ASTRO Tokens
pub fn handle_withdraw_airdrop_rewards(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;
    let mut user_info = USERS.load(deps.storage, &info.sender.clone())?;

    // CHECK :: HAS THE BOOTSTRAP AUCTION CONCLUDED ?
    if !config.are_claims_enabled {
        return Err(StdError::generic_err(
            "LP Boostrap auction in progress. Claims not allowed during this period",
        ));
    }

    // CHECK :: HAS USER ALREADY WITHDRAWN THEIR REWARDS ?
    if user_info.tokens_withdrawn {
        return Err(StdError::generic_err("Already claimed"));
    }

    // TRANSFER ASTRO IF CLAIMS ARE ALLOWED (i.e LP Boostrap auction has concluded)
    user_info.tokens_withdrawn = true;

    let tokens_to_withdraw = user_info.airdrop_amount - user_info.delegated_amount;
    if tokens_to_withdraw.is_zero() {
        return Err(StdError::generic_err("Nothing to withdraw"));
    }

    let transfer_msg = build_transfer_cw20_token_msg(
        info.sender.clone(),
        config.astro_token_address.to_string(),
        tokens_to_withdraw,
    )?;

    USERS.save(deps.storage, &info.sender, &user_info)?;

    Ok(Response::new()
        .add_message(transfer_msg)
        .add_attributes(vec![
            attr("action", "Airdrop::ExecuteMsg::WithdrawAirdropRewards"),
            attr("user", info.sender.to_string()),
            attr("claimed_amount", tokens_to_withdraw),
            attr("total_airdrop", user_info.airdrop_amount),
        ]))
}

/// @dev Admin function to transfer ASTRO Tokens to the recepient address
/// @param recepient Recepient receiving the ASTRO tokens
/// @param amount Amount of ASTRO to be transferred
pub fn handle_transfer_unclaimed_tokens(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    recipient: String,
    amount: Uint128,
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // CHECK :: CAN ONLY BE CALLED BY THE OWNER
    if info.sender != config.owner {
        return Err(StdError::generic_err("Sender not authorized!"));
    }

    // CHECK :: CAN ONLY BE CALLED AFTER THE CLAIM PERIOD IS OVER
    if config.till_timestamp > _env.block.time.seconds() {
        return Err(StdError::generic_err(
            "Airdrop claim period has not concluded",
        ));
    }

    if amount > state.unclaimed_tokens {
        return Err(StdError::generic_err(
            "Amount cannot exceed unclaimed token balance",
        ));
    }

    // COSMOS MSG :: TRANSFER ASTRO TOKENS
    state.unclaimed_tokens -= amount;
    let transfer_msg = build_transfer_cw20_token_msg(
        deps.api.addr_validate(&recipient)?,
        config.astro_token_address.to_string(),
        amount,
    )?;

    Ok(Response::new()
        .add_message(transfer_msg)
        .add_attributes(vec![
            attr("action", "Airdrop::ExecuteMsg::TransferUnclaimedRewards"),
            attr("recipient", recipient),
            attr("amount", amount),
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
        are_claims_allowed: config.are_claims_enabled,
    })
}

/// @dev Returns the airdrop contract state
fn query_state(deps: Deps) -> StdResult<StateResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(StateResponse {
        total_airdrop_size: state.total_airdrop_size,
        total_delegated_amount: state.total_delegated_amount,
        unclaimed_tokens: state.unclaimed_tokens,
    })
}

/// @dev Returns details around user's ASTRO Airdrop claim
fn query_user_info(deps: Deps, user_address: String) -> StdResult<UserInfoResponse> {
    let user_address = deps.api.addr_validate(&user_address)?;
    let user_info = USERS
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();
    Ok(UserInfoResponse {
        airdrop_amount: user_info.airdrop_amount,
        delegated_amount: user_info.delegated_amount,
        tokens_withdrawn: user_info.tokens_withdrawn,
    })
}

/// @dev Returns true if the user has claimed the airdrop [EVM addresses to be provided in lower-case without the '0x' prefix]
fn query_user_claimed(deps: Deps, address: String) -> StdResult<ClaimResponse> {
    Ok(ClaimResponse {
        is_claimed: CLAIMS.load(deps.storage, address).unwrap_or(false),
    })
}

/// @dev Returns the recovered public key, evm address and a boolean value which is true if the evm address provided was used for signing the message.
/// @param evm_address : EVM address claiming the airdop. Needs to be in lower case without the `0x` prefix
/// @param evm_signature : ECDSA Signature string generated by signing the message (without the `0x` prefix and the last 2 characters which originate from `v`)
/// @param signed_msg_hash : Keccak256 hash of the signed message following the EIP-191 prefix standard.(without the `0x` prefix)
fn verify_signature(
    _deps: Deps,
    evm_address: String,
    evm_signature: String,
    signed_msg_hash: String,
) -> StdResult<SignatureResponse> {
    let verification_response =
        handle_verify_signature(_deps.api, evm_address, evm_signature, signed_msg_hash);

    Ok(SignatureResponse {
        is_valid: verification_response.is_valid,
        public_key: verification_response.public_key,
        recovered_address: verification_response.recovered_address,
    })
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

//         let mut is_claimed = query_user_claimed(deps.as_ref(), "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string() ).unwrap();
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

//         is_claimed = query_user_claimed(deps.as_ref(), "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string() ).unwrap();
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

//         is_claimed = query_user_claimed(deps.as_ref(), "terra1757tkx08n0cqrw7p86ny9lnxsqeth0wgp0em95".to_string() ).unwrap();
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

//         is_claimed = query_user_claimed(deps.as_ref(), "terra1757tkx08n0cqrw7p86ny9lnxsqeth0wgp0em95".to_string() ).unwrap();
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

//         let mut is_claimed = query_user_claimed(deps.as_ref(), user_info_evm_address.to_string() ).unwrap();
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

//         is_claimed = query_user_claimed(deps.as_ref(), user_info_evm_address.to_string() ).unwrap();
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
