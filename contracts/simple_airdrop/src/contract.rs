use crate::crypto::verify_claim;
use crate::state::{CONFIG, STATE, USERS};
use astroport::asset::addr_validate_to_lower;
use astroport_periphery::helpers::{build_transfer_cw20_token_msg, cw20_get_balance};
use astroport_periphery::simple_airdrop::{
    ClaimResponse, Config, Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, State,
    UserInfoResponse,
};
use cosmwasm_std::{
    attr, entry_point, from_binary, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response,
    StdError, StdResult, Uint128,
};
use cw2::set_contract_version;
use cw20::Cw20ReceiveMsg;

/// Contract name that is used for migration.
const CONTRACT_NAME: &str = "astroport_simple_airdrop";
/// Contract version that is used for migration.
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Creates a new contract with the specified parameters packed in the `msg` variable.
/// Returns a [`Response`] with the specified attributes if the operation was successful, or a [`ContractError`] if the contract was not created
/// ## Params
/// * **deps** is an object of type [`DepsMut`].
///
/// * **env** is an object of type [`Env`].
///
/// * **info** is an object of type [`MessageInfo`].
///
/// * **msg**  is a message of type [`InstantiateMsg`] which contains the parameters used for creating the contract.
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
        addr_validate_to_lower(deps.api, &owner)?
    } else {
        info.sender
    };

    let config = Config {
        owner,
        astro_token_address: addr_validate_to_lower(deps.api, &msg.astro_token_address)?,
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

/// ## Description
/// Exposes all the execute functions available in the contract.
/// ## Params
/// * **deps** is an object of type [`DepsMut`].
///
/// * **env** is an object of type [`Env`].
///
/// * **info** is an object of type [`MessageInfo`].
///
/// * **msg** is an object of type [`ExecuteMsg`].
///
/// ## Execute messages
///
/// * **ExecuteMsg::Receive(msg)** Parse incoming messages from the ASTRO token.
///
/// * **ExecuteMsg::UpdateConfig {
///             owner,
///             auction_contract_address,
///             merkle_roots,
///             from_timestamp,
///             to_timestamp,
///         }** Admin function to update any of the configuration parameters.
///
/// * **ExecuteMsg::Claim {
///             claim_amount,
///             merkle_proof,
///             root_index,
///         }** Executes an airdrop claim for Users.
///
/// * **ExecuteMsg::DelegateAstroToBootstrapAuction { amount_to_delegate }** Delegates ASTRO to bootstrap auction contract.
///
/// * **ExecuteMsg::EnableClaims {}** Enables ASTRO withdrawals by the airdrop recipients.
///
/// * **ExecuteMsg::WithdrawAirdropReward {}** Facilitates ASTRO withdrawal for airdrop recipients
///
/// * **ExecuteMsg::TransferUnclaimedTokens { recipient, amount }** Transfers unclaimed ASTRO tokens available with the contract to the recipient address.
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

/// Receives a message of type [`Cw20ReceiveMsg`] and processes it depending on the received template.
/// If the template is not found in the received message, then a [`ContractError`] is returned,
/// otherwise returns the [`Response`] with the specified attributes if the operation was successful
/// ## Params
/// * **deps** is an object of type [`DepsMut`].
///
/// * **env** is an object of type [`Env`].
///
/// * **info** is an object of type [`MessageInfo`].
///
/// * **cw20_msg** is an object of type [`Cw20ReceiveMsg`]. This is the CW20 message to process.
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

/// Exposes all the queries available in the contract.
/// ## Params
/// * **deps** is an object of type [`Deps`].
///
/// * **_env** is an object of type [`Env`].
///
/// * **msg** is an object of type [`QueryMsg`].
///
/// ## Queries
/// * **QueryMsg::Config {}** Returns the config info.
///
/// * **QueryMsg::State {}** Returns the contract's state info.
///
/// * **QueryMsg::HasUserClaimed { address }** Returns a boolean value indicating
/// if the corresponding address have yet claimed their airdrop or not.
///
/// * **QueryMsg::UserInfo { address }** Returns user's airdrop claim state.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&CONFIG.load(deps.storage)?),
        QueryMsg::State {} => to_binary(&STATE.load(deps.storage)?),
        QueryMsg::HasUserClaimed { address } => to_binary(&query_user_claimed(deps, address)?),
        QueryMsg::UserInfo { address } => to_binary(&query_user_info(deps, address)?),
    }
}

/// Used for contract migration. Returns a default object of type [`Response`].
/// ## Params
/// * **_deps** is an object of type [`DepsMut`].
///
/// * **_env** is an object of type [`Env`].
///
/// * **_msg** is an object of type [`MigrateMsg`].
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}

/// Admin function to update configuration parameters. Returns a [`StdError`] on failure.
/// ## Params
/// * **deps** is an object of type [`DepsMut`].
///
/// * **_env** is an object of type [`Env`].
///
/// * **info** is an object of type [`MessageInfo`].
///
/// * **owner** is an optional object of type [`String`] that contains address of the new owner.
///
/// * **auction_contract_address** is an optional object of type [`String`] that contains address of the new auction contract address.
///
/// * **merkle_roots** is an optional vector of type [`String`] that contains new Markle roots.
///
/// * **from_timestamp** is an optional object of type [`u64`].
///
/// * **to_timestamp** is an optional object of type [`u64`].
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
        config.owner = addr_validate_to_lower(deps.api, &owner)?;
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

/// Increases ASTRO incentives. Returns a [`StdError`] on failure.
/// ## Params
/// * **deps** is an object of type [`DepsMut`].
///
/// * **amount** is an object of type [`Uint128`].
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

/// Executes an airdrop claim for Users. Returns a [`StdError`] on failure.
/// ## Params
/// * **deps** is an object of type [`DepsMut`].
///
/// * **env** is an object of type [`Env`].
///
/// * **info** is an object of type [`MessageInfo`].
///
/// * **claim_amount** is an object of type [`Uint128`]. Airdrop to be claimed by the user
///
/// * **merkle_proof** is a vector of type [`String`]. Array of hashes to prove the input is a leaf of the Merkle Tree
///
/// * **root_index** is a vector of type [`u32`]. Merkle Tree root identifier to be used for verification
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

    let merkle_root = config
        .merkle_roots
        .get(root_index as usize)
        .ok_or_else(|| StdError::generic_err("Incorrect Merkle Root Index"))?;

    if !verify_claim(&recipient, claim_amount, merkle_proof, merkle_root)? {
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

/// Transfers unclaimed tokens. Returns a [`StdError`] on failure.
/// ## Params
/// * **deps** is an object of type [`DepsMut`].
///
/// * **env** is an object of type [`Env`].
///
/// * **info** is an object of type [`MessageInfo`].
///
/// * **recipient** is an object of type [`String`]. Recipient receiving the ASTRO tokens
///
/// * **amount** is an object of type [`Uint128`]. Amount of ASTRO to be transferred
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
        addr_validate_to_lower(deps.api, &recipient)?,
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

/// Returns details around user's ASTRO Airdrop claim. Returns a [`StdError`] on failure.
/// ## Params
/// * **deps** is an object of type [`Deps`].
///
/// * **user_address** is an object of type [`String`].
fn query_user_info(deps: Deps, user_address: String) -> StdResult<UserInfoResponse> {
    let user_address = addr_validate_to_lower(deps.api, &user_address)?;
    let user_info = USERS
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();
    Ok(UserInfoResponse {
        airdrop_amount: user_info.airdrop_amount,
    })
}

/// Returns a boolean value indicating if the corresponding address have yet claimed their airdrop or not. Returns a [`StdError`] on failure.
/// ## Params
/// * **deps** is an object of type [`Deps`].
///
/// * **address** is an object of type [`String`].
fn query_user_claimed(deps: Deps, address: String) -> StdResult<ClaimResponse> {
    let user_address = addr_validate_to_lower(deps.api, &address)?;
    let user_info = USERS
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    Ok(ClaimResponse {
        is_claimed: !user_info.airdrop_amount.is_zero(),
    })
}
