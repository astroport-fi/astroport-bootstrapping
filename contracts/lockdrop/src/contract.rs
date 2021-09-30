use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::{
    entry_point, to_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Env,
    MessageInfo, QueryRequest, Response, StdError, StdResult, Uint128, WasmMsg, WasmQuery,
};

use mars::address_provider::helpers::{query_address, query_addresses};
use mars::address_provider::msg::MarsContract;
use mars::helpers::{cw20_get_balance, option_string_to_addr, zero_address};
use mars::incentives::msg::QueryMsg::UserUnclaimedRewards;
use mars::tax::deduct_tax;

use mars_periphery::lockdrop::{
    CallbackMsg, ConfigResponse, ExecuteMsg, GlobalStateResponse, InstantiateMsg,
    LockUpInfoResponse, QueryMsg, UpdateConfigMsg, UserInfoResponse,
};

use crate::state::{Config, State, UserInfo, CONFIG, LOCKUP_INFO, STATE, USER_INFO};


//----------------------------------------------------------------------------------------
// Entry Points
//----------------------------------------------------------------------------------------

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    // CHECK :: init_timestamp needs to be valid
    if msg.init_timestamp < _env.block.time.seconds() {
        return Err(StdError::generic_err("Invalid timestamp"));
    }
    // CHECK :: deposit_window,withdrawal_window need to be valid (withdrawal_window < deposit_window)
    if msg.deposit_window == 0u64
        || msg.withdrawal_window == 0u64
        || msg.deposit_window <= msg.withdrawal_window
    {
        return Err(StdError::generic_err("Invalid deposit / withdraw window"));
    }

    // CHECK :: min_lock_duration , max_lock_duration need to be valid (min_lock_duration < max_lock_duration)
    if msg.max_duration <= msg.min_duration {
        return Err(StdError::generic_err("Invalid Lockup durations"));
    }

    let config = Config {
        owner: deps.api.addr_validate(&msg.owner)?,
        address_provider: option_string_to_addr(deps.api, msg.address_provider, zero_address())?,
        ma_ust_token: option_string_to_addr(deps.api, msg.ma_ust_token, zero_address())?,
        init_timestamp: msg.init_timestamp,
        deposit_window: msg.deposit_window,
        withdrawal_window: msg.withdrawal_window,
        min_lock_duration: msg.min_duration,
        max_lock_duration: msg.max_duration,
        seconds_per_week: msg.seconds_per_week,
        weekly_multiplier: msg.weekly_multiplier.unwrap_or(Decimal256::zero()),
        denom: msg.denom.unwrap_or("uusd".to_string()),
        lockdrop_incentives: msg.lockdrop_incentives.unwrap_or(Uint256::zero()),
    };

    let state = State {
        final_ust_locked: Uint256::zero(),
        final_maust_locked: Uint256::zero(),
        total_ust_locked: Uint256::zero(),
        total_maust_locked: Uint256::zero(),
        total_deposits_weight: Uint256::zero(),
        global_reward_index: Decimal256::zero(),
    };

    CONFIG.save(deps.storage, &config)?;
    STATE.save(deps.storage, &state)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> StdResult<Response> {
    match msg {
        ExecuteMsg::UpdateConfig { new_config } => update_config(deps, _env, info, new_config),
        ExecuteMsg::DepositUst { duration } => try_deposit_ust(deps, _env, info, duration),
        ExecuteMsg::WithdrawUst { duration, amount } => {
            try_withdraw_ust(deps, _env, info, duration, amount)
        }
        ExecuteMsg::DepositUstInRedBank {} => try_deposit_in_red_bank(deps, _env, info),
        ExecuteMsg::ClaimRewards {} => try_claim(deps, _env, info),
        ExecuteMsg::Unlock { duration } => try_unlock_position(deps, _env, info, duration),
        ExecuteMsg::Callback(msg) => _handle_callback(deps, _env, info, msg),
    }
}

fn _handle_callback(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: CallbackMsg,
) -> StdResult<Response> {
    // Callback functions can only be called this contract itself
    if info.sender != env.contract.address {
        return Err(StdError::generic_err(
            "callbacks cannot be invoked externally",
        ));
    }
    match msg {
        CallbackMsg::UpdateStateOnRedBankDeposit {
            prev_ma_ust_balance,
        } => update_state_on_red_bank_deposit(deps, env, prev_ma_ust_balance),
        CallbackMsg::UpdateStateOnClaim {
            user,
            prev_xmars_balance,
        } => update_state_on_claim(deps, env, user, prev_xmars_balance),
        CallbackMsg::DissolvePosition { user, duration } => {
            try_dissolve_position(deps, env, user, duration)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State {} => to_binary(&query_state(deps)?),
        QueryMsg::UserInfo { address } => to_binary(&query_user_info(deps, _env, address)?),
        QueryMsg::LockUpInfo { address, duration } => {
            to_binary(&query_lockup_info(deps, address, duration)?)
        }
        QueryMsg::LockUpInfoWithId { lockup_id } => {
            to_binary(&query_lockup_info_with_id(deps, lockup_id)?)
        }
    }
}

//----------------------------------------------------------------------------------------
// Handle Functions
//----------------------------------------------------------------------------------------

pub fn update_config(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    new_config: UpdateConfigMsg,
) -> StdResult<Response> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.owner {
        return Err(StdError::generic_err("Only owner can update configuration"));
    }

    // UPDATE :: ADDRESSES IF PROVIDED
    config.address_provider = option_string_to_addr(
        deps.api,
        new_config.address_provider,
        config.address_provider,
    )?;
    config.ma_ust_token =
        option_string_to_addr(deps.api, new_config.ma_ust_token, config.ma_ust_token)?;
    config.owner = option_string_to_addr(deps.api, new_config.owner, config.owner)?;

    // UPDATE :: init_timestamp (if provided) :: ALLOWED BEFORE THE LOCKUP DEPOSIT WINDOW OPENS
    if env.block.time.seconds() < config.init_timestamp {
        config.init_timestamp = new_config.init_timestamp.unwrap_or(config.init_timestamp);
        config.min_lock_duration = new_config.min_duration.unwrap_or(config.min_lock_duration);
        config.max_lock_duration = new_config.max_duration.unwrap_or(config.max_lock_duration);
        config.weekly_multiplier = new_config
            .weekly_multiplier
            .unwrap_or(config.weekly_multiplier);
    }

    // LOCKDROP INCENTIVES
    config.lockdrop_incentives = new_config
        .lockdrop_incentives
        .unwrap_or(config.lockdrop_incentives);

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "lockdrop::ExecuteMsg::UpdateConfig"))
}

// USER SENDS UST --> USER'S LOCKUP POSITION IS UPDATED
pub fn try_deposit_ust(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    duration: u64,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // UST DEPOSITED & USER ADDRESS
    let deposit_amount = get_denom_amount_from_coins(&info.funds, &config.denom);
    let depositor_address = info.sender.clone();

    // CHECK :: Lockdrop deposit window open
    if !is_deposit_open(env.block.time.seconds(), &config) {
        return Err(StdError::generic_err("Deposit window closed"));
    }

    // CHECK :: Valid Deposit Amount
    if deposit_amount == Uint256::zero() {
        return Err(StdError::generic_err("Amount cannot be zero"));
    }

    // CHECK :: Valid Lockup Duration
    if duration > config.max_lock_duration || duration < config.min_lock_duration {
        return Err(StdError::generic_err(format!(
            "Lockup duration needs to be between {} and {}",
            config.min_lock_duration, config.max_lock_duration
        )));
    }

    // LOCKUP INFO :: RETRIEVE --> UPDATE
    let lockup_id = depositor_address.clone().to_string() + &duration.to_string();
    let mut lockup_info = LOCKUP_INFO
        .may_load(deps.storage, lockup_id.clone().as_bytes())?
        .unwrap_or_default();
    lockup_info.ust_locked += deposit_amount;
    lockup_info.duration = duration;
    lockup_info.unlock_timestamp = calculate_unlock_timestamp(&config, duration);

    // USER INFO :: RETRIEVE --> UPDATE
    let mut user_info = USER_INFO
        .may_load(deps.storage, &depositor_address.clone())?
        .unwrap_or_default();
    user_info.total_ust_locked += deposit_amount;
    if !is_lockup_present_in_user_info(&user_info, lockup_id.clone()) {
        user_info.lockup_positions.push(lockup_id.clone());
    }

    // STATE :: UPDATE --> SAVE
    state.total_ust_locked += deposit_amount;
    state.total_deposits_weight +=
        calculate_weight(deposit_amount, duration, config.weekly_multiplier);

    STATE.save(deps.storage, &state)?;
    LOCKUP_INFO.save(deps.storage, lockup_id.clone().as_bytes(), &lockup_info)?;
    USER_INFO.save(deps.storage, &depositor_address, &user_info)?;

    Ok(Response::new().add_attributes(vec![
        ("action", "lockdrop::ExecuteMsg::LockUST"),
        ("user", &depositor_address.to_string()),
        ("duration", duration.to_string().as_str()),
        ("ust_deposited", deposit_amount.to_string().as_str()),
    ]))
}

// USER WITHDRAWS UST --> USER'S LOCKUP POSITION IS UPDATED
pub fn try_withdraw_ust(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    duration: u64,
    withdraw_amount: Uint256,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // USER ADDRESS AND LOCKUP DETAILS
    let withdrawer_address = info.sender.clone();
    let lockup_id = withdrawer_address.clone().to_string() + &duration.to_string();
    let mut lockup_info = LOCKUP_INFO
        .may_load(deps.storage, lockup_id.clone().as_bytes())?
        .unwrap_or_default();

    // CHECK :: Lockdrop withdrawal window open
    if !is_withdraw_open(env.block.time.seconds(), &config) {
        return Err(StdError::generic_err("Withdrawals not allowed"));
    }

    // CHECK :: Valid Lockup
    if lockup_info.ust_locked == Uint256::zero() {
        return Err(StdError::generic_err("Lockup doesn't exist"));
    }

    // CHECK :: Valid Withdraw Amount
    if withdraw_amount == Uint256::zero() || withdraw_amount > lockup_info.ust_locked {
        return Err(StdError::generic_err("Invalid withdrawal request"));
    }

    // LOCKUP INFO :: RETRIEVE --> UPDATE
    lockup_info.ust_locked = lockup_info.ust_locked - withdraw_amount;

    // USER INFO :: RETRIEVE --> UPDATE
    let mut user_info = USER_INFO
        .may_load(deps.storage, &withdrawer_address.clone())?
        .unwrap_or_default();
    user_info.total_ust_locked = user_info.total_ust_locked - withdraw_amount;
    if lockup_info.ust_locked == Uint256::zero() {
        remove_lockup_pos_from_user_info(&mut user_info, lockup_id.clone());
    }

    // STATE :: UPDATE --> SAVE
    state.total_ust_locked = state.total_ust_locked - withdraw_amount;
    state.total_deposits_weight = state.total_deposits_weight
        - calculate_weight(withdraw_amount, duration, config.weekly_multiplier);

    STATE.save(deps.storage, &state)?;
    LOCKUP_INFO.save(deps.storage, lockup_id.clone().as_bytes(), &lockup_info)?;
    USER_INFO.save(deps.storage, &withdrawer_address, &user_info)?;

    // COSMOS_MSG ::TRANSFER WITHDRAWN UST
    let withdraw_msg = build_send_native_asset_msg(
        deps.as_ref(),
        withdrawer_address.clone(),
        &config.denom.clone(),
        withdraw_amount,
    )?;

    Ok(Response::new()
        .add_messages(vec![withdraw_msg])
        .add_attributes(vec![
            ("action", "lockdrop::ExecuteMsg::WithdrawUST"),
            ("user", &withdrawer_address.to_string()),
            ("duration", duration.to_string().as_str()),
            ("ust_withdrawn", withdraw_amount.to_string().as_str()),
        ]))
}

// ADMIN FUNCTION :: DEPOSITS UST INTO THE RED BANK AND UPDATES STATE VIA THE CALLBANK FUNCTION
pub fn try_deposit_in_red_bank(deps: DepsMut, env: Env, info: MessageInfo) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;

    // CHECK :: Only Owner can call this function
    if info.sender != config.owner {
        return Err(StdError::generic_err("Unauthorized"));
    }

    // CHECK :: Lockdrop deposit window should be closed
    if env.block.time.seconds() < config.init_timestamp
        || is_deposit_open(env.block.time.seconds(), &config)
    {
        return Err(StdError::generic_err(
            "Lockdrop deposits haven't concluded yet",
        ));
    }

    // CHECK :: Revert in-case funds have already been deposited in red-bank
    if state.final_maust_locked > Uint256::zero() {
        return Err(StdError::generic_err("Already deposited"));
    }

    // FETCH CURRENT BALANCES (UST / maUST), PREPARE DEPOSIT MSG
    let red_bank = query_address(
        &deps.querier,
        config.address_provider,
        MarsContract::RedBank,
    )?;
    let ma_ust_balance = Uint256::from(cw20_get_balance(
        &deps.querier,
        config.ma_ust_token.clone(),
        env.contract.address.clone(),
    )?);

    // COSMOS_MSG :: DEPOSIT UST IN RED BANK
    let deposit_msg = build_deposit_into_redbank_msg(
        deps.as_ref(),
        red_bank,
        config.denom.clone(),
        state.total_ust_locked,
    )?;

    // COSMOS_MSG :: UPDATE CONTRACT STATE
    let update_state_msg = CallbackMsg::UpdateStateOnRedBankDeposit {
        prev_ma_ust_balance: ma_ust_balance,
    }
    .to_cosmos_msg(&env.contract.address)?;

    Ok(Response::new()
        .add_messages(vec![deposit_msg, update_state_msg])
        .add_attributes(vec![
            ("action", "lockdrop::ExecuteMsg::DepositInRedBank"),
            (
                "ust_deposited_in_red_bank",
                state.total_ust_locked.to_string().as_str(),
            ),
            ("timestamp", env.block.time.seconds().to_string().as_str()),
        ]))
}

// USER CLAIMS REWARDS ::: claim xMARS --> UpdateStateOnClaim(callback)
pub fn try_claim(deps: DepsMut, env: Env, info: MessageInfo) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let user_address = info.sender.clone();
    let user_info = USER_INFO
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    // CHECK :: REWARDS CAN BE CLAIMED
    if is_deposit_open(env.block.time.seconds(), &config) {
        return Err(StdError::generic_err("Claim not allowed"));
    }
    // CHECK :: HAS VALID LOCKUP POSITIONS
    if user_info.total_ust_locked == Uint256::zero() {
        return Err(StdError::generic_err("No lockup to claim rewards for"));
    }

    // QUERY:: Contract addresses
    let mars_contracts = vec![MarsContract::Incentives, MarsContract::XMarsToken];
    let mut addresses_query =
        query_addresses(&deps.querier, config.address_provider, mars_contracts)?;
    // XMARS TOKEN ADDRESS
    let xmars_address = addresses_query.pop().unwrap();
    // UST RED BANK INCENTIVIZATION CONTRACT
    let incentives_address = addresses_query.pop().unwrap();

    // QUERY :: ARE XMARS REWARDS TO BE CLAIMED > 0 ?
    let xmars_unclaimed: Uint128 = deps
        .querier
        .query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: incentives_address.to_string(),
            msg: to_binary(&UserUnclaimedRewards {
                user_address: env.contract.address.to_string(),
            })
            .unwrap(),
        }))
        .unwrap();

    // Get XMARS Balance
    let xmars_balance: cw20::BalanceResponse = deps
        .querier
        .query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: xmars_address.to_string(),
            msg: to_binary(&mars::xmars_token::msg::QueryMsg::Balance {
                address: env.contract.address.to_string(),
            })
            .unwrap(),
        }))
        .unwrap();

    let mut messages_ = vec![];

    // COSMOS MSG's :: IF UNCLAIMED XMARS REWARDS > 0, CLAIM THESE REWARDS
    if xmars_unclaimed > Uint128::zero() {
        messages_.push(build_claim_xmars_rewards(incentives_address.clone())?);
    }

    // COSMOS MSG's ::  UPDATE STATE VIA CALLBACK
    let callback_msg = CallbackMsg::UpdateStateOnClaim {
        user: user_address,
        prev_xmars_balance: xmars_balance.balance.into(),
    }
    .to_cosmos_msg(&env.contract.address)?;
    messages_.push(callback_msg);

    Ok(Response::new().add_messages(messages_).add_attributes(vec![
        ("action", "lockdrop::ExecuteMsg::ClaimRewards"),
        ("unclaimed_xMars", &xmars_unclaimed.to_string()),
    ]))
}

// USER UNLOCKS UST --> CONTRACT WITHDRAWS FROM RED BANK --> STATE UPDATED VIA EXTEND MSG
pub fn try_unlock_position(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    duration: u64,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;
    let depositor_address = info.sender.clone();

    // LOCKUP INFO :: RETRIEVE
    let lockup_id = depositor_address.clone().to_string() + &duration.to_string();
    let lockup_info = LOCKUP_INFO
        .may_load(deps.storage, lockup_id.as_bytes())?
        .unwrap_or_default();

    // CHECK :: IS VALID LOCKUP
    if lockup_info.ust_locked == Uint256::zero() {
        return Err(StdError::generic_err("Invalid lockup"));
    }

    // CHECK :: LOCKUP CAN BE UNLOCKED
    if lockup_info.unlock_timestamp > env.block.time.seconds() {
        let time_remaining = lockup_info.unlock_timestamp - env.block.time.seconds();
        return Err(StdError::generic_err(format!(
            "{} seconds to Unlock",
            time_remaining
        )));
    }

    // MaUST :: AMOUNT TO BE SENT TO THE USER
    let maust_unlocked = calculate_user_ma_ust_share(
        lockup_info.ust_locked,
        state.final_ust_locked,
        state.final_maust_locked,
    );

    // QUERY:: Contract addresses
    let mars_contracts = vec![MarsContract::Incentives, MarsContract::XMarsToken];
    let mut addresses_query =
        query_addresses(&deps.querier, config.address_provider, mars_contracts)?;
    let xmars_address = addresses_query.pop().unwrap();
    let incentives_address = addresses_query.pop().unwrap();

    // QUERY :: XMARS Balance
    let xmars_balance: cw20::BalanceResponse = deps
        .querier
        .query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: xmars_address.to_string(),
            msg: to_binary(&mars::xmars_token::msg::QueryMsg::Balance {
                address: env.contract.address.to_string(),
            })
            .unwrap(),
        }))
        .unwrap();

    // QUERY :: ARE XMARS REWARDS TO BE CLAIMED > 0 ?
    let xmars_unclaimed: Uint128 = deps
        .querier
        .query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: incentives_address.to_string(),
            msg: to_binary(&UserUnclaimedRewards {
                user_address: env.contract.address.to_string(),
            })
            .unwrap(),
        }))
        .unwrap();

    let mut messages_ = vec![];

    // COSMOS MSG's :: IF UNCLAIMED XMARS REWARDS > 0, CLAIM THESE REWARDS (before dissolving lockup position)
    if xmars_unclaimed > Uint128::zero() {
        messages_.push(build_claim_xmars_rewards(incentives_address.clone())?);
    }

    // CALLBACK MSG :: UPDATE STATE ON CLAIM (before dissolving lockup position)
    let callback_claim_xmars_msg = CallbackMsg::UpdateStateOnClaim {
        user: depositor_address.clone(),
        prev_xmars_balance: xmars_balance.balance.into(),
    }
    .to_cosmos_msg(&env.contract.address)?;
    messages_.push(callback_claim_xmars_msg);
    // CALLBACK MSG :: DISSOLVE LOCKUP POSITION
    let callback_dissolve_position_msg = CallbackMsg::DissolvePosition {
        user: depositor_address.clone(),
        duration: duration,
    }
    .to_cosmos_msg(&env.contract.address)?;
    messages_.push(callback_dissolve_position_msg);

    // COSMOS MSG :: TRANSFER USER POSITION's MA-UST SHARE
    let maust_transfer_msg = build_send_cw20_token_msg(
        depositor_address.clone(),
        config.ma_ust_token,
        maust_unlocked,
    )?;
    messages_.push(maust_transfer_msg);

    Ok(Response::new().add_messages(messages_).add_attributes(vec![
        ("action", "lockdrop::ExecuteMsg::UnlockPosition"),
        ("owner", info.sender.as_str()),
        ("duration", duration.to_string().as_str()),
        ("maUST_unlocked", maust_unlocked.to_string().as_str()),
    ]))
}

//----------------------------------------------------------------------------------------
// Callback Functions
//----------------------------------------------------------------------------------------

// CALLBACK :: CALLED AFTER UST DEPOSITED INTO RED BANK --> UPDATES CONTRACT STATE
pub fn update_state_on_red_bank_deposit(
    deps: DepsMut,
    env: Env,
    prev_ma_ust_balance: Uint256,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    let cur_ma_ust_balance = Uint256::from(cw20_get_balance(
        &deps.querier,
        config.ma_ust_token.clone(),
        env.contract.address.clone(),
    )?);
    let m_ust_minted = cur_ma_ust_balance - prev_ma_ust_balance;
    // STATE :: UPDATE --> SAVE
    state.final_ust_locked = state.total_ust_locked;
    state.final_maust_locked = m_ust_minted;
    state.total_ust_locked = Uint256::zero();
    state.total_maust_locked = m_ust_minted;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new().add_attributes(vec![
        ("action", "lockdrop::CallbackMsg::RedBankDeposit"),
        ("maUST_minted", m_ust_minted.to_string().as_str()),
    ]))
}

// CALLBACK :: CALLED AFTER XMARS CLAIMED BY CONTRACT --> TRANSFER REWARDS (MARS, XMARS) TO THE USER
pub fn update_state_on_claim(
    deps: DepsMut,
    env: Env,
    user: Addr,
    prev_xmars_balance: Uint256,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?; // Index is updated
    let mut user_info = USER_INFO.may_load(deps.storage, &user)?.unwrap_or_default();
    // QUERY:: Contract addresses
    let mars_contracts = vec![MarsContract::MarsToken, MarsContract::XMarsToken];
    let mut addresses_query = query_addresses(
        &deps.querier,
        config.address_provider.clone(),
        mars_contracts,
    )?;
    let xmars_address = addresses_query.pop().unwrap();
    let mars_address = addresses_query.pop().unwrap();

    // Get XMARS Balance
    let cur_xmars_balance: cw20::BalanceResponse = deps
        .querier
        .query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: xmars_address.to_string(),
            msg: to_binary(&mars::xmars_token::msg::QueryMsg::Balance {
                address: env.contract.address.to_string(),
            })
            .unwrap(),
        }))
        .unwrap();
    // XMARS REWARDS CLAIMED (can be 0 )
    let xmars_accured = Uint256::from(cur_xmars_balance.balance) - prev_xmars_balance;

    let mut total_mars_rewards = Uint256::zero();

    // LOCKDROP :: LOOP OVER ALL LOCKUP POSITIONS TO CALCULATE THE LOCKDROP REWARD (if its not already claimed)
    if !user_info.lockdrop_claimed {
        let mut rewards: Uint256;
        for lockup_id in &mut user_info.lockup_positions {
            let mut lockup_info = LOCKUP_INFO
                .may_load(deps.storage, lockup_id.as_bytes())?
                .unwrap_or_default();
            rewards = calculate_lockdrop_reward(
                lockup_info.ust_locked,
                lockup_info.duration,
                &config,
                state.total_deposits_weight,
            );
            lockup_info.lockdrop_reward = rewards;
            total_mars_rewards += rewards;
            LOCKUP_INFO.save(deps.storage, lockup_id.as_bytes(), &lockup_info)?;
        }
        user_info.lockdrop_claimed = true;
    }

    let mut total_xmars_rewards = Uint256::zero();

    // UPDATE :: GLOBAL INDEX (XMARS rewards tracker)
    // TO BE CLAIMED :::: CALCULATE ACCRUED X-MARS AS DEPOSIT INCENTIVES
    if xmars_accured > Uint256::zero() {
        update_xmars_rewards_index(&mut state, xmars_accured);
        compute_user_accrued_reward(&state, &mut user_info);
        total_xmars_rewards = user_info.pending_xmars;
        user_info.pending_xmars = Uint256::zero();
    }

    // SAVE UPDATED STATES
    STATE.save(deps.storage, &state)?;
    USER_INFO.save(deps.storage, &user.clone(), &user_info)?;

    let mut messages_ = vec![];
    // COSMOS MSG :: SEND MARS (LOCKDROP REWARD) IF > 0
    if total_mars_rewards > Uint256::zero() {
        let transfer_mars_msg =
            build_send_cw20_token_msg(user.clone(), mars_address, total_mars_rewards)?;
        messages_.push(transfer_mars_msg);
    }
    // COSMOS MSG :: SEND X-MARS (DEPOSIT INCENTIVES) IF > 0
    if total_xmars_rewards > Uint256::zero() {
        let transfer_xmars_msg =
            build_send_cw20_token_msg(user.clone(), xmars_address, total_xmars_rewards)?;
        messages_.push(transfer_xmars_msg);
    }

    Ok(Response::new().add_messages(messages_).add_attributes(vec![
        ("action", "lockdrop::CallbackMsg::ClaimRewards"),
        ("total_xmars_claimed", xmars_accured.to_string().as_str()),
        ("user", &user.to_string()),
        ("mars_claimed", total_mars_rewards.to_string().as_str()),
        ("xmars_claimed", total_xmars_rewards.to_string().as_str()),
    ]))
}

// CALLBACK :: CALLED BY try_unlock_position FUNCTION --> DELETES LOCKUP POSITION
pub fn try_dissolve_position(
    deps: DepsMut,
    _env: Env,
    user: Addr,
    duration: u64,
) -> StdResult<Response> {
    // RETRIEVE :: State, User_Info and lockup position
    let mut state = STATE.load(deps.storage)?; // total_maust_locked is updated
    let mut user_info = USER_INFO.may_load(deps.storage, &user)?.unwrap_or_default();
    let lockup_id = user.to_string() + &duration.to_string();
    let mut lockup_info = LOCKUP_INFO
        .may_load(deps.storage, lockup_id.clone().as_bytes())?
        .unwrap_or_default();

    // UPDATE STATE
    state.total_maust_locked = state.total_maust_locked
        - calculate_user_ma_ust_share(
            lockup_info.ust_locked,
            state.final_ust_locked,
            state.final_maust_locked,
        );

    // UPDATE USER INFO
    user_info.total_ust_locked = user_info.total_ust_locked - lockup_info.ust_locked;

    // DISSOLVE LOCKUP POSITION
    lockup_info.ust_locked = Uint256::zero();
    remove_lockup_pos_from_user_info(&mut user_info, lockup_id.clone());

    STATE.save(deps.storage, &state)?;
    USER_INFO.save(deps.storage, &user, &user_info)?;
    LOCKUP_INFO.save(deps.storage, lockup_id.clone().as_bytes(), &lockup_info)?;

    Ok(Response::new().add_attributes(vec![
        ("action", "lockdrop::Callback::DissolvePosition"),
        ("user", user.clone().as_str()),
        ("duration", duration.to_string().as_str()),
    ]))
}

//----------------------------------------------------------------------------------------
// Query Functions
//----------------------------------------------------------------------------------------

/// @dev Returns the contract's configuration
pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;

    Ok(ConfigResponse {
        owner: config.owner.to_string(),
        address_provider: config.address_provider.to_string(),
        ma_ust_token: config.ma_ust_token.to_string(),
        init_timestamp: config.init_timestamp,
        deposit_window: config.deposit_window,
        withdrawal_window: config.withdrawal_window,
        min_duration: config.min_lock_duration,
        max_duration: config.max_lock_duration,
        multiplier: config.weekly_multiplier,
        lockdrop_incentives: config.lockdrop_incentives,
    })
}

/// @dev Returns the contract's Global State
pub fn query_state(deps: Deps) -> StdResult<GlobalStateResponse> {
    let state: State = STATE.load(deps.storage)?;
    Ok(GlobalStateResponse {
        final_ust_locked: state.final_ust_locked,
        final_maust_locked: state.final_maust_locked,
        total_ust_locked: state.total_ust_locked,
        total_maust_locked: state.total_maust_locked,
        global_reward_index: state.global_reward_index,
        total_deposits_weight: state.total_deposits_weight,
    })
}

/// @dev Returns summarized details regarding the user
pub fn query_user_info(deps: Deps, env: Env, user: String) -> StdResult<UserInfoResponse> {
    let config = CONFIG.load(deps.storage)?;
    let user_address = deps.api.addr_validate(&user)?;
    let mut state: State = STATE.load(deps.storage)?;
    let mut user_info = USER_INFO
        .may_load(deps.storage, &user_address.clone())?
        .unwrap_or_default();

    // If address_provider is not set yet
    if config.address_provider == zero_address() {
        return Ok(UserInfoResponse {
            total_ust_locked: user_info.total_ust_locked,
            total_maust_locked: Uint256::zero(),
            lockup_position_ids: user_info.lockup_positions,
            is_lockdrop_claimed: user_info.lockdrop_claimed,
            reward_index: user_info.reward_index,
            pending_xmars: user_info.pending_xmars,
        })
    }

    // QUERY:: Contract addresses
    let mars_contracts = vec![MarsContract::Incentives];
    let mut addresses_query =
        query_addresses(&deps.querier, config.address_provider, mars_contracts)?;
    let incentives_address = addresses_query.pop().unwrap();

    // QUERY :: XMARS REWARDS TO BE CLAIMED  ?
    let xmars_accured: Uint128 = deps
        .querier
        .query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: incentives_address.to_string(),
            msg: to_binary(&UserUnclaimedRewards {
                user_address: env.contract.address.to_string(),
            })
            .unwrap(),
        }))
        .unwrap();

    update_xmars_rewards_index(&mut state, Uint256::from(xmars_accured));
    compute_user_accrued_reward(&state, &mut user_info);
    Ok(UserInfoResponse {
        total_ust_locked: user_info.total_ust_locked,
        total_maust_locked: calculate_user_ma_ust_share(
            user_info.total_ust_locked,
            state.final_ust_locked,
            state.final_maust_locked,
        ),
        lockup_position_ids: user_info.lockup_positions,
        is_lockdrop_claimed: user_info.lockdrop_claimed,
        reward_index: user_info.reward_index,
        pending_xmars: user_info.pending_xmars,
    })
}

/// @dev Returns summarized details regarding the user
pub fn query_lockup_info(deps: Deps, user: String, duration: u64) -> StdResult<LockUpInfoResponse> {
    let lockup_id = user.to_string() + &duration.to_string();
    query_lockup_info_with_id(deps, lockup_id)
}

/// @dev Returns summarized details regarding the user
pub fn query_lockup_info_with_id(deps: Deps, lockup_id: String) -> StdResult<LockUpInfoResponse> {
    let lockup_info = LOCKUP_INFO
        .may_load(deps.storage, lockup_id.clone().as_bytes())?
        .unwrap_or_default();
    let state: State = STATE.load(deps.storage)?;

    let mut lockup_response = LockUpInfoResponse {
        duration: lockup_info.duration,
        ust_locked: lockup_info.ust_locked,
        maust_balance: calculate_user_ma_ust_share(
            lockup_info.ust_locked,
            state.final_ust_locked,
            state.final_maust_locked,
        ),
        lockdrop_reward: lockup_info.lockdrop_reward,
        unlock_timestamp: lockup_info.unlock_timestamp,
    };

    if lockup_response.lockdrop_reward == Uint256::zero() {
        let config = CONFIG.load(deps.storage)?;
        lockup_response.lockdrop_reward = calculate_lockdrop_reward(
            lockup_response.ust_locked,
            lockup_response.duration,
            &config,
            state.total_deposits_weight,
        );
    }

    Ok(lockup_response)
}

//----------------------------------------------------------------------------------------
// HELPERS
//----------------------------------------------------------------------------------------

/// true if deposits are allowed
fn is_deposit_open(current_timestamp: u64, config: &Config) -> bool {
    let deposits_opened_till = config.init_timestamp + config.deposit_window;
    (current_timestamp >= config.init_timestamp) && (deposits_opened_till >= current_timestamp)
}

/// true if withdrawals are allowed
fn is_withdraw_open(current_timestamp: u64, config: &Config) -> bool {
    let withdrawals_opened_till = config.init_timestamp + config.withdrawal_window;
    (current_timestamp >= config.init_timestamp) && (withdrawals_opened_till >= current_timestamp)
}

/// Returns the timestamp when the lockup will get unlocked
fn calculate_unlock_timestamp(config: &Config, duration: u64) -> u64 {
    config.init_timestamp + config.deposit_window + (duration * config.seconds_per_week)
}

// Calculate Lockdrop Reward
fn calculate_lockdrop_reward(
    deposited_ust: Uint256,
    duration: u64,
    config: &Config,
    total_deposits_weight: Uint256,
) -> Uint256 {
    if total_deposits_weight == Uint256::zero() {
        return Uint256::zero();
    }
    let amount_weight = calculate_weight(deposited_ust, duration, config.weekly_multiplier);
    config.lockdrop_incentives * Decimal256::from_ratio(amount_weight, total_deposits_weight)
}

// Returns effective weight for the amount to be used for calculating airdrop rewards
fn calculate_weight(amount: Uint256, duration: u64, weekly_multiplier: Decimal256) -> Uint256 {
    let duration_weighted_amount = amount * Uint256::from(duration);
    duration_weighted_amount * weekly_multiplier
}

// native coins
fn get_denom_amount_from_coins(coins: &[Coin], denom: &str) -> Uint256 {
    coins
        .iter()
        .find(|c| c.denom == denom)
        .map(|c| Uint256::from(c.amount))
        .unwrap_or_else(Uint256::zero)
}

//-----------------------------
// MARS REWARDS COMPUTATION
//-----------------------------

// Accrue XMARS rewards by updating the reward index
fn update_xmars_rewards_index(state: &mut State, xmars_accured: Uint256) {
    if state.total_maust_locked == Uint256::zero() {
        return;
    }
    let xmars_rewards_index_increment = Decimal256::from_ratio(
        Uint256::from(xmars_accured),
        Uint256::from(state.total_maust_locked),
    );
    state.global_reward_index = state.global_reward_index + xmars_rewards_index_increment;
}

// Accrue MARS reward for the user by updating the user reward index and adding rewards to the pending rewards
fn compute_user_accrued_reward(state: &State, user_info: &mut UserInfo) {
    if state.final_ust_locked == Uint256::zero() {
        return;
    }
    let user_maust_share = calculate_user_ma_ust_share(
        user_info.total_ust_locked,
        state.final_ust_locked,
        state.final_maust_locked,
    );
    let pending_xmars = (user_maust_share * state.global_reward_index)
        - (user_maust_share * user_info.reward_index);
    user_info.reward_index = state.global_reward_index;
    user_info.pending_xmars += pending_xmars;
}

// Returns User's maUST Token share :: Calculated as =  (User's deposited UST / Final UST deposited) * Final maUST Locked
fn calculate_user_ma_ust_share(
    ust_locked_share: Uint256,
    final_ust_locked: Uint256,
    final_maust_locked: Uint256,
) -> Uint256 {
    if final_ust_locked == Uint256::zero() {
        return Uint256::zero();
    }
    final_maust_locked * Decimal256::from_ratio(ust_locked_share, final_ust_locked)
}

// Returns true if the user_info stuct's lockup_positions vector contains the lockup_id
fn is_lockup_present_in_user_info(user_info: &UserInfo, lockup_id: String) -> bool {
    if user_info.lockup_positions.iter().any(|id| id == &lockup_id) {
        return true;
    }
    false
}

// REMOVE LOCKUP INFO FROM lockup_positions array IN USER INFO
fn remove_lockup_pos_from_user_info(user_info: &mut UserInfo, lockup_id: String) {
    let index = user_info
        .lockup_positions
        .iter()
        .position(|x| *x == lockup_id)
        .unwrap();
    user_info.lockup_positions.remove(index);
}

//-----------------------------
// COSMOS_MSGs
//-----------------------------

fn build_send_native_asset_msg(
    deps: Deps,
    recipient: Addr,
    denom: &str,
    amount: Uint256,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Bank(BankMsg::Send {
        to_address: recipient.into(),
        amount: vec![deduct_tax(
            deps,
            Coin {
                denom: denom.to_string(),
                amount: amount.into(),
            },
        )?],
    }))
}

fn build_send_cw20_token_msg(
    recipient: Addr,
    token_contract_address: Addr,
    amount: Uint256,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: token_contract_address.into(),
        msg: to_binary(&cw20::Cw20ExecuteMsg::Transfer {
            recipient: recipient.into(),
            amount: amount.into(),
        })?,
        funds: vec![],
    }))
}

fn build_deposit_into_redbank_msg(
    deps: Deps,
    redbank_address: Addr,
    denom_stable: String,
    amount: Uint256,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: redbank_address.to_string(),
        funds: vec![deduct_tax(
            deps,
            Coin {
                denom: denom_stable.to_string(),
                amount: amount.into(),
            },
        )?],
        msg: to_binary(&mars::red_bank::msg::ExecuteMsg::DepositNative {
            denom: denom_stable,
        })?,
    }))
}

fn build_claim_xmars_rewards(incentives_contract: Addr) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: incentives_contract.to_string(),
        funds: vec![],
        msg: to_binary(&mars::incentives::msg::ExecuteMsg::ClaimRewards {})?,
    }))
}

//----------------------------------------------------------------------------------------
// TESTS
//----------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    use cosmwasm_std::testing::{MockApi, MockStorage, MOCK_CONTRACT_ADDR};
    use cosmwasm_std::{attr, coin, Coin, Decimal, OwnedDeps, SubMsg, Timestamp, Uint128};

    use mars::testing::{
        assert_generic_error_message, mock_dependencies, mock_env, mock_info, MarsMockQuerier,
        MockEnvParams,
    };

    use mars_periphery::lockdrop::{CallbackMsg, ExecuteMsg, InstantiateMsg, UpdateConfigMsg};

    #[test]
    fn test_proper_initialization() {
        let mut deps = mock_dependencies(&[]);
        let info = mock_info("owner");
        let env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(10_000_001),
            ..Default::default()
        });

        let mut base_config = InstantiateMsg {
            owner: "owner".to_string(),
            address_provider: None,
            ma_ust_token: None,
            init_timestamp: 10_000_000,
            deposit_window: 100000,
            withdrawal_window: 72000,
            min_duration: 1,
            max_duration: 5,
            seconds_per_week: 7 * 86400 as u64, 
            denom: Some("uusd".to_string()),
            weekly_multiplier: Some(Decimal256::from_ratio(9u64, 100u64)),
            lockdrop_incentives: None,
        };

        // ***
        // *** Test :: "Invalid timestamp" ***
        // ***
        base_config.init_timestamp = 10_000_000;
        let mut res_f = instantiate(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            base_config.clone(),
        );
        assert_generic_error_message(res_f, "Invalid timestamp");

        // ***
        // *** Test :: "Invalid deposit / withdraw window" ***
        // ***
        base_config.init_timestamp = 10_000_007;
        base_config.deposit_window = 15u64;
        base_config.withdrawal_window = 15u64;
        res_f = instantiate(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            base_config.clone(),
        );
        assert_generic_error_message(res_f, "Invalid deposit / withdraw window");

        // ***
        // *** Test :: "Invalid Lockup durations" ***
        // ***
        base_config.init_timestamp = 10_000_007;
        base_config.deposit_window = 15u64;
        base_config.withdrawal_window = 9u64;
        base_config.max_duration = 9u64;
        base_config.min_duration = 9u64;
        res_f = instantiate(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            base_config.clone(),
        );
        assert_generic_error_message(res_f, "Invalid Lockup durations");

        // ***
        // *** Test :: Should instantiate successfully ***
        // ***
        base_config.min_duration = 1u64;
        let res_s = instantiate(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            base_config.clone(),
        )
        .unwrap();
        assert_eq!(0, res_s.messages.len());
        // let's verify the config
        let config_ = query_config(deps.as_ref()).unwrap();
        assert_eq!("owner".to_string(), config_.owner);
        assert_eq!("".to_string(), config_.address_provider);
        assert_eq!("".to_string(), config_.ma_ust_token);
        assert_eq!(10_000_007, config_.init_timestamp);
        assert_eq!(15u64, config_.deposit_window);
        assert_eq!(9u64, config_.withdrawal_window);
        assert_eq!(1u64, config_.min_duration);
        assert_eq!(9u64, config_.max_duration);
        assert_eq!(Decimal256::from_ratio(9u64, 100u64), config_.multiplier);
        assert_eq!(Uint256::zero(), config_.lockdrop_incentives);

        // let's verify the state
        let state_ = query_state(deps.as_ref()).unwrap();
        assert_eq!(Uint256::zero(), state_.final_ust_locked);
        assert_eq!(Uint256::zero(), state_.final_maust_locked);
        assert_eq!(Uint256::zero(), state_.total_ust_locked);
        assert_eq!(Uint256::zero(), state_.total_maust_locked);
        assert_eq!(Decimal256::zero(), state_.global_reward_index);
        assert_eq!(Uint256::zero(), state_.total_deposits_weight);
    }

    #[test]
    fn test_update_config() {
        let mut deps = mock_dependencies(&[]);
        let mut info = mock_info("owner");
        let mut env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(1_000_000_00),
            ..Default::default()
        });

        // *** Instantiate successfully ***
        let base_config = InstantiateMsg {
            owner: "owner".to_string(),
            address_provider: None,
            ma_ust_token: None,
            init_timestamp: 1_000_000_05,
            deposit_window: 100000u64,
            withdrawal_window: 72000u64,
            min_duration: 1u64,
            max_duration: 5u64,
            seconds_per_week: 7 * 86400 as u64, 
            denom: Some("uusd".to_string()),
            weekly_multiplier: Some(Decimal256::from_ratio(9u64, 100u64)),
            lockdrop_incentives: None,
        };
        let res_s = instantiate(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            base_config.clone(),
        )
        .unwrap();
        assert_eq!(0, res_s.messages.len());

        // ***
        // *** Test :: Error "Only owner can update configuration" ***
        // ***
        info = mock_info("not_owner");
        let mut update_config = UpdateConfigMsg {
            owner: Some("new_owner".to_string()),
            address_provider: Some("new_address_provider".to_string()),
            ma_ust_token: Some("new_ma_ust_token".to_string()),
            init_timestamp: None,
            deposit_window: None,
            withdrawal_window: None,
            min_duration: None,
            max_duration: None,
            weekly_multiplier: None,
            lockdrop_incentives: None,
        };
        let mut update_config_msg = ExecuteMsg::UpdateConfig {
            new_config: update_config.clone(),
        };

        let res_f = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            update_config_msg.clone(),
        );
        assert_generic_error_message(res_f, "Only owner can update configuration");

        // ***
        // *** Test :: Update addresses successfully ***
        // ***
        info = mock_info("owner");
        let update_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            update_config_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            update_s.attributes,
            vec![attr("action", "lockdrop::ExecuteMsg::UpdateConfig")]
        );
        // let's verify the config
        let mut config_ = query_config(deps.as_ref()).unwrap();
        assert_eq!("new_owner".to_string(), config_.owner);
        assert_eq!("new_address_provider".to_string(), config_.address_provider);
        assert_eq!("new_ma_ust_token".to_string(), config_.ma_ust_token);
        assert_eq!(1_000_000_05, config_.init_timestamp);
        assert_eq!(100000u64, config_.deposit_window);
        assert_eq!(72000u64, config_.withdrawal_window);
        assert_eq!(1u64, config_.min_duration);
        assert_eq!(5u64, config_.max_duration);
        assert_eq!(Decimal256::from_ratio(9u64, 100u64), config_.multiplier);
        assert_eq!(Uint256::zero(), config_.lockdrop_incentives);

        // ***
        // *** Test :: Don't Update init_timestamp,min_lock_duration, max_lock_duration, weekly_multiplier (Reason :: env.block.time.seconds() >= config.init_timestamp)  ***
        // ***
        info = mock_info("new_owner");
        env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(1_000_000_05),
            ..Default::default()
        });
        update_config.init_timestamp = Some(1_000_000_39);
        update_config.min_duration = Some(3u64);
        update_config.max_duration = Some(9u64);
        update_config.weekly_multiplier = Some(Decimal256::from_ratio(17u64, 100u64));
        update_config.lockdrop_incentives = Some(Uint256::from(100000u64));
        update_config_msg = ExecuteMsg::UpdateConfig {
            new_config: update_config.clone(),
        };

        let mut update_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            update_config_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            update_s.attributes,
            vec![attr("action", "lockdrop::ExecuteMsg::UpdateConfig")]
        );

        config_ = query_config(deps.as_ref()).unwrap();
        assert_eq!(1_000_000_05, config_.init_timestamp);
        assert_eq!(1u64, config_.min_duration);
        assert_eq!(5u64, config_.max_duration);
        assert_eq!(Decimal256::from_ratio(9u64, 100u64), config_.multiplier);
        assert_eq!(Uint256::from(100000u64), config_.lockdrop_incentives);

        // ***
        // *** Test :: Update init_timestamp successfully ***
        // ***
        env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(1_000_000_01),
            ..Default::default()
        });
        update_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            update_config_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            update_s.attributes,
            vec![attr("action", "lockdrop::ExecuteMsg::UpdateConfig")]
        );

        config_ = query_config(deps.as_ref()).unwrap();
        assert_eq!(1_000_000_39, config_.init_timestamp);
        assert_eq!(3u64, config_.min_duration);
        assert_eq!(9u64, config_.max_duration);
        assert_eq!(Decimal256::from_ratio(17u64, 100u64), config_.multiplier);
        assert_eq!(Uint256::from(100000u64), config_.lockdrop_incentives);
    }

    #[test]
    fn test_deposit_ust() {
        let mut deps = th_setup(&[]);
        let deposit_amount = 110000u128;
        let mut info =
            cosmwasm_std::testing::mock_info("depositor", &[coin(deposit_amount, "uusd")]);
        deps.querier
            .set_incentives_address(Addr::unchecked("incentives".to_string()));
        deps.querier
            .set_unclaimed_rewards("cosmos2contract".to_string(), Uint128::from(0u64));
        // ***
        // *** Test :: Error "Deposit window closed" Reason :: Deposit attempt before deposit window is open ***
        // ***
        let mut env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(1_000_000_05),
            ..Default::default()
        });
        let mut deposit_msg = ExecuteMsg::DepositUst { duration: 3u64 };
        let mut deposit_f = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            deposit_msg.clone(),
        );
        assert_generic_error_message(deposit_f, "Deposit window closed");

        // ***
        // *** Test :: Error "Deposit window closed" Reason :: Deposit attempt after deposit window is closed ***
        // ***
        env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(1_010_000_01),
            ..Default::default()
        });
        deposit_f = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            deposit_msg.clone(),
        );
        assert_generic_error_message(deposit_f, "Deposit window closed");

        // ***
        // *** Test :: Error "Amount cannot be zero" ***
        // ***
        env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(1_000_000_15),
            ..Default::default()
        });
        info = cosmwasm_std::testing::mock_info("depositor", &[coin(0u128, "uusd")]);
        deposit_f = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            deposit_msg.clone(),
        );
        assert_generic_error_message(deposit_f, "Amount cannot be zero");

        // ***
        // *** Test :: Error "Lockup duration needs to be between {} and {}" Reason :: Selected lockup duration < min_duration ***
        // ***
        info = cosmwasm_std::testing::mock_info("depositor", &[coin(10000u128, "uusd")]);
        deposit_msg = ExecuteMsg::DepositUst { duration: 1u64 };
        deposit_f = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            deposit_msg.clone(),
        );
        assert_generic_error_message(deposit_f, "Lockup duration needs to be between 3 and 9");

        // ***
        // *** Test :: Error "Lockup duration needs to be between {} and {}" Reason :: Selected lockup duration > max_duration ***
        // ***
        deposit_msg = ExecuteMsg::DepositUst { duration: 21u64 };
        deposit_f = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            deposit_msg.clone(),
        );
        assert_generic_error_message(deposit_f, "Lockup duration needs to be between 3 and 9");

        // ***
        // *** Test #1 :: Successfully deposit UST  ***
        // ***
        deposit_msg = ExecuteMsg::DepositUst { duration: 3u64 };
        let mut deposit_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            deposit_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            deposit_s.attributes,
            vec![
                attr("action", "lockdrop::ExecuteMsg::LockUST"),
                attr("user", "depositor"),
                attr("duration", "3"),
                attr("ust_deposited", "10000")
            ]
        );
        // let's verify the Lockdrop
        let mut lockdrop_ =
            query_lockup_info_with_id(deps.as_ref(), "depositor3".to_string()).unwrap();
        assert_eq!(3u64, lockdrop_.duration);
        assert_eq!(Uint256::from(10000u64), lockdrop_.ust_locked);
        assert_eq!(Uint256::zero(), lockdrop_.maust_balance);
        assert_eq!(Uint256::from(21432423343u64), lockdrop_.lockdrop_reward);
        assert_eq!(101914410u64, lockdrop_.unlock_timestamp);
        // let's verify the User
        let mut user_ =
            query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
        assert_eq!(Uint256::from(10000u64), user_.total_ust_locked);
        assert_eq!(Uint256::zero(), user_.total_maust_locked);
        assert_eq!(vec!["depositor3".to_string()], user_.lockup_position_ids);
        assert_eq!(false, user_.is_lockdrop_claimed);
        assert_eq!(Decimal256::zero(), user_.reward_index);
        assert_eq!(Uint256::zero(), user_.pending_xmars);
        // let's verify the state
        let mut state_ = query_state(deps.as_ref()).unwrap();
        assert_eq!(Uint256::zero(), state_.final_ust_locked);
        assert_eq!(Uint256::zero(), state_.final_maust_locked);
        assert_eq!(Uint256::from(10000u64), state_.total_ust_locked);
        assert_eq!(Uint256::zero(), state_.total_maust_locked);
        assert_eq!(Uint256::from(2700u64), state_.total_deposits_weight);

        // ***
        // *** Test #2 :: Successfully deposit UST  ***
        // ***
        info = cosmwasm_std::testing::mock_info("depositor", &[coin(100u128, "uusd")]);
        deposit_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            deposit_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            deposit_s.attributes,
            vec![
                attr("action", "lockdrop::ExecuteMsg::LockUST"),
                attr("user", "depositor"),
                attr("duration", "3"),
                attr("ust_deposited", "100")
            ]
        );
        // let's verify the Lockdrop
        lockdrop_ = query_lockup_info_with_id(deps.as_ref(), "depositor3".to_string()).unwrap();
        assert_eq!(3u64, lockdrop_.duration);
        assert_eq!(Uint256::from(10100u64), lockdrop_.ust_locked);
        assert_eq!(101914410u64, lockdrop_.unlock_timestamp);
        // let's verify the User
        user_ = query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
        assert_eq!(Uint256::from(10100u64), user_.total_ust_locked);
        assert_eq!(vec!["depositor3".to_string()], user_.lockup_position_ids);
        // let's verify the state
        state_ = query_state(deps.as_ref()).unwrap();
        assert_eq!(Uint256::from(10100u64), state_.total_ust_locked);
        assert_eq!(Uint256::from(2727u64), state_.total_deposits_weight);

        // ***
        // *** Test #3 :: Successfully deposit UST (new lockup)  ***
        // ***
        deposit_msg = ExecuteMsg::DepositUst { duration: 5u64 };
        info = cosmwasm_std::testing::mock_info("depositor", &[coin(5432u128, "uusd")]);
        deposit_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            deposit_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            deposit_s.attributes,
            vec![
                attr("action", "lockdrop::ExecuteMsg::LockUST"),
                attr("user", "depositor"),
                attr("duration", "5"),
                attr("ust_deposited", "5432")
            ]
        );
        // let's verify the Lockdrop
        lockdrop_ = query_lockup_info_with_id(deps.as_ref(), "depositor5".to_string()).unwrap();
        assert_eq!(5u64, lockdrop_.duration);
        assert_eq!(Uint256::from(5432u64), lockdrop_.ust_locked);
        assert_eq!(103124010u64, lockdrop_.unlock_timestamp);
        // let's verify the User
        user_ = query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
        assert_eq!(Uint256::from(15532u64), user_.total_ust_locked);
        assert_eq!(
            vec!["depositor3".to_string(), "depositor5".to_string()],
            user_.lockup_position_ids
        );
        // let's verify the state
        state_ = query_state(deps.as_ref()).unwrap();
        assert_eq!(Uint256::from(15532u64), state_.total_ust_locked);
        assert_eq!(Uint256::from(5171u64), state_.total_deposits_weight);
    }

    #[test]
    fn test_withdraw_ust() {
        let mut deps = th_setup(&[]);
        let deposit_amount = 1000000u128;
        let info = cosmwasm_std::testing::mock_info("depositor", &[coin(deposit_amount, "uusd")]);
        deps.querier
            .set_incentives_address(Addr::unchecked("incentives".to_string()));
        deps.querier
            .set_unclaimed_rewards("cosmos2contract".to_string(), Uint128::from(0u64));
        // Set tax data
        deps.querier.set_native_tax(
            Decimal::from_ratio(1u128, 100u128),
            &[(String::from("uusd"), Uint128::new(100u128))],
        );

        // ***** Setup *****

        let mut env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(1_000_000_15),
            ..Default::default()
        });
        // Create a lockdrop position for testing
        let mut deposit_msg = ExecuteMsg::DepositUst { duration: 3u64 };
        let mut deposit_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            deposit_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            deposit_s.attributes,
            vec![
                attr("action", "lockdrop::ExecuteMsg::LockUST"),
                attr("user", "depositor"),
                attr("duration", "3"),
                attr("ust_deposited", "1000000")
            ]
        );
        deposit_msg = ExecuteMsg::DepositUst { duration: 5u64 };
        deposit_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            deposit_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            deposit_s.attributes,
            vec![
                attr("action", "lockdrop::ExecuteMsg::LockUST"),
                attr("user", "depositor"),
                attr("duration", "5"),
                attr("ust_deposited", "1000000")
            ]
        );

        // ***
        // *** Test :: Error "Withdrawals not allowed" Reason :: Withdrawal attempt after the window is closed ***
        // ***
        env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(10_00_720_11),
            ..Default::default()
        });
        let mut withdrawal_msg = ExecuteMsg::WithdrawUst {
            amount: Uint256::from(100u64),
            duration: 5u64,
        };
        let mut withdrawal_f = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            withdrawal_msg.clone(),
        );
        assert_generic_error_message(withdrawal_f, "Withdrawals not allowed");

        // ***
        // *** Test :: Error "Lockup doesn't exist" Reason :: Invalid lockup ***
        // ***
        env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(10_00_120_10),
            ..Default::default()
        });
        withdrawal_msg = ExecuteMsg::WithdrawUst {
            amount: Uint256::from(100u64),
            duration: 4u64,
        };
        withdrawal_f = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            withdrawal_msg.clone(),
        );
        assert_generic_error_message(withdrawal_f, "Lockup doesn't exist");

        // ***
        // *** Test :: Error "Invalid withdrawal request" Reason :: Invalid amount ***
        // ***
        withdrawal_msg = ExecuteMsg::WithdrawUst {
            amount: Uint256::from(100000000u64),
            duration: 5u64,
        };
        withdrawal_f = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            withdrawal_msg.clone(),
        );
        assert_generic_error_message(withdrawal_f, "Invalid withdrawal request");

        withdrawal_msg = ExecuteMsg::WithdrawUst {
            amount: Uint256::from(0u64),
            duration: 5u64,
        };
        withdrawal_f = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            withdrawal_msg.clone(),
        );
        assert_generic_error_message(withdrawal_f, "Invalid withdrawal request");

        // ***
        // *** Test #1 :: Successfully withdraw UST  ***
        // ***
        withdrawal_msg = ExecuteMsg::WithdrawUst {
            amount: Uint256::from(42u64),
            duration: 5u64,
        };
        let mut withdrawal_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            withdrawal_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            withdrawal_s.attributes,
            vec![
                attr("action", "lockdrop::ExecuteMsg::WithdrawUST"),
                attr("user", "depositor"),
                attr("duration", "5"),
                attr("ust_withdrawn", "42")
            ]
        );
        // let's verify the Lockdrop
        let mut lockdrop_ =
            query_lockup_info_with_id(deps.as_ref(), "depositor5".to_string()).unwrap();
        assert_eq!(5u64, lockdrop_.duration);
        assert_eq!(Uint256::from(999958u64), lockdrop_.ust_locked);
        assert_eq!(103124010u64, lockdrop_.unlock_timestamp);
        // let's verify the User
        let mut user_ =
            query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
        assert_eq!(Uint256::from(1999958u64), user_.total_ust_locked);
        assert_eq!(
            vec!["depositor3".to_string(), "depositor5".to_string()],
            user_.lockup_position_ids
        );
        // let's verify the state
        let mut state_ = query_state(deps.as_ref()).unwrap();
        assert_eq!(Uint256::from(1999958u64), state_.total_ust_locked);
        assert_eq!(Uint256::from(719982u64), state_.total_deposits_weight);

        // ***
        // *** Test #2 :: Successfully withdraw UST  ***
        // ***
        withdrawal_msg = ExecuteMsg::WithdrawUst {
            amount: Uint256::from(999958u64),
            duration: 5u64,
        };
        withdrawal_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            withdrawal_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            withdrawal_s.attributes,
            vec![
                attr("action", "lockdrop::ExecuteMsg::WithdrawUST"),
                attr("user", "depositor"),
                attr("duration", "5"),
                attr("ust_withdrawn", "999958")
            ]
        );
        // let's verify the Lockdrop
        lockdrop_ = query_lockup_info_with_id(deps.as_ref(), "depositor5".to_string()).unwrap();
        assert_eq!(5u64, lockdrop_.duration);
        assert_eq!(Uint256::from(0u64), lockdrop_.ust_locked);
        assert_eq!(103124010u64, lockdrop_.unlock_timestamp);
        // let's verify the User
        user_ = query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
        assert_eq!(Uint256::from(1000000u64), user_.total_ust_locked);
        assert_eq!(vec!["depositor3".to_string()], user_.lockup_position_ids);
        // let's verify the state
        state_ = query_state(deps.as_ref()).unwrap();
        assert_eq!(Uint256::from(1000000u64), state_.total_ust_locked);
        assert_eq!(Uint256::from(270001u64), state_.total_deposits_weight);

        // ***
        // *** Test #3 :: Successfully withdraw UST  ***
        // ***
        withdrawal_msg = ExecuteMsg::WithdrawUst {
            amount: Uint256::from(1000u64),
            duration: 3u64,
        };
        withdrawal_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            withdrawal_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            withdrawal_s.attributes,
            vec![
                attr("action", "lockdrop::ExecuteMsg::WithdrawUST"),
                attr("user", "depositor"),
                attr("duration", "3"),
                attr("ust_withdrawn", "1000")
            ]
        );
        // let's verify the Lockdrop
        lockdrop_ = query_lockup_info_with_id(deps.as_ref(), "depositor3".to_string()).unwrap();
        assert_eq!(3u64, lockdrop_.duration);
        assert_eq!(Uint256::from(999000u64), lockdrop_.ust_locked);
        assert_eq!(101914410u64, lockdrop_.unlock_timestamp);
        // let's verify the User
        user_ = query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
        assert_eq!(Uint256::from(999000u64), user_.total_ust_locked);
        assert_eq!(vec!["depositor3".to_string()], user_.lockup_position_ids);
        // let's verify the state
        state_ = query_state(deps.as_ref()).unwrap();
        assert_eq!(Uint256::from(999000u64), state_.total_ust_locked);
        assert_eq!(Uint256::from(269731u64), state_.total_deposits_weight);
    }

    #[test]
    fn test_deposit_ust_in_red_bank() {
        let mut deps = th_setup(&[]);
        let deposit_amount = 1000000u128;
        let mut info =
            cosmwasm_std::testing::mock_info("depositor", &[coin(deposit_amount, "uusd")]);
        // Set tax data
        deps.querier.set_native_tax(
            Decimal::from_ratio(1u128, 100u128),
            &[(String::from("uusd"), Uint128::new(100u128))],
        );

        // ***** Setup *****

        let mut env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(1_000_000_15),
            ..Default::default()
        });
        // Create a lockdrop position for testing
        let mut deposit_msg = ExecuteMsg::DepositUst { duration: 3u64 };
        let mut deposit_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            deposit_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            deposit_s.attributes,
            vec![
                attr("action", "lockdrop::ExecuteMsg::LockUST"),
                attr("user", "depositor"),
                attr("duration", "3"),
                attr("ust_deposited", "1000000")
            ]
        );
        deposit_msg = ExecuteMsg::DepositUst { duration: 5u64 };
        deposit_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            deposit_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            deposit_s.attributes,
            vec![
                attr("action", "lockdrop::ExecuteMsg::LockUST"),
                attr("user", "depositor"),
                attr("duration", "5"),
                attr("ust_deposited", "1000000")
            ]
        );

        // ***
        // *** Test :: Error "Unauthorized" ***
        // ***
        let deposit_in_redbank_msg = ExecuteMsg::DepositUstInRedBank {};
        let deposit_in_redbank_response_f = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            deposit_in_redbank_msg.clone(),
        );
        assert_generic_error_message(deposit_in_redbank_response_f, "Unauthorized");

        // ***
        // *** Test :: Error "Lockdrop deposits haven't concluded yet" ***
        // ***
        info = mock_info("owner");
        let mut deposit_in_redbank_response_f = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            deposit_in_redbank_msg.clone(),
        );
        assert_generic_error_message(
            deposit_in_redbank_response_f,
            "Lockdrop deposits haven't concluded yet",
        );

        env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(1_000_000_09),
            ..Default::default()
        });
        deposit_in_redbank_response_f = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            deposit_in_redbank_msg.clone(),
        );
        assert_generic_error_message(
            deposit_in_redbank_response_f,
            "Lockdrop deposits haven't concluded yet",
        );

        env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(1_001_000_09),
            ..Default::default()
        });
        deposit_in_redbank_response_f = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            deposit_in_redbank_msg.clone(),
        );
        assert_generic_error_message(
            deposit_in_redbank_response_f,
            "Lockdrop deposits haven't concluded yet",
        );

        // ***
        // *** Successfully deposited ***
        // ***
        env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(1_001_000_11),
            ..Default::default()
        });
        deps.querier.set_cw20_balances(
            Addr::unchecked("ma_ust_token".to_string()),
            &[(Addr::unchecked(MOCK_CONTRACT_ADDR), Uint128::new(0u128))],
        );
        let deposit_in_redbank_response_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            deposit_in_redbank_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            deposit_in_redbank_response_s.attributes,
            vec![
                attr("action", "lockdrop::ExecuteMsg::DepositInRedBank"),
                attr("ust_deposited_in_red_bank", "2000000"),
                attr("timestamp", "100100011")
            ]
        );
        assert_eq!(
            deposit_in_redbank_response_s.messages,
            vec![
                SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "red_bank".to_string(),
                    msg: to_binary(&mars::red_bank::msg::ExecuteMsg::DepositNative {
                        denom: "uusd".to_string(),
                    })
                    .unwrap(),
                    funds: vec![Coin {
                        denom: "uusd".to_string(),
                        amount: Uint128::from(1999900u128),
                    }]
                })),
                SubMsg::new(
                    CallbackMsg::UpdateStateOnRedBankDeposit {
                        prev_ma_ust_balance: Uint256::from(0u64)
                    }
                    .to_cosmos_msg(&env.clone().contract.address)
                    .unwrap()
                ),
            ]
        );
        // let's verify the state
        let state_ = query_state(deps.as_ref()).unwrap();
        assert_eq!(Uint256::zero(), state_.final_ust_locked);
        assert_eq!(Uint256::zero(), state_.final_maust_locked);
        assert_eq!(Uint256::from(2000000u64), state_.total_ust_locked);
        assert_eq!(Uint256::zero(), state_.total_maust_locked);
        assert_eq!(Decimal256::zero(), state_.global_reward_index);
        assert_eq!(Uint256::from(720000u64), state_.total_deposits_weight);
    }

    #[test]
    fn test_update_state_on_red_bank_deposit_callback() {
        let mut deps = th_setup(&[]);
        let deposit_amount = 1000000u128;
        let mut info =
            cosmwasm_std::testing::mock_info("depositor", &[coin(deposit_amount, "uusd")]);
        deps.querier
            .set_incentives_address(Addr::unchecked("incentives".to_string()));
        deps.querier
            .set_unclaimed_rewards("cosmos2contract".to_string(), Uint128::from(0u64));
        // Set tax data
        deps.querier.set_native_tax(
            Decimal::from_ratio(1u128, 100u128),
            &[(String::from("uusd"), Uint128::new(100u128))],
        );
        deps.querier.set_cw20_balances(
            Addr::unchecked("ma_ust_token".to_string()),
            &[(
                Addr::unchecked(MOCK_CONTRACT_ADDR),
                Uint128::new(197000u128),
            )],
        );

        // ***** Setup *****

        let env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(1_000_000_15),
            ..Default::default()
        });
        // Create a lockdrop position for testing
        let mut deposit_msg = ExecuteMsg::DepositUst { duration: 3u64 };
        let mut deposit_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            deposit_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            deposit_s.attributes,
            vec![
                attr("action", "lockdrop::ExecuteMsg::LockUST"),
                attr("user", "depositor"),
                attr("duration", "3"),
                attr("ust_deposited", "1000000")
            ]
        );
        deposit_msg = ExecuteMsg::DepositUst { duration: 5u64 };
        deposit_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            deposit_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            deposit_s.attributes,
            vec![
                attr("action", "lockdrop::ExecuteMsg::LockUST"),
                attr("user", "depositor"),
                attr("duration", "5"),
                attr("ust_deposited", "1000000")
            ]
        );

        // ***
        // *** Successfully updates the state post deposit in Red Bank ***
        // ***
        info = mock_info(&env.clone().contract.address.to_string());
        let callback_msg = ExecuteMsg::Callback(CallbackMsg::UpdateStateOnRedBankDeposit {
            prev_ma_ust_balance: Uint256::from(100u64),
        });
        let redbank_callback_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            callback_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            redbank_callback_s.attributes,
            vec![
                attr("action", "lockdrop::CallbackMsg::RedBankDeposit"),
                attr("maUST_minted", "196900")
            ]
        );

        // let's verify the state
        let state_ = query_state(deps.as_ref()).unwrap();
        // final : tracks Total UST deposited / Total MA-UST Minted
        assert_eq!(Uint256::from(2000000u64), state_.final_ust_locked);
        assert_eq!(Uint256::from(196900u64), state_.final_maust_locked);
        // Total : tracks UST / MA-UST Available with the lockdrop contract
        assert_eq!(Uint256::zero(), state_.total_ust_locked);
        assert_eq!(Uint256::from(196900u64), state_.total_maust_locked);
        // global_reward_index, total_deposits_weight :: Used for lockdrop / X-Mars distribution
        assert_eq!(Decimal256::zero(), state_.global_reward_index);
        assert_eq!(Uint256::from(720000u64), state_.total_deposits_weight);

        // let's verify the User
        let user_ = query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
        assert_eq!(Uint256::from(2000000u64), user_.total_ust_locked);
        assert_eq!(Uint256::from(196900u64), user_.total_maust_locked);
        assert_eq!(false, user_.is_lockdrop_claimed);
        assert_eq!(Decimal256::zero(), user_.reward_index);
        assert_eq!(Uint256::zero(), user_.pending_xmars);
        assert_eq!(
            vec!["depositor3".to_string(), "depositor5".to_string()],
            user_.lockup_position_ids
        );

        // let's verify the lockup #1
        let mut lockdrop_ =
            query_lockup_info_with_id(deps.as_ref(), "depositor3".to_string()).unwrap();
        assert_eq!(3u64, lockdrop_.duration);
        assert_eq!(Uint256::from(1000000u64), lockdrop_.ust_locked);
        assert_eq!(Uint256::from(98450u64), lockdrop_.maust_balance);
        assert_eq!(Uint256::from(8037158753u64), lockdrop_.lockdrop_reward);
        assert_eq!(101914410u64, lockdrop_.unlock_timestamp);

        // let's verify the lockup #2
        lockdrop_ = query_lockup_info_with_id(deps.as_ref(), "depositor5".to_string()).unwrap();
        assert_eq!(5u64, lockdrop_.duration);
        assert_eq!(Uint256::from(1000000u64), lockdrop_.ust_locked);
        assert_eq!(Uint256::from(98450u64), lockdrop_.maust_balance);
        assert_eq!(Uint256::from(13395264589u64), lockdrop_.lockdrop_reward);
        assert_eq!(103124010u64, lockdrop_.unlock_timestamp);
    }

    #[test]
    fn test_try_claim() {
        let mut deps = th_setup(&[]);
        let deposit_amount = 1000000u128;
        let mut info =
            cosmwasm_std::testing::mock_info("depositor", &[coin(deposit_amount, "uusd")]);
        // Set tax data
        deps.querier.set_native_tax(
            Decimal::from_ratio(1u128, 100u128),
            &[(String::from("uusd"), Uint128::new(100u128))],
        );
        deps.querier
            .set_incentives_address(Addr::unchecked("incentives".to_string()));

        // ***** Setup *****

        let mut env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(1_000_000_15),
            ..Default::default()
        });
        // Create a lockdrop position for testing
        let mut deposit_msg = ExecuteMsg::DepositUst { duration: 3u64 };
        let mut deposit_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            deposit_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            deposit_s.attributes,
            vec![
                attr("action", "lockdrop::ExecuteMsg::LockUST"),
                attr("user", "depositor"),
                attr("duration", "3"),
                attr("ust_deposited", "1000000")
            ]
        );
        deposit_msg = ExecuteMsg::DepositUst { duration: 5u64 };
        deposit_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            deposit_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            deposit_s.attributes,
            vec![
                attr("action", "lockdrop::ExecuteMsg::LockUST"),
                attr("user", "depositor"),
                attr("duration", "5"),
                attr("ust_deposited", "1000000")
            ]
        );

        // ***
        // *** Test :: Error "Claim not allowed" ***
        // ***
        env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(1_001_000_09),
            ..Default::default()
        });
        let claim_rewards_msg = ExecuteMsg::ClaimRewards {};
        let mut claim_rewards_response_f = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            claim_rewards_msg.clone(),
        );
        assert_generic_error_message(claim_rewards_response_f, "Claim not allowed");

        env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(1_001_000_09),
            ..Default::default()
        });
        claim_rewards_response_f = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            claim_rewards_msg.clone(),
        );
        assert_generic_error_message(claim_rewards_response_f, "Claim not allowed");

        // ***
        // *** Test :: Error "No lockup to claim rewards for" ***
        // ***
        env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(1_001_001_09),
            ..Default::default()
        });
        info = mock_info("not_depositor");
        claim_rewards_response_f = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            claim_rewards_msg.clone(),
        );
        assert_generic_error_message(claim_rewards_response_f, "No lockup to claim rewards for");

        // ***
        // *** Test #1 :: Successfully Claim Rewards ***
        // ***
        deps.querier
            .set_unclaimed_rewards("cosmos2contract".to_string(), Uint128::from(100u64));
        deps.querier.set_cw20_balances(
            Addr::unchecked("xmars_token".to_string()),
            &[(Addr::unchecked(MOCK_CONTRACT_ADDR), Uint128::new(0u128))],
        );
        info = mock_info("depositor");
        let mut claim_rewards_response_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            claim_rewards_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            claim_rewards_response_s.attributes,
            vec![
                attr("action", "lockdrop::ExecuteMsg::ClaimRewards"),
                attr("unclaimed_xMars", "100")
            ]
        );
        assert_eq!(
            claim_rewards_response_s.messages,
            vec![
                SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "incentives".to_string(),
                    msg: to_binary(&mars::incentives::msg::ExecuteMsg::ClaimRewards {}).unwrap(),
                    funds: vec![]
                })),
                SubMsg::new(
                    CallbackMsg::UpdateStateOnClaim {
                        user: Addr::unchecked("depositor".to_string()),
                        prev_xmars_balance: Uint256::from(0u64)
                    }
                    .to_cosmos_msg(&env.clone().contract.address)
                    .unwrap()
                ),
            ]
        );

        // ***
        // *** Test #2 :: Successfully Claim Rewards (doesn't claim XMars as no rewards to claim) ***
        // ***
        deps.querier
            .set_unclaimed_rewards("cosmos2contract".to_string(), Uint128::from(0u64));
        deps.querier.set_cw20_balances(
            Addr::unchecked("xmars_token".to_string()),
            &[(Addr::unchecked(MOCK_CONTRACT_ADDR), Uint128::new(58460u128))],
        );
        claim_rewards_response_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            claim_rewards_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            claim_rewards_response_s.attributes,
            vec![
                attr("action", "lockdrop::ExecuteMsg::ClaimRewards"),
                attr("unclaimed_xMars", "0")
            ]
        );
        assert_eq!(
            claim_rewards_response_s.messages,
            vec![SubMsg::new(
                CallbackMsg::UpdateStateOnClaim {
                    user: Addr::unchecked("depositor".to_string()),
                    prev_xmars_balance: Uint256::from(58460u64)
                }
                .to_cosmos_msg(&env.clone().contract.address)
                .unwrap()
            ),]
        );
    }

    #[test]
    fn test_update_state_on_claim() {
        let mut deps = th_setup(&[]);
        let deposit_amount = 1000000u128;
        let mut info =
            cosmwasm_std::testing::mock_info("depositor", &[coin(deposit_amount, "uusd")]);
        deps.querier
            .set_unclaimed_rewards("cosmos2contract".to_string(), Uint128::from(0u64));
        deps.querier
            .set_incentives_address(Addr::unchecked("incentives".to_string()));
        // Set tax data
        deps.querier.set_native_tax(
            Decimal::from_ratio(1u128, 100u128),
            &[(String::from("uusd"), Uint128::new(100u128))],
        );
        deps.querier
            .set_incentives_address(Addr::unchecked("incentives".to_string()));

        // ***** Setup *****

        let env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(1_000_000_15),
            ..Default::default()
        });
        // Create some lockdrop positions for testing
        let mut deposit_msg = ExecuteMsg::DepositUst { duration: 3u64 };
        let mut deposit_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            deposit_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            deposit_s.attributes,
            vec![
                attr("action", "lockdrop::ExecuteMsg::LockUST"),
                attr("user", "depositor"),
                attr("duration", "3"),
                attr("ust_deposited", "1000000")
            ]
        );
        deposit_msg = ExecuteMsg::DepositUst { duration: 5u64 };
        deposit_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            deposit_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            deposit_s.attributes,
            vec![
                attr("action", "lockdrop::ExecuteMsg::LockUST"),
                attr("user", "depositor"),
                attr("duration", "5"),
                attr("ust_deposited", "1000000")
            ]
        );

        info = cosmwasm_std::testing::mock_info("depositor2", &[coin(6450000u128, "uusd")]);
        deposit_msg = ExecuteMsg::DepositUst { duration: 3u64 };
        deposit_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            deposit_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            deposit_s.attributes,
            vec![
                attr("action", "lockdrop::ExecuteMsg::LockUST"),
                attr("user", "depositor2"),
                attr("duration", "3"),
                attr("ust_deposited", "6450000")
            ]
        );
        deposit_msg = ExecuteMsg::DepositUst { duration: 5u64 };
        deposit_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            deposit_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            deposit_s.attributes,
            vec![
                attr("action", "lockdrop::ExecuteMsg::LockUST"),
                attr("user", "depositor2"),
                attr("duration", "5"),
                attr("ust_deposited", "6450000")
            ]
        );

        // *** Successfully updates the state post deposit in Red Bank ***
        deps.querier.set_cw20_balances(
            Addr::unchecked("ma_ust_token".to_string()),
            &[(
                Addr::unchecked(MOCK_CONTRACT_ADDR),
                Uint128::new(197000u128),
            )],
        );
        info = mock_info(&env.clone().contract.address.to_string());
        let callback_msg = ExecuteMsg::Callback(CallbackMsg::UpdateStateOnRedBankDeposit {
            prev_ma_ust_balance: Uint256::from(0u64),
        });
        let redbank_callback_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            callback_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            redbank_callback_s.attributes,
            vec![
                attr("action", "lockdrop::CallbackMsg::RedBankDeposit"),
                attr("maUST_minted", "197000")
            ]
        );

        // let's verify the state
        let mut state_ = query_state(deps.as_ref()).unwrap();
        // final : tracks Total UST deposited / Total MA-UST Minted
        assert_eq!(Uint256::from(14900000u64), state_.final_ust_locked);
        assert_eq!(Uint256::from(197000u64), state_.final_maust_locked);
        // Total : tracks UST / MA-UST Available with the lockdrop contract
        assert_eq!(Uint256::zero(), state_.total_ust_locked);
        assert_eq!(Uint256::from(197000u64), state_.total_maust_locked);
        // global_reward_index, total_deposits_weight :: Used for lockdrop / X-Mars distribution
        assert_eq!(Decimal256::zero(), state_.global_reward_index);
        assert_eq!(Uint256::from(5364000u64), state_.total_deposits_weight);

        // ***
        // *** Test #1 :: Successfully updates state on Reward claim (Claims both MARS and XMARS) ***
        // ***

        deps.querier.set_cw20_balances(
            Addr::unchecked("xmars_token".to_string()),
            &[(Addr::unchecked(MOCK_CONTRACT_ADDR), Uint128::new(58460u128))],
        );
        deps.querier.set_cw20_balances(
            Addr::unchecked("mars_token".to_string()),
            &[(
                Addr::unchecked(MOCK_CONTRACT_ADDR),
                Uint128::new(54568460u128),
            )],
        );

        info = mock_info(&env.clone().contract.address.to_string());
        let mut callback_msg = ExecuteMsg::Callback(CallbackMsg::UpdateStateOnClaim {
            user: Addr::unchecked("depositor".to_string()),
            prev_xmars_balance: Uint256::from(100u64),
        });
        let mut redbank_callback_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            callback_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            redbank_callback_s.attributes,
            vec![
                attr("action", "lockdrop::CallbackMsg::ClaimRewards"),
                attr("total_xmars_claimed", "58360"),
                attr("user", "depositor"),
                attr("mars_claimed", "2876835347"),
                attr("xmars_claimed", "7833")
            ]
        );
        assert_eq!(
            redbank_callback_s.messages,
            vec![
                SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "mars_token".to_string(),
                    msg: to_binary(&cw20::Cw20ExecuteMsg::Transfer {
                        recipient: "depositor".to_string(),
                        amount: Uint128::from(2876835347u128),
                    })
                    .unwrap(),
                    funds: vec![]
                })),
                SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "xmars_token".to_string(),
                    msg: to_binary(&cw20::Cw20ExecuteMsg::Transfer {
                        recipient: "depositor".to_string(),
                        amount: Uint128::from(7833u128),
                    })
                    .unwrap(),
                    funds: vec![]
                })),
            ]
        );
        // let's verify the state
        state_ = query_state(deps.as_ref()).unwrap();
        assert_eq!(Uint256::zero(), state_.total_ust_locked);
        assert_eq!(
            Decimal256::from_ratio(58360u64, 197000u64),
            state_.global_reward_index
        );
        // let's verify the User
        let mut user_ =
            query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
        assert_eq!(Uint256::from(2000000u64), user_.total_ust_locked);
        assert_eq!(Uint256::from(26442u64), user_.total_maust_locked);
        assert_eq!(true, user_.is_lockdrop_claimed);
        assert_eq!(
            Decimal256::from_ratio(58360u64, 197000u64),
            user_.reward_index
        );
        assert_eq!(Uint256::zero(), user_.pending_xmars);
        assert_eq!(
            vec!["depositor3".to_string(), "depositor5".to_string()],
            user_.lockup_position_ids
        );
        // // let's verify user's lockup #1
        let mut lockdrop_ =
            query_lockup_info_with_id(deps.as_ref(), "depositor3".to_string()).unwrap();
        assert_eq!(Uint256::from(1000000u64), lockdrop_.ust_locked);
        assert_eq!(Uint256::from(13221u64), lockdrop_.maust_balance);
        assert_eq!(Uint256::from(1078813255u64), lockdrop_.lockdrop_reward);
        assert_eq!(101914410u64, lockdrop_.unlock_timestamp);
        // // let's verify user's lockup #1
        lockdrop_ = query_lockup_info_with_id(deps.as_ref(), "depositor5".to_string()).unwrap();
        assert_eq!(Uint256::from(1000000u64), lockdrop_.ust_locked);
        assert_eq!(Uint256::from(13221u64), lockdrop_.maust_balance);
        assert_eq!(Uint256::from(1798022092u64), lockdrop_.lockdrop_reward);
        assert_eq!(103124010u64, lockdrop_.unlock_timestamp);

        // ***
        // *** Test #2 :: Successfully updates state on Reward claim (Claims only XMARS) ***
        // ***
        deps.querier.set_cw20_balances(
            Addr::unchecked("xmars_token".to_string()),
            &[(
                Addr::unchecked(MOCK_CONTRACT_ADDR),
                Uint128::new(43534460u128),
            )],
        );
        callback_msg = ExecuteMsg::Callback(CallbackMsg::UpdateStateOnClaim {
            user: Addr::unchecked("depositor".to_string()),
            prev_xmars_balance: Uint256::from(56430u64),
        });
        redbank_callback_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            callback_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            redbank_callback_s.attributes,
            vec![
                attr("action", "lockdrop::CallbackMsg::ClaimRewards"),
                attr("total_xmars_claimed", "43478030"),
                attr("user", "depositor"),
                attr("mars_claimed", "0"),
                attr("xmars_claimed", "5835767")
            ]
        );
        assert_eq!(
            redbank_callback_s.messages,
            vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "xmars_token".to_string(),
                msg: to_binary(&cw20::Cw20ExecuteMsg::Transfer {
                    recipient: "depositor".to_string(),
                    amount: Uint128::from(5835767u128),
                })
                .unwrap(),
                funds: vec![]
            })),]
        );
        // let's verify the User
        user_ = query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
        assert_eq!(true, user_.is_lockdrop_claimed);
        assert_eq!(Uint256::zero(), user_.pending_xmars);

        // ***
        // *** Test #3 :: Successfully updates state on Reward claim (Claims MARS and XMARS for 2nd depositor) ***
        // ***
        callback_msg = ExecuteMsg::Callback(CallbackMsg::UpdateStateOnClaim {
            user: Addr::unchecked("depositor2".to_string()),
            prev_xmars_balance: Uint256::from(0u64),
        });
        redbank_callback_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            callback_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            redbank_callback_s.attributes,
            vec![
                attr("action", "lockdrop::CallbackMsg::ClaimRewards"),
                attr("total_xmars_claimed", "43534460"),
                attr("user", "depositor2"),
                attr("mars_claimed", "18555587994"),
                attr("xmars_claimed", "75383466")
            ]
        );
        assert_eq!(
            redbank_callback_s.messages,
            vec![
                SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "mars_token".to_string(),
                    msg: to_binary(&cw20::Cw20ExecuteMsg::Transfer {
                        recipient: "depositor2".to_string(),
                        amount: Uint128::from(18555587994u128),
                    })
                    .unwrap(),
                    funds: vec![]
                })),
                SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "xmars_token".to_string(),
                    msg: to_binary(&cw20::Cw20ExecuteMsg::Transfer {
                        recipient: "depositor2".to_string(),
                        amount: Uint128::from(75383466u128),
                    })
                    .unwrap(),
                    funds: vec![]
                })),
            ]
        );
        // let's verify the User
        user_ = query_user_info(deps.as_ref(), env.clone(), "depositor2".to_string()).unwrap();
        assert_eq!(Uint256::from(12900000u64), user_.total_ust_locked);
        assert_eq!(Uint256::from(170557u64), user_.total_maust_locked);
        assert_eq!(true, user_.is_lockdrop_claimed);
        assert_eq!(Uint256::zero(), user_.pending_xmars);
        assert_eq!(
            vec!["depositor23".to_string(), "depositor25".to_string()],
            user_.lockup_position_ids
        );
        // // let's verify user's lockup #1
        lockdrop_ = query_lockup_info_with_id(deps.as_ref(), "depositor23".to_string()).unwrap();
        assert_eq!(Uint256::from(6450000u64), lockdrop_.ust_locked);
        assert_eq!(Uint256::from(85278u64), lockdrop_.maust_balance);
        assert_eq!(Uint256::from(6958345498u64), lockdrop_.lockdrop_reward);
        assert_eq!(101914410u64, lockdrop_.unlock_timestamp);
        // // let's verify user's lockup #1
        lockdrop_ = query_lockup_info_with_id(deps.as_ref(), "depositor25".to_string()).unwrap();
        assert_eq!(Uint256::from(6450000u64), lockdrop_.ust_locked);
        assert_eq!(Uint256::from(85278u64), lockdrop_.maust_balance);
        assert_eq!(Uint256::from(11597242496u64), lockdrop_.lockdrop_reward);
        assert_eq!(103124010u64, lockdrop_.unlock_timestamp);
    }

    #[test]
    fn test_try_unlock_position() {
        let mut deps = th_setup(&[]);
        let deposit_amount = 1000000u128;
        let mut info =
            cosmwasm_std::testing::mock_info("depositor", &[coin(deposit_amount, "uusd")]);
        // Set tax data
        deps.querier.set_native_tax(
            Decimal::from_ratio(1u128, 100u128),
            &[(String::from("uusd"), Uint128::new(100u128))],
        );
        deps.querier
            .set_incentives_address(Addr::unchecked("incentives".to_string()));

        // ***** Setup *****

        let mut env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(1_000_000_15),
            ..Default::default()
        });

        // Create a lockdrop position for testing
        let mut deposit_msg = ExecuteMsg::DepositUst { duration: 3u64 };
        let mut deposit_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            deposit_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            deposit_s.attributes,
            vec![
                attr("action", "lockdrop::ExecuteMsg::LockUST"),
                attr("user", "depositor"),
                attr("duration", "3"),
                attr("ust_deposited", "1000000")
            ]
        );
        deposit_msg = ExecuteMsg::DepositUst { duration: 5u64 };
        deposit_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            deposit_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            deposit_s.attributes,
            vec![
                attr("action", "lockdrop::ExecuteMsg::LockUST"),
                attr("user", "depositor"),
                attr("duration", "5"),
                attr("ust_deposited", "1000000")
            ]
        );

        // *** Successfully updates the state post deposit in Red Bank ***
        deps.querier.set_cw20_balances(
            Addr::unchecked("ma_ust_token".to_string()),
            &[(
                Addr::unchecked(MOCK_CONTRACT_ADDR),
                Uint128::new(19700000u128),
            )],
        );
        info = mock_info(&env.clone().contract.address.to_string());
        let callback_msg = ExecuteMsg::Callback(CallbackMsg::UpdateStateOnRedBankDeposit {
            prev_ma_ust_balance: Uint256::from(0u64),
        });
        execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            callback_msg.clone(),
        )
        .unwrap();

        // ***
        // *** Test :: Error "Invalid lockup" ***
        // ***
        let mut unlock_msg = ExecuteMsg::Unlock { duration: 4u64 };
        let mut unlock_f = execute(deps.as_mut(), env.clone(), info.clone(), unlock_msg.clone());
        assert_generic_error_message(unlock_f, "Invalid lockup");

        // ***
        // *** Test :: Error "{} seconds to Unlock" ***
        // ***
        info = mock_info("depositor");
        env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(1_000_040_95),
            ..Default::default()
        });
        unlock_msg = ExecuteMsg::Unlock { duration: 3u64 };
        unlock_f = execute(deps.as_mut(), env.clone(), info.clone(), unlock_msg.clone());
        assert_generic_error_message(unlock_f, "1910315 seconds to Unlock");

        // ***
        // *** Test :: Should unlock successfully ***
        // ***
        deps.querier
            .set_incentives_address(Addr::unchecked("incentives".to_string()));
        deps.querier
            .set_unclaimed_rewards("cosmos2contract".to_string(), Uint128::from(8706700u64));
        deps.querier.set_cw20_balances(
            Addr::unchecked("xmars_token".to_string()),
            &[(
                Addr::unchecked(MOCK_CONTRACT_ADDR),
                Uint128::new(19700000u128),
            )],
        );
        env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(1_020_040_95),
            ..Default::default()
        });
        let unlock_s =
            execute(deps.as_mut(), env.clone(), info.clone(), unlock_msg.clone()).unwrap();
        assert_eq!(
            unlock_s.attributes,
            vec![
                attr("action", "lockdrop::ExecuteMsg::UnlockPosition"),
                attr("owner", "depositor"),
                attr("duration", "3"),
                attr("maUST_unlocked", "9850000")
            ]
        );
        assert_eq!(
            unlock_s.messages,
            vec![
                SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "incentives".to_string(),
                    msg: to_binary(&mars::incentives::msg::ExecuteMsg::ClaimRewards {}).unwrap(),
                    funds: vec![]
                })),
                SubMsg::new(
                    CallbackMsg::UpdateStateOnClaim {
                        user: Addr::unchecked("depositor".to_string()),
                        prev_xmars_balance: Uint256::from(19700000u64)
                    }
                    .to_cosmos_msg(&env.clone().contract.address)
                    .unwrap()
                ),
                SubMsg::new(
                    CallbackMsg::DissolvePosition {
                        user: Addr::unchecked("depositor".to_string()),
                        duration: 3u64
                    }
                    .to_cosmos_msg(&env.clone().contract.address)
                    .unwrap()
                ),
                SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "ma_ust_token".to_string(),
                    msg: to_binary(&cw20::Cw20ExecuteMsg::Transfer {
                        recipient: "depositor".to_string(),
                        amount: Uint128::from(9850000u128),
                    })
                    .unwrap(),
                    funds: vec![]
                })),
            ]
        );
    }

    #[test]
    fn test_try_dissolve_position() {
        let mut deps = th_setup(&[]);
        let deposit_amount = 1000000u128;
        let mut info =
            cosmwasm_std::testing::mock_info("depositor", &[coin(deposit_amount, "uusd")]);
        deps.querier
            .set_incentives_address(Addr::unchecked("incentives".to_string()));
        // Set tax data
        deps.querier.set_native_tax(
            Decimal::from_ratio(1u128, 100u128),
            &[(String::from("uusd"), Uint128::new(100u128))],
        );
        deps.querier
            .set_incentives_address(Addr::unchecked("incentives".to_string()));

        // ***** Setup *****

        let env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(1_000_000_15),
            ..Default::default()
        });

        // Create a lockdrop position for testing
        let mut deposit_msg = ExecuteMsg::DepositUst { duration: 3u64 };
        let mut deposit_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            deposit_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            deposit_s.attributes,
            vec![
                attr("action", "lockdrop::ExecuteMsg::LockUST"),
                attr("user", "depositor"),
                attr("duration", "3"),
                attr("ust_deposited", "1000000")
            ]
        );
        deposit_msg = ExecuteMsg::DepositUst { duration: 5u64 };
        deposit_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            deposit_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            deposit_s.attributes,
            vec![
                attr("action", "lockdrop::ExecuteMsg::LockUST"),
                attr("user", "depositor"),
                attr("duration", "5"),
                attr("ust_deposited", "1000000")
            ]
        );

        // *** Successfully updates the state post deposit in Red Bank ***
        deps.querier.set_cw20_balances(
            Addr::unchecked("ma_ust_token".to_string()),
            &[(
                Addr::unchecked(MOCK_CONTRACT_ADDR),
                Uint128::new(19700000u128),
            )],
        );
        info = mock_info(&env.clone().contract.address.to_string());
        let callback_msg = ExecuteMsg::Callback(CallbackMsg::UpdateStateOnRedBankDeposit {
            prev_ma_ust_balance: Uint256::from(0u64),
        });
        execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            callback_msg.clone(),
        )
        .unwrap();

        // ***
        // *** Test #1 :: Should successfully dissolve the position ***
        // ***
        let mut callback_dissolve_msg = ExecuteMsg::Callback(CallbackMsg::DissolvePosition {
            user: Addr::unchecked("depositor".to_string()),
            duration: 3u64,
        });
        let mut dissolve_position_callback_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            callback_dissolve_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            dissolve_position_callback_s.attributes,
            vec![
                attr("action", "lockdrop::Callback::DissolvePosition"),
                attr("user", "depositor"),
                attr("duration", "3"),
            ]
        );
        // let's verify the state
        let mut state_ = query_state(deps.as_ref()).unwrap();
        assert_eq!(Uint256::from(2000000u64), state_.final_ust_locked);
        assert_eq!(Uint256::from(19700000u64), state_.final_maust_locked);
        assert_eq!(Uint256::from(9850000u64), state_.total_maust_locked);
        assert_eq!(Uint256::from(720000u64), state_.total_deposits_weight);
        // let's verify the User
        deps.querier
            .set_unclaimed_rewards("cosmos2contract".to_string(), Uint128::from(0u64));
        let mut user_ =
            query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
        assert_eq!(Uint256::from(1000000u64), user_.total_ust_locked);
        assert_eq!(Uint256::from(9850000u64), user_.total_maust_locked);
        assert_eq!(vec!["depositor5".to_string()], user_.lockup_position_ids);
        // let's verify user's lockup #1 (which is dissolved)
        let mut lockdrop_ =
            query_lockup_info_with_id(deps.as_ref(), "depositor3".to_string()).unwrap();
        assert_eq!(Uint256::from(0u64), lockdrop_.ust_locked);
        assert_eq!(Uint256::from(0u64), lockdrop_.maust_balance);

        // ***
        // *** Test #2 :: Should successfully dissolve the position ***
        // ***
        callback_dissolve_msg = ExecuteMsg::Callback(CallbackMsg::DissolvePosition {
            user: Addr::unchecked("depositor".to_string()),
            duration: 5u64,
        });
        dissolve_position_callback_s = execute(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            callback_dissolve_msg.clone(),
        )
        .unwrap();
        assert_eq!(
            dissolve_position_callback_s.attributes,
            vec![
                attr("action", "lockdrop::Callback::DissolvePosition"),
                attr("user", "depositor"),
                attr("duration", "5"),
            ]
        );
        // let's verify the state
        state_ = query_state(deps.as_ref()).unwrap();
        assert_eq!(Uint256::from(2000000u64), state_.final_ust_locked);
        assert_eq!(Uint256::from(19700000u64), state_.final_maust_locked);
        assert_eq!(Uint256::from(0u64), state_.total_maust_locked);
        assert_eq!(Uint256::from(720000u64), state_.total_deposits_weight);
        // let's verify the User
        user_ = query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
        assert_eq!(Uint256::from(0u64), user_.total_ust_locked);
        assert_eq!(Uint256::from(0u64), user_.total_maust_locked);
        // let's verify user's lockup #1 (which is dissolved)
        lockdrop_ = query_lockup_info_with_id(deps.as_ref(), "depositor5".to_string()).unwrap();
        assert_eq!(Uint256::from(0u64), lockdrop_.ust_locked);
        assert_eq!(Uint256::from(0u64), lockdrop_.maust_balance);
    }

    fn th_setup(contract_balances: &[Coin]) -> OwnedDeps<MockStorage, MockApi, MarsMockQuerier> {
        let mut deps = mock_dependencies(contract_balances);
        let info = mock_info("owner");
        let env = mock_env(MockEnvParams {
            block_time: Timestamp::from_seconds(1_000_000_00),
            ..Default::default()
        });
        // Config with valid base params
        let base_config = InstantiateMsg {
            owner: "owner".to_string(),
            address_provider: Some("address_provider".to_string()),
            ma_ust_token: Some("ma_ust_token".to_string()),
            init_timestamp: 1_000_000_10,
            deposit_window: 100000u64,
            withdrawal_window: 72000u64,
            min_duration: 3u64,
            max_duration: 9u64,
            seconds_per_week: 7 * 86400 as u64, 
            denom: Some("uusd".to_string()),
            weekly_multiplier: Some(Decimal256::from_ratio(9u64, 100u64)),
            lockdrop_incentives: Some(Uint256::from(21432423343u64)),
        };
        instantiate(deps.as_mut(), env, info, base_config).unwrap();
        deps
    }
}
