use crate::crypto::verify_claim;
use crate::state::{Config, State, CONFIG, STATE, USERS};
use astroport_periphery::helpers::{build_transfer_cw20_token_msg, cw20_get_balance};
use astroport_periphery::simple_airdrop::{
    ClaimResponse, ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg,
    StateResponse, UserInfoResponse,
};
use cosmwasm_std::{
    attr, entry_point, from_binary, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response,
    StdError, StdResult, Uint128,
};
use cw2::set_contract_version;
use cw20::Cw20ReceiveMsg;

// version info for migration info
const CONTRACT_NAME: &str = "astroport_simple_airdrop";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

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
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
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

    let config = Config {
        owner,
        astro_token_address: deps.api.addr_validate(&msg.astro_token_address)?,
        merkle_roots: msg.merkle_roots.unwrap_or_default(),
        from_timestamp,
        to_timestamp: msg.to_timestamp,
    };

    let state = State {
        total_airdrop_size: Uint128::zero(),
        unclaimed_tokens: Uint128::zero(),
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
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::UpdateConfig {
            owner,
            merkle_roots,
            from_timestamp,
            to_timestamp,
        } => handle_update_config(
            deps,
            env,
            info,
            owner,
            merkle_roots,
            from_timestamp,
            to_timestamp,
        ),
        ExecuteMsg::Claim {
            claim_amount,
            merkle_proof,
            root_index,
        } => handle_claim(deps, env, info, claim_amount, merkle_proof, root_index),
        ExecuteMsg::TransferUnclaimedTokens { recipient, amount } => {
            handle_transfer_unclaimed_tokens(deps, env, info, recipient, amount)
        }
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;

    if info.sender != config.astro_token_address {
        return Err(StdError::generic_err("Only astro tokens are received!"));
    }

    // CHECK :: CAN ONLY BE CALLED BY THE OWNER
    if cw20_msg.sender != config.owner {
        return Err(StdError::generic_err("Sender not authorized!"));
    }

    // CHECK ::: Amount needs to be valid
    if cw20_msg.amount.is_zero() {
        return Err(StdError::generic_err("Amount must be greater than 0"));
    }

    match from_binary(&cw20_msg.msg)? {
        Cw20HookMsg::IncreaseAstroIncentives {} => {
            handle_increase_astro_incentives(deps, cw20_msg.amount)
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
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}

//----------------------------------------------------------------------------------------
// Handle functions
//----------------------------------------------------------------------------------------

/// @dev Admin function to update Configuration parameters
/// @param new_config : Same as InstantiateMsg struct
pub fn handle_update_config(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    owner: Option<String>,
    merkle_roots: Option<Vec<String>>,
    from_timestamp: Option<u64>,
    to_timestamp: Option<u64>,
) -> StdResult<Response> {
    let mut config = CONFIG.load(deps.storage)?;
    let mut attributes = vec![attr("action", "Airdrop::ExecuteMsg::UpdateConfig")];

    // CHECK :: ONLY OWNER CAN CALL THIS FUNCTION
    if info.sender != config.owner {
        return Err(StdError::generic_err("Only owner can update configuration"));
    }

    if let Some(owner) = owner {
        config.owner = deps.api.addr_validate(&owner)?;
        attributes.push(attr("new_owner", owner.as_str()))
    }

    if let Some(merkle_roots) = merkle_roots {
        config.merkle_roots = merkle_roots
    }

    if let Some(from_timestamp) = from_timestamp {
        if env.block.time.seconds() >= config.from_timestamp {
            return Err(StdError::generic_err(
                "from_timestamp can't be changed after window starts",
            ));
        }
        config.from_timestamp = from_timestamp;
        attributes.push(attr("new_from_timestamp", from_timestamp.to_string()))
    }

    if let Some(to_timestamp) = to_timestamp {
        if env.block.time.seconds() >= config.from_timestamp && to_timestamp < config.to_timestamp {
            return Err(StdError::generic_err(
                "When window starts to_timestamp can only be increased",
            ));
        }
        config.to_timestamp = to_timestamp;
        attributes.push(attr("new_to_timestamp", to_timestamp.to_string()))
    }

    if config.to_timestamp <= config.from_timestamp {
        return Err(StdError::generic_err("Invalid airdrop claim window"));
    }

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attributes(attributes))
}

/// @dev Facilitates increasing ASTRO airdrop amount
pub fn handle_increase_astro_incentives(
    deps: DepsMut,
    amount: Uint128,
) -> Result<Response, StdError> {
    let mut state = STATE.load(deps.storage)?;
    state.total_airdrop_size += amount;
    state.unclaimed_tokens += amount;

    STATE.save(deps.storage, &state)?;
    Ok(Response::new()
        .add_attribute("action", "astro_airdrop_increased")
        .add_attribute("total_airdrop_size", state.total_airdrop_size))
}

/// @dev Executes an airdrop claim for a Terra User
/// @param claim_amount : Airdrop to be claimed by the user
/// @param merkle_proof : Array of hashes to prove the input is a leaf of the Merkle Tree
/// @param root_index : Merkle Tree root identifier to be used for verification
pub fn handle_claim(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    claim_amount: Uint128,
    merkle_proof: Vec<String>,
    root_index: u32,
) -> Result<Response, StdError> {
    let recipient = info.sender;

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

    let merkle_root = config.merkle_roots.get(root_index as usize);
    if merkle_root.is_none() {
        return Err(StdError::generic_err("Incorrect Merkle Root Index"));
    }

    if !verify_claim(&recipient, claim_amount, merkle_proof, merkle_root.unwrap()) {
        return Err(StdError::generic_err("Incorrect Merkle Proof"));
    }

    let mut user_info = USERS.load(deps.storage, &recipient).unwrap_or_default();

    // Check if addr has already claimed the tokens
    if !user_info.airdrop_amount.is_zero() {
        return Err(StdError::generic_err("Already claimed"));
    }

    let mut messages = vec![];

    // check is sufficient ASTRO available
    if state.unclaimed_tokens < claim_amount {
        return Err(StdError::generic_err("Insufficient ASTRO available"));
    }

    // TRANSFER ASTRO to the user
    messages.push(build_transfer_cw20_token_msg(
        recipient.clone(),
        config.astro_token_address.to_string(),
        claim_amount,
    )?);

    // Update amounts
    state.unclaimed_tokens -= claim_amount;
    user_info.airdrop_amount = claim_amount;

    USERS.save(deps.storage, &recipient, &user_info)?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "Airdrop::ExecuteMsg::Claim"),
        attr("addr", recipient),
        attr("airdrop", claim_amount),
    ]))
}

/// @dev Admin function to transfer ASTRO Tokens to the recipient address
/// @param recipient Recipient receiving the ASTRO tokens
/// @param amount Amount of ASTRO to be transferred
pub fn handle_transfer_unclaimed_tokens(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
    amount: Uint128,
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;

    // CHECK :: CAN ONLY BE CALLED BY THE OWNER
    if info.sender != config.owner {
        return Err(StdError::generic_err("Sender not authorized!"));
    }

    // CHECK :: CAN ONLY BE CALLED AFTER THE CLAIM PERIOD IS OVER
    if config.to_timestamp > env.block.time.seconds() {
        return Err(StdError::generic_err(format!(
            "{} seconds left before unclaimed tokens can be transferred",
            { config.to_timestamp - env.block.time.seconds() }
        )));
    }

    let max_transferrable_tokens = cw20_get_balance(
        &deps.querier,
        config.astro_token_address.clone(),
        env.contract.address,
    )?;

    // CHECK :: Amount needs to be less than max_transferrable_tokens balance
    if amount > max_transferrable_tokens {
        return Err(StdError::generic_err(format!(
            "Amount cannot exceed max available ASTRO balance {}",
            max_transferrable_tokens
        )));
    }

    // COSMOS MSG :: TRANSFER ASTRO TOKENS
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
        merkle_roots: config.merkle_roots,
        from_timestamp: config.from_timestamp,
        to_timestamp: config.to_timestamp,
    })
}

/// @dev Returns the airdrop contract state
fn query_state(deps: Deps) -> StdResult<StateResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(StateResponse {
        total_airdrop_size: state.total_airdrop_size,
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
    })
}

/// @dev Returns true if the user has claimed the airdrop [EVM addresses to be provided in lower-case without the '0x' prefix]
fn query_user_claimed(deps: Deps, address: String) -> StdResult<ClaimResponse> {
    let user_address = deps.api.addr_validate(&address)?;
    let user_info = USERS
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    Ok(ClaimResponse {
        is_claimed: !user_info.airdrop_amount.is_zero(),
    })
}
