use astroport_periphery::airdrop::{
    ClaimResponse, ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg, SignatureResponse,
    StateResponse, UserInfoResponse,
};
use astroport_periphery::auction::Cw20HookMsg::DelegateAstroTokens;
use astroport_periphery::helpers::{build_send_cw20_token_msg, build_transfer_cw20_token_msg};
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

    if msg.to_timestamp <= from_timestamp {
        return Err(StdError::generic_err(
            "Invalid airdrop claim window closure timestamp",
        ));
    }

    let owner = if let Some(owner) = msg.owner {
        deps.api.addr_validate(&owner)?
    } else {
        info.sender
    };

    if msg.total_airdrop_size.is_zero() {
        return Err(StdError::generic_err("Invalid total airdrop amount"));
    }

    let config = Config {
        owner,
        astro_token_address: deps.api.addr_validate(&msg.astro_token_address)?,
        terra_merkle_roots: msg.terra_merkle_roots.unwrap_or_default(),
        evm_merkle_roots: msg.evm_merkle_roots.unwrap_or_default(),
        from_timestamp,
        to_timestamp: msg.to_timestamp,
        boostrap_auction_address: deps.api.addr_validate(&msg.boostrap_auction_address)?,
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
        ExecuteMsg::UpdateConfig {
            owner,
            boostrap_auction_address,
            terra_merkle_roots,
            evm_merkle_roots,
            from_timestamp,
            to_timestamp,
        } => handle_update_config(
            deps,
            info,
            owner,
            boostrap_auction_address,
            terra_merkle_roots,
            evm_merkle_roots,
            from_timestamp,
            to_timestamp,
        ),
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
    owner: Option<String>,
    boostrap_auction_address: Option<String>,
    terra_merkle_roots: Option<Vec<String>>,
    evm_merkle_roots: Option<Vec<String>>,
    from_timestamp: Option<u64>,
    to_timestamp: Option<u64>,
) -> StdResult<Response> {
    let mut config = CONFIG.load(deps.storage)?;

    // CHECK :: ONLY OWNER CAN CALL THIS FUNCTION
    if info.sender != config.owner {
        return Err(StdError::generic_err("Only owner can update configuration"));
    }

    if let Some(owner) = owner {
        config.owner = deps.api.addr_validate(&owner)?;
    }

    if let Some(boostrap_auction_address) = boostrap_auction_address {
        config.boostrap_auction_address = deps.api.addr_validate(&boostrap_auction_address)?;
    }

    if let Some(terra_merkle_roots) = terra_merkle_roots {
        config.terra_merkle_roots = terra_merkle_roots
    }

    if let Some(evm_merkle_roots) = evm_merkle_roots {
        config.evm_merkle_roots = evm_merkle_roots
    }

    if let Some(from_timestamp) = from_timestamp {
        config.from_timestamp = from_timestamp
    }

    if let Some(to_timestamp) = to_timestamp {
        if to_timestamp <= config.from_timestamp {
            return Err(StdError::generic_err(
                "Invalid airdrop claim window closure timestamp",
            ));
        }

        config.to_timestamp = to_timestamp
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
        return Err(StdError::generic_err("Claims already enabled"));
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
    if config.to_timestamp < env.block.time.seconds() {
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
        user_info.tokens_withdrawn = true;
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
    if config.to_timestamp > _env.block.time.seconds() {
        return Err(StdError::generic_err(format!(
            "{} seconds left before unclaimed tokens can be transferred",
            { config.to_timestamp - _env.block.time.seconds() }
        )));
    }

    // CHECK :: Amount needs to be less than unclaimed_tokens balance
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

    STATE.save(deps.storage, &state)?;
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
        to_timestamp: config.to_timestamp,
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
        is_claimed: CLAIMS.may_load(deps.storage, address)?.unwrap_or(false),
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
