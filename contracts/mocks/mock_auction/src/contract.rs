use std::ops::Div;

use cosmwasm_bignumber::{Decimal256, Uint256};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, from_binary, to_binary, Addr, Binary, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env,
    MessageInfo, QueryRequest, Response, StdError, StdResult, Uint128, WasmMsg, WasmQuery,
};

use astroport_periphery::airdrop::ExecuteMsg::EnableClaims as AirdropEnableClaims;
use astroport_periphery::auction::{
    CallbackMsg, ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, StateResponse,
    UpdateConfigMsg, UserInfoResponse,
};
use astroport_periphery::helpers::{
    build_approve_cw20_msg, build_send_native_asset_msg, build_transfer_cw20_token_msg,
    cw20_get_balance, get_denom_amount_from_coins, option_string_to_addr, zero_address,
};
use astroport_periphery::lockdrop::ExecuteMsg::EnableClaims as LockdropEnableClaims;
use astroport_periphery::tax::compute_tax;

use astroport::asset::{Asset, AssetInfo};
use astroport::generator::{PendingTokenResponse, QueryMsg as GenQueryMsg};

use crate::state::{Config, State, UserInfo, CONFIG, STATE, USERS};
use cw20::Cw20ReceiveMsg;

//----------------------------------------------------------------------------------------
// Entry points
//----------------------------------------------------------------------------------------

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let config = Config {
        owner: deps.api.addr_validate(&msg.owner)?,
        astro_token_address: deps.api.addr_validate(&msg.astro_token_address)?,
        airdrop_contract_address: deps.api.addr_validate(&msg.airdrop_contract_address)?,
        lockdrop_contract_address: deps.api.addr_validate(&msg.lockdrop_contract_address)?,
        astroport_lp_pool: option_string_to_addr(deps.api, msg.astroport_lp_pool, zero_address())?,
        lp_token_address: option_string_to_addr(deps.api, msg.lp_token_address, zero_address())?,
        generator_contract: option_string_to_addr(
            deps.api,
            msg.generator_contract,
            zero_address(),
        )?,
        astro_rewards: msg.astro_rewards,
        astro_vesting_duration: msg.astro_vesting_duration,
        lp_tokens_vesting_duration: msg.lp_tokens_vesting_duration,
        init_timestamp: msg.init_timestamp,
        deposit_window: msg.deposit_window,
        withdrawal_window: msg.withdrawal_window,
    };

    let state = State {
        total_astro_deposited: Uint256::zero(),
        total_ust_deposited: Uint256::zero(),
        lp_shares_minted: Uint256::zero(),
        lp_shares_withdrawn: Uint256::zero(),
        pool_init_timestamp: 0u64,
        are_staked: false,
        global_reward_index: Decimal256::zero(),
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
        ExecuteMsg::UpdateConfig { new_config } => handle_update_config(deps, info, new_config),

        ExecuteMsg::DepositUst {} => handle_deposit_ust(deps, env, info),
        ExecuteMsg::WithdrawUst { amount } => handle_withdraw_ust(deps, env, info, amount),

        ExecuteMsg::AddLiquidityToAstroportPool { slippage } => {
            handle_add_liquidity_to_astroport_pool(deps, env, info, slippage)
        }
        ExecuteMsg::StakeLpTokens {} => handle_stake_lp_tokens(deps, env, info),

        ExecuteMsg::ClaimRewards {} => handle_claim_rewards(deps, env, info),
        ExecuteMsg::WithdrawLpShares {} => handle_withdraw_unlocked_lp_shares(deps, env, info),

        ExecuteMsg::Callback(msg) => _handle_callback(deps, env, info, msg),
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;

    // CHECK :: ASTRO deposits can happen only via airdrop / lockdrop contracts
    if config.airdrop_contract_address != cw20_msg.sender
        && config.lockdrop_contract_address != cw20_msg.sender
    {
        return Err(StdError::generic_err("Unauthorized"));
    }

    let amount = cw20_msg.amount;
    // CHECK ::: Amount needs to be valid
    if amount == Uint128::zero() {
        return Err(StdError::generic_err("Amount must be greater than 0"));
    }

    match from_binary(&cw20_msg.msg)? {
        Cw20HookMsg::DepositAstroTokens { user_address } => {
            handle_deposit_astro_tokens(deps, env, info, user_address, amount.into())
        }
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
        CallbackMsg::UpdateStateOnLiquidityAdditionToPool { prev_lp_balance } => {
            update_state_on_liquidity_addition_to_pool(deps, env, prev_lp_balance)
        }
        CallbackMsg::UpdateStateOnRewardClaim {
            user_address,
            prev_astro_balance,
        } => update_state_on_reward_claim(deps, env, user_address, prev_astro_balance),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State {} => to_binary(&query_state(deps)?),
        QueryMsg::UserInfo { address } => to_binary(&query_user_info(deps, _env, address)?),
    }
}

//----------------------------------------------------------------------------------------
// Handle functions
//----------------------------------------------------------------------------------------

/// @dev Admin function to update Configuration parameters
/// @param new_config : Same as UpdateConfigMsg struct
pub fn handle_update_config(
    deps: DepsMut,
    info: MessageInfo,
    new_config: UpdateConfigMsg,
) -> StdResult<Response> {
    let mut config = CONFIG.load(deps.storage)?;

    // CHECK :: ONLY OWNER CAN CALL THIS FUNCTION
    if info.sender != config.owner {
        return Err(StdError::generic_err("Only owner can update configuration"));
    }

    // UPDATE :: ADDRESSES IF PROVIDED
    config.owner = option_string_to_addr(deps.api, new_config.owner, config.owner)?;
    config.astroport_lp_pool = option_string_to_addr(
        deps.api,
        new_config.astroport_lp_pool,
        config.astroport_lp_pool,
    )?;
    config.lp_token_address = option_string_to_addr(
        deps.api,
        new_config.lp_token_address,
        config.lp_token_address,
    )?;
    config.generator_contract = option_string_to_addr(
        deps.api,
        new_config.generator_contract,
        config.generator_contract,
    )?;

    // UPDATE :: VALUES IF PROVIDED
    config.astro_rewards = new_config.astro_rewards.unwrap_or(config.astro_rewards);

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "Auction::ExecuteMsg::UpdateConfig"))
}

/// @dev Accepts ASTRO tokens to be used for the LP Bootstrapping via auction. Callable only by Airdrop / Lockdrop contracts
/// @param user_address : User address who is delegating the ASTRO tokens for LP Pool bootstrap via auction
/// @param amount : Number of ASTRO Tokens being deposited
pub fn handle_deposit_astro_tokens(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    user_address: Addr,
    amount: Uint256,
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;

    // CHECK :: Lockdrop deposit window open
    if !is_deposit_open(_env.block.time.seconds(), &config) {
        return Err(StdError::generic_err("Deposit window closed"));
    }

    let mut state = STATE.load(deps.storage)?;
    let mut user_info = USERS
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    // UPDATE STATE
    state.total_astro_deposited += amount;
    user_info.astro_deposited += amount;

    // SAVE UPDATED STATE
    STATE.save(deps.storage, &state)?;
    USERS.save(deps.storage, &user_address, &user_info)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "Auction::ExecuteMsg::DepositAstroTokens"),
        attr("user", user_address.to_string()),
        attr("astro_deposited", amount),
    ]))
}

/// @dev Facilitates UST deposits by users to be used for LP Bootstrapping via auction
pub fn handle_deposit_ust(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;

    // CHECK :: Lockdrop deposit window open
    if !is_deposit_open(_env.block.time.seconds(), &config) {
        return Err(StdError::generic_err("Deposit window closed"));
    }

    let mut state = STATE.load(deps.storage)?;
    let user_address = info.sender;
    let mut user_info = USERS
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    // Retrieve UST sent by the user
    let deposit_amount = get_denom_amount_from_coins(&info.funds, &"uusd".to_string());

    // CHECK ::: Amount needs to be valid
    if deposit_amount == Uint256::zero() {
        return Err(StdError::generic_err("Amount must be greater than 0"));
    }

    // UPDATE STATE
    state.total_ust_deposited += deposit_amount;
    user_info.ust_deposited += deposit_amount;

    // SAVE UPDATED STATE
    STATE.save(deps.storage, &state)?;
    USERS.save(deps.storage, &user_address, &user_info)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "Auction::ExecuteMsg::DepositUst"),
        attr("user", user_address.to_string()),
        attr("ust_deposited", deposit_amount),
    ]))
}

/// @dev Facilitates UST withdrawals by users from their deposit positions
/// @param amount : UST amount being withdrawn
pub fn handle_withdraw_ust(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    amount: Uint256,
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let user_address = info.sender;
    let mut user_info = USERS
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    // CHECK :: Has the user already withdrawn during the current window
    if user_info.withdrawl_counter {
        return Err(StdError::generic_err(
            "Max 1 withdrawal allowed during current window",
        ));
    }

    // Check :: Amount should be within the allowed withdrawal limit bounds
    let max_withdrawal_percent =
        calculate_max_withdrawal_percent_allowed(_env.block.time.seconds(), &config);
    let max_withdrawal_allowed = user_info.ust_deposited * max_withdrawal_percent;
    if amount > max_withdrawal_allowed {
        return Err(StdError::generic_err(format!(
            "Amount exceeds maximum allowed withdrawal limit of {}",
            max_withdrawal_percent
        )));
    }
    // Set user's withdrawl_counter to true incase no further withdrawals are allowed for the user
    if max_withdrawal_percent <= Decimal256::from_ratio(50u32, 100u32) {
        user_info.withdrawl_counter = true;
    }

    // UPDATE STATE
    state.total_ust_deposited = state.total_ust_deposited - amount;
    user_info.ust_deposited = user_info.ust_deposited - amount;

    // SAVE UPDATED STATE
    STATE.save(deps.storage, &state)?;
    USERS.save(deps.storage, &user_address, &user_info)?;

    // COSMOSMSG :: Transfer UST to the user
    let ust_transfer_msg =
        build_send_native_asset_msg(deps.as_ref(), user_address.clone(), "uusd", amount)?;

    Ok(Response::new()
        .add_message(ust_transfer_msg)
        .add_attributes(vec![
            attr("action", "Auction::ExecuteMsg::WithdrawUst"),
            attr("user", user_address.to_string()),
            attr("ust_withdrawn", amount),
        ]))
}

/// @dev Admin function to bootstrap the ASTRO-UST Liquidity pool by depositing all ASTRO, UST tokens deposited to the Astroport pool
/// @param slippage Optional, to handle slippage that may be there when adding liquidity to the pool
pub fn handle_add_liquidity_to_astroport_pool(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    slippage: Option<Decimal>,
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;

    // CHECK :: Only admin can call this function
    if state.lp_shares_minted != Uint256::zero() {
        return Err(StdError::generic_err("Liquidity already added"));
    }

    // CHECK :: Only admin can call this function
    if info.sender != config.owner {
        return Err(StdError::generic_err("Unauthorized"));
    }

    // CHECK :: Deposit / withdrawal windows need to be over
    if !are_windows_closed(_env.block.time.seconds(), &config) {
        return Err(StdError::generic_err(
            "Deposit/withdrawal windows are still open",
        ));
    }

    let mut msgs_ = vec![];
    // QUERY CURRENT LP TOKEN BALANCE (FOR SAFETY - IN ANY CASE)
    let cur_lp_balance = cw20_get_balance(
        &deps.querier,
        config.lp_token_address.clone(),
        _env.contract.address.clone(),
    )?;

    // COSMOS MSGS
    // :: 1.  APPROVE ASTRO WITH LP POOL ADDRESS AS BENEFICIARY
    // :: 2.  ADD LIQUIDITY
    // :: 3. CallbackMsg :: Update state on liquidity addition to LP Pool
    // :: 4. Activate Claims on Lockdrop Contract
    // :: 5. Activate Claims on Airdrop Contract
    let approve_astro_msg = build_approve_cw20_msg(
        config.astro_token_address.to_string(),
        config.astroport_lp_pool.to_string(),
        state.total_astro_deposited.into(),
    )?;
    let add_liquidity_msg =
        build_provide_liquidity_to_lp_pool_msg(deps.as_ref(), &config, &state, slippage)?;
    let update_state_msg = CallbackMsg::UpdateStateOnLiquidityAdditionToPool {
        prev_lp_balance: cur_lp_balance.into(),
    }
    .to_cosmos_msg(&_env.contract.address)?;
    msgs_.push(approve_astro_msg);
    msgs_.push(add_liquidity_msg);
    msgs_.push(update_state_msg);

    Ok(Response::new().add_messages(msgs_).add_attributes(vec![
        attr("action", "Auction::ExecuteMsg::AddLiquidityToAstroportPool"),
        attr("astro_deposited", state.total_astro_deposited),
        attr("ust_deposited", state.total_ust_deposited),
    ]))
}

/// @dev Admin function to stake Astroport LP tokens with the generator contract
pub fn handle_stake_lp_tokens(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // CHECK :: Only admin can call this function
    if info.sender != config.owner {
        return Err(StdError::generic_err("Unauthorized"));
    }

    // CHECK :: Fail if already staked
    if state.are_staked {
        return Err(StdError::generic_err("Already staked"));
    }

    // COSMOS MSGs
    // :: Add increase allowance msg so generator contract can transfer tokens to itself
    // :: Add stake LP Tokens to the Astroport generator contract msg
    let mut cosmos_msgs = vec![];
    cosmos_msgs.push(build_approve_cw20_msg(
        config.lp_token_address.to_string(),
        config.generator_contract.to_string(),
        state.lp_shares_minted.into(),
    )?);
    cosmos_msgs.push(build_stake_with_generator_msg(
        &config,
        state.lp_shares_minted,
    )?);
    state.are_staked = true;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_messages(cosmos_msgs)
        .add_attributes(vec![
            attr("action", "Auction::ExecuteMsg::StakeLPTokens"),
            attr("staked_amount", state.lp_shares_minted.to_string()),
        ]))
}

/// @dev Facilitates ASTRO Reward claim for users
pub fn handle_claim_rewards(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;
    let user_address = info.sender;
    let mut user_info = USERS
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    // CHECK :: Deposit / withdrawal windows need to be over
    if !are_windows_closed(_env.block.time.seconds(), &config) {
        return Err(StdError::generic_err("Deposit/withdrawal windows are open"));
    }

    // CHECK :: Does user have valid ASTRO / UST deposit balances
    if user_info.astro_deposited == Uint256::zero() && user_info.ust_deposited == Uint256::zero() {
        return Err(StdError::generic_err("Invalid request"));
    }

    // User's LP shares :: Calculate if not already calculated
    if user_info.lp_shares == Uint256::zero() {
        user_info.lp_shares = calculate_user_lp_share(&state, &user_info);
    }
    // ASTRO INCENTIVES :: Calculates ASTRO rewards for auction participation for a user if not already done
    if user_info.total_auction_incentives == Uint256::zero() {
        user_info.total_auction_incentives =
            calculate_auction_reward_for_user(&state, &user_info, config.astro_rewards);
    }

    let mut cosmos_msgs = vec![];

    // QUERY :: ARE ASTRO REWARDS TO BE CLAIMED FOR LP STAKING > 0 ?
    // --> If unclaimed rewards > 0, add claimReward {} msg to the cosmos msg array
    if state.are_staked {
        let unclaimed_rewards_response =
            query_unclaimed_staking_rewards(deps.as_ref(), &config, _env.contract.address.clone());
        if unclaimed_rewards_response > Uint128::zero() {
            cosmos_msgs.push(build_claim_astro_rewards(
                _env.contract.address.clone(),
                config.lp_token_address,
                config.generator_contract.clone(),
            )?);
        }
    }

    // QUERY :: Current ASTRO Contract Balance
    // -->add CallbackMsg::UpdateStateOnRewardClaim{} msg to the cosmos msg array
    let astro_balance = cw20_get_balance(
        &deps.querier,
        config.astro_token_address,
        _env.contract.address.clone(),
    )?;
    let update_state_msg = CallbackMsg::UpdateStateOnRewardClaim {
        user_address: user_address.clone(),
        prev_astro_balance: astro_balance.into(),
    }
    .to_cosmos_msg(&_env.contract.address)?;
    cosmos_msgs.push(update_state_msg);

    USERS.save(deps.storage, &user_address, &user_info)?;

    Ok(Response::new()
        .add_messages(cosmos_msgs)
        .add_attributes(vec![
            attr("action", "Auction::ExecuteMsg::ClaimRewards"),
            attr("user", user_address.to_string()),
        ]))
}

/// @dev Facilitates ASTRO Reward claim for users
pub fn handle_withdraw_unlocked_lp_shares(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let user_address = info.sender;
    let mut user_info = USERS
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    // CHECK :: Deposit / withdrawal windows need to be over
    if !are_windows_closed(_env.block.time.seconds(), &config) {
        return Err(StdError::generic_err("Deposit/withdrawal windows are open"));
    }

    // CHECK :: User has valid delegation / deposit balances
    if user_info.astro_deposited == Uint256::zero() && user_info.ust_deposited == Uint256::zero() {
        return Err(StdError::generic_err(
            "Invalid request. No LP Tokens to claim",
        ));
    }

    // User's LP shares :: Calculate if not already calculated
    if user_info.lp_shares == Uint256::zero() {
        user_info.lp_shares = calculate_user_lp_share(&state, &user_info);
    }
    // ASTRO INCENTIVES :: Calculates ASTRO rewards for auction participation for a user if not already done
    if user_info.total_auction_incentives == Uint256::zero() {
        user_info.total_auction_incentives =
            calculate_auction_reward_for_user(&state, &user_info, config.astro_rewards);
    }

    let mut cosmos_msgs = vec![];

    // QUERY :: ARE ASTRO REWARDS TO BE CLAIMED FOR LP STAKING > 0 ?
    // --> If unclaimed rewards > 0, add claimReward {} msg to the cosmos msg array
    if state.are_staked {
        let unclaimed_rewards_response =
            query_unclaimed_staking_rewards(deps.as_ref(), &config, _env.contract.address.clone());
        if unclaimed_rewards_response > Uint128::zero() {
            cosmos_msgs.push(build_claim_astro_rewards(
                _env.contract.address.clone(),
                config.lp_token_address.clone(),
                config.generator_contract.clone(),
            )?);
        }
    }

    // QUERY :: Current ASTRO Token Balance
    // -->add CallbackMsg::UpdateStateOnRewardClaim{} msg to the cosmos msg array
    let astro_balance = cw20_get_balance(
        &deps.querier,
        config.astro_token_address.clone(),
        _env.contract.address.clone(),
    )?;
    let update_state_msg = CallbackMsg::UpdateStateOnRewardClaim {
        user_address: user_address.clone(),
        prev_astro_balance: astro_balance.into(),
    }
    .to_cosmos_msg(&_env.contract.address)?;
    cosmos_msgs.push(update_state_msg);

    // CALCULATE LP SHARES THAT THE USER CAN WITHDRAW (TO DO :: FIGURE THE LOGIC i.e cliff or vesting)
    let lp_shares_to_withdraw =
        calculate_withdrawable_lp_shares(_env.block.time.seconds(), &config, &state, &user_info);
    if lp_shares_to_withdraw == Uint256::zero() {
        return Err(StdError::generic_err("No LP shares to withdraw"));
    }

    // COSMOS MSG's :: LP SHARES CLAIM
    // --> 1. Withdraw LP shares
    // --> 2. Transfer LP shares
    if state.are_staked {
        let unstake_lp_shares = build_unstake_from_generator_msg(&config, lp_shares_to_withdraw)?;
        cosmos_msgs.push(unstake_lp_shares);
    }
    let transfer_lp_shares = build_transfer_cw20_token_msg(
        user_address.clone(),
        config.lp_token_address.to_string(),
        lp_shares_to_withdraw.into(),
    )?;
    cosmos_msgs.push(transfer_lp_shares);

    // STATE UPDATE --> SAVE
    user_info.withdrawn_lp_shares += lp_shares_to_withdraw;
    state.lp_shares_withdrawn += lp_shares_to_withdraw;
    USERS.save(deps.storage, &user_address, &user_info)?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_messages(cosmos_msgs)
        .add_attributes(vec![
            attr("action", "Auction::ExecuteMsg::WithdrawLPShares"),
            attr("user", user_address.to_string()),
            attr("LP_shares_withdrawn", lp_shares_to_withdraw),
        ]))
}

//----------------------------------------------------------------------------------------
// Handle::Callback functions
//----------------------------------------------------------------------------------------

// CALLBACK :: CALLED AFTER ASTRO, UST LIQUIDITY IS ADDED TO THE LP POOL
pub fn update_state_on_liquidity_addition_to_pool(
    deps: DepsMut,
    env: Env,
    prev_lp_balance: Uint256,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // QUERY CURRENT LP TOKEN BALANCE :: NEWLY MINTED LP TOKENS
    let cur_lp_balance = cw20_get_balance(
        &deps.querier,
        config.lp_token_address.clone(),
        env.contract.address,
    )?;

    // STATE :: UPDATE --> SAVE
    state.lp_shares_minted = Uint256::from(cur_lp_balance) - prev_lp_balance;
    state.pool_init_timestamp = env.block.time.seconds();
    STATE.save(deps.storage, &state)?;

    let mut cosmos_msgs = vec![];
    let activate_claims_lockdrop =
        build_activate_claims_lockdrop_msg(config.lockdrop_contract_address)?;
    let activate_claims_airdrop =
        build_activate_claims_airdrop_msg(config.airdrop_contract_address)?;
    cosmos_msgs.push(activate_claims_lockdrop);
    cosmos_msgs.push(activate_claims_airdrop);

    Ok(Response::new()
        .add_messages(cosmos_msgs)
        .add_attributes(vec![
            (
                "action",
                "Auction::CallbackMsg::UpdateStateOnLiquidityAddition",
            ),
            (
                "lp_shares_minted",
                state.lp_shares_minted.to_string().as_str(),
            ),
        ]))
}

// @dev CallbackMsg :: Facilitates state update and ASTRO rewards transfer to users post ASTRO incentives claim from the generator contract
pub fn update_state_on_reward_claim(
    deps: DepsMut,
    _env: Env,
    user_address: Addr,
    prev_astro_balance: Uint256,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let mut user_info = USERS
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    // QUERY ASTRO TOKEN BALANCE :: Claimed Rewards
    let cur_astro_balance = cw20_get_balance(
        &deps.querier,
        config.astro_token_address.clone(),
        _env.contract.address,
    )?;
    let astro_claimed = Uint256::from(cur_astro_balance) - prev_astro_balance;

    let mut user_astro_rewards = Uint256::zero();
    let mut staking_reward = Uint256::zero();

    // ASTRO Incentives :: Calculate the unvested amount which can be claimed by the user
    user_astro_rewards += calculate_withdrawable_auction_reward_for_user(
        _env.block.time.seconds(),
        &config,
        &state,
        &user_info,
    );
    user_info.withdrawn_auction_incentives += user_astro_rewards;

    // ASTRO Generator (Staking) rewards :: Calculate the astro amount (from LP staking incentives) which can be claimed by the user
    if astro_claimed > Uint256::zero() {
        update_astro_rewards_index(&mut state, astro_claimed);
        staking_reward = compute_user_accrued_reward(&state, &mut user_info);
        user_astro_rewards += staking_reward;
    }

    let mut cosmos_msgs = vec![];

    // COSMOS MSG :: Transfer Rewards to the user
    if user_astro_rewards > Uint256::zero() {
        let transfer_astro_rewards = build_transfer_cw20_token_msg(
            user_address.clone(),
            config.astro_token_address.to_string(),
            user_astro_rewards.into(),
        )?;
        cosmos_msgs.push(transfer_astro_rewards);
    }

    // SAVE UPDATED STATE
    STATE.save(deps.storage, &state)?;
    USERS.save(deps.storage, &user_address, &user_info)?;

    Ok(Response::new()
        .add_messages(cosmos_msgs)
        .add_attributes(vec![
            ("action", "Auction::CallbackMsg::UpdateStateOnRewardClaim"),
            ("user_address", user_address.to_string().as_str()),
            (
                "auction_participation_reward",
                &(user_astro_rewards - staking_reward).to_string(),
            ),
            ("staking_lp_reward", &staking_reward.to_string()),
        ]))
}

//----------------------------------------------------------------------------------------
// Query functions
//----------------------------------------------------------------------------------------

/// @dev Returns the airdrop configuration
fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        owner: config.owner.to_string(),
        astro_token_address: config.astro_token_address.to_string(),
        airdrop_contract_address: config.airdrop_contract_address.to_string(),
        lockdrop_contract_address: config.lockdrop_contract_address.to_string(),
        astroport_lp_pool: config.astroport_lp_pool.to_string(),
        lp_token_address: config.lp_token_address.to_string(),
        generator_contract: config.generator_contract.to_string(),
        astro_rewards: config.astro_rewards,
        init_timestamp: config.init_timestamp,
        deposit_window: config.deposit_window,
        withdrawal_window: config.withdrawal_window,
    })
}

/// @dev Returns the airdrop contract state
fn query_state(deps: Deps) -> StdResult<StateResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(StateResponse {
        total_astro_deposited: state.total_astro_deposited,
        total_ust_deposited: state.total_ust_deposited,
        lp_shares_minted: state.lp_shares_minted,
        lp_shares_withdrawn: state.lp_shares_withdrawn,
        are_staked: state.are_staked,
        pool_init_timestamp: state.pool_init_timestamp,
        global_reward_index: state.global_reward_index,
    })
}

/// @dev Returns details around user's ASTRO Airdrop claim
fn query_user_info(deps: Deps, _env: Env, user_address: String) -> StdResult<UserInfoResponse> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let user_address = deps.api.addr_validate(&user_address)?;
    let mut user_info = USERS
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    if user_info.lp_shares == Uint256::zero() {
        user_info.lp_shares = calculate_user_lp_share(&state, &user_info);
    }

    if user_info.total_auction_incentives == Uint256::zero() {
        user_info.total_auction_incentives =
            calculate_auction_reward_for_user(&state, &user_info, config.astro_rewards);
    }
    let withdrawable_lp_shares =
        calculate_withdrawable_lp_shares(_env.block.time.seconds(), &config, &state, &user_info);
    let claimable_auction_reward = calculate_withdrawable_auction_reward_for_user(
        _env.block.time.seconds(),
        &config,
        &state,
        &user_info,
    );
    let mut claimable_staking_reward = Uint256::zero();

    if state.are_staked {
        let unclaimed_rewards_response =
            query_unclaimed_staking_rewards(deps, &config, _env.contract.address.clone());
        if unclaimed_rewards_response > Uint128::zero() {
            update_astro_rewards_index(&mut state, unclaimed_rewards_response.into());
            claimable_staking_reward = compute_user_accrued_reward(&state, &mut user_info);
        }
    }

    Ok(UserInfoResponse {
        astro_deposited: user_info.astro_deposited,
        ust_deposited: user_info.ust_deposited,
        lp_shares: user_info.lp_shares,
        withdrawn_lp_shares: user_info.withdrawn_lp_shares,
        withdrawable_lp_shares,
        total_auction_incentives: user_info.total_auction_incentives,
        withdrawn_auction_incentives: user_info.withdrawn_auction_incentives,
        withdrawable_auction_incentives: claimable_auction_reward,
        user_reward_index: user_info.user_reward_index,
        claimable_staking_incentives: claimable_staking_reward,
    })
}

//----------------------------------------------------------------------------------------
// HELPERS :: LP & REWARD CALCULATIONS
//----------------------------------------------------------------------------------------

/// @dev Calculates user's ASTRO-UST LP Shares
/// Formula -
/// user's ASTRO share %  = user's ASTRO deposits / Total ASTRO deposited
/// user's UST share %  = user's UST deposits / Total UST deposited
/// user's LP balance  = ( user's ASTRO share % + user's UST share % ) / 2 * Total LPs Minted
/// @param state : Contract State
/// @param user_info : User Info State
fn calculate_user_lp_share(state: &State, user_info: &UserInfo) -> Uint256 {
    if state.total_astro_deposited == Uint256::zero()
        || state.total_ust_deposited == Uint256::zero()
    {
        return user_info.lp_shares;
    }
    let user_astro_shares_percent =
        Decimal256::from_ratio(user_info.astro_deposited, state.total_astro_deposited);
    let user_ust_shares_percent =
        Decimal256::from_ratio(user_info.ust_deposited, state.total_ust_deposited);
    let user_total_share_percent = user_astro_shares_percent + user_ust_shares_percent;

    user_total_share_percent.div(Decimal256::from_ratio(2u64, 1u64)) * state.lp_shares_minted
}

/// @dev Calculates ASTRO tokens receivable by a user for participating (providing UST & ASTRO) in the bootstraping phase of the ASTRO-UST Pool
/// Formula - ASTRO per LP share = total ASTRO Incentives / Total LP shares minted
/// user's LP receivable = user's LP share * ASTRO per LP share
/// @param config : Configuration
/// @param state : Contract State
/// @param total_astro_rewards : Total ASTRO tokens to be distributed as auction participation reward
fn calculate_auction_reward_for_user(
    state: &State,
    user_info: &UserInfo,
    total_astro_rewards: Uint256,
) -> Uint256 {
    if user_info.total_auction_incentives > Uint256::zero()
        || state.total_astro_deposited == Uint256::zero()
        || state.total_ust_deposited == Uint256::zero()
    {
        return Uint256::zero();
    }

    let astro_per_lp_share = Decimal256::from_ratio(total_astro_rewards, state.lp_shares_minted);
    astro_per_lp_share * user_info.lp_shares
}

/// @dev Returns LP Balance that a user can withdraw based on the vesting schedule
/// Formula -
/// time elapsed = current timestamp - timestamp when liquidity was added to the ASTRO-UST LP Pool
/// Total LP shares that a user can withdraw =  User's LP shares *  time elapsed / vesting duration
/// LP shares that a user can currently withdraw =  Total LP shares that a user can withdraw  - LP shares withdrawn
/// @param current_timestamp : Current timestamp
/// @param config : Configuration
/// @param state : Contract State
/// @param user_info : User Info State
pub fn calculate_withdrawable_lp_shares(
    cur_timestamp: u64,
    config: &Config,
    state: &State,
    user_info: &UserInfo,
) -> Uint256 {
    if state.pool_init_timestamp == 0u64 {
        return Uint256::zero();
    }
    let time_elapsed = cur_timestamp - state.pool_init_timestamp;

    if time_elapsed >= config.lp_tokens_vesting_duration {
        return user_info.lp_shares - user_info.withdrawn_lp_shares;
    }

    let withdrawable_lp_balance = user_info.lp_shares
        * Decimal256::from_ratio(time_elapsed, config.lp_tokens_vesting_duration);
    withdrawable_lp_balance - user_info.withdrawn_lp_shares
}

/// @dev Returns ASTRO auction incentives that a user can withdraw based on the vesting schedule
/// Formula -
/// time elapsed = current timestamp - timestamp when liquidity was added to the ASTRO-UST LP Pool
/// Total ASTRO that a user can withdraw =  User's ASTRO reward *  time elapsed / vesting duration
/// ASTRO rewards that a user can currently withdraw =  Total ASTRO rewards that a user can withdraw  - ASTRO rewards withdrawn
/// @param current_timestamp : Current timestamp
/// @param config : Configuration
/// @param state : Contract State
/// @param user_info : User Info State
pub fn calculate_withdrawable_auction_reward_for_user(
    cur_timestamp: u64,
    config: &Config,
    state: &State,
    user_info: &UserInfo,
) -> Uint256 {
    if user_info.withdrawn_auction_incentives == user_info.total_auction_incentives
        || state.pool_init_timestamp == 0u64
    {
        return Uint256::zero();
    }

    let time_elapsed = cur_timestamp - state.pool_init_timestamp;
    if time_elapsed >= config.astro_vesting_duration {
        return user_info.total_auction_incentives - user_info.withdrawn_auction_incentives;
    }
    let withdrawable_auction_incentives = user_info.total_auction_incentives
        * Decimal256::from_ratio(time_elapsed, config.astro_vesting_duration);
    withdrawable_auction_incentives - user_info.withdrawn_auction_incentives
}

/// @dev Accrue ASTRO rewards by updating the global reward index
/// Formula -
/// Increment rewards index by amount = ASTRO accrued in generator / (LP shares staked with the generator)
/// global reward index += increment rewards index by amount
fn update_astro_rewards_index(state: &mut State, astro_accured: Uint256) {
    let staked_lp_shares = state.lp_shares_minted - state.lp_shares_withdrawn;
    if !state.are_staked || staked_lp_shares == Uint256::zero() {
        return;
    }
    let astro_rewards_index_increment = Decimal256::from_ratio(astro_accured, staked_lp_shares);
    state.global_reward_index += astro_rewards_index_increment;
}

/// @dev Accrue ASTRO reward for the user by updating the user reward index and adding rewards to the pending rewards
/// Formula -
/// user's staked LP shares
/// Pending user rewards = (user's staked LP shares) * ( global reward index - user reward index )
fn compute_user_accrued_reward(state: &State, user_info: &mut UserInfo) -> Uint256 {
    let staked_lp_shares = user_info.lp_shares - user_info.withdrawn_lp_shares;
    if !state.are_staked {
        return Uint256::zero();
    }
    let pending_user_rewards = (staked_lp_shares * state.global_reward_index)
        - (staked_lp_shares * user_info.user_reward_index);
    user_info.user_reward_index = state.global_reward_index;
    pending_user_rewards
}

//----------------------------------------------------------------------------------------
// HELPERS :: DEPOSIT / WITHDRAW CALCULATIONS
//----------------------------------------------------------------------------------------

/// @dev Helper function. Returns true if the deposit & withdrawal windows are closed, else returns false
/// @param current_timestamp : Current timestamp
/// @param config : Configuration
fn are_windows_closed(current_timestamp: u64, config: &Config) -> bool {
    let opened_till = config.init_timestamp + config.deposit_window + config.withdrawal_window;
    (current_timestamp > opened_till) || (current_timestamp < config.init_timestamp)
}

/// @dev Helper function. Returns true if deposits are allowed
/// @param current_timestamp : Current timestamp
/// @param config : Configuration
fn is_deposit_open(current_timestamp: u64, config: &Config) -> bool {
    let deposits_opened_till = config.init_timestamp + config.deposit_window;
    (config.init_timestamp <= current_timestamp) && (current_timestamp <= deposits_opened_till)
}

///  @dev Helper function to calculate maximum % of their total UST deposited that can be withdrawn.  Returns % UST that can be withdrawn
/// @param current_timestamp : Current timestamp
/// @param config : Configuration
fn calculate_max_withdrawal_percent_allowed(current_timestamp: u64, config: &Config) -> Decimal256 {
    let withdrawal_cutoff_init_point = config.init_timestamp + config.deposit_window;
    // Deposit window :: 100% withdrawals allowed
    if current_timestamp <= withdrawal_cutoff_init_point {
        return Decimal256::from_ratio(100u32, 100u32);
    }

    let withdrawal_cutoff_sec_point =
        withdrawal_cutoff_init_point + (config.withdrawal_window / 2u64);
    // Deposit window closed, 1st half of withdrawal window :: 50% withdrawals allowed
    if current_timestamp <= withdrawal_cutoff_sec_point {
        return Decimal256::from_ratio(50u32, 100u32);
    }
    let withdrawal_cutoff_final = withdrawal_cutoff_sec_point + (config.withdrawal_window / 2u64);
    //  Deposit window closed, 2nd half of withdrawal window :: max withdrawal allowed decreases linearly from 50% to 0% vs time elapsed
    if current_timestamp < withdrawal_cutoff_final {
        let slope = Decimal256::from_ratio(50u64, config.withdrawal_window / 2u64);
        let time_elapsed = current_timestamp - withdrawal_cutoff_sec_point;
        Decimal256::from_ratio(time_elapsed, 1u64) * slope
    }
    // Withdrawals not allowed
    else {
        Decimal256::from_ratio(0u32, 100u32)
    }
}

//----------------------------------------------------------------------------------------
// HELPERS :: QUERIES
//----------------------------------------------------------------------------------------

/// @dev Queries pending rewards to be claimed from the generator contract for the 'contract_addr'
/// @param config : Configuration
/// @param contract_addr : Address for which pending rewards are to be queried
fn query_unclaimed_staking_rewards(deps: Deps, config: &Config, contract_addr: Addr) -> Uint128 {
    let pending_rewards: PendingTokenResponse = deps
        .querier
        .query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: config.generator_contract.to_string(),
            msg: to_binary(&GenQueryMsg::PendingToken {
                lp_token: config.lp_token_address.clone(),
                user: contract_addr,
            })
            .unwrap(),
        }))
        .unwrap();
    pending_rewards.pending
}

//----------------------------------------------------------------------------------------
// HELPERS :: BUILD COSMOS MSG
//----------------------------------------------------------------------------------------

/// @dev Returns CosmosMsg struct to stake LP Tokens with the Generator contract
/// @param config : Configuration
/// @param amount : LP tokens to stake with generator  
pub fn build_stake_with_generator_msg(config: &Config, amount: Uint256) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.generator_contract.to_string(),
        msg: to_binary(&astroport::generator::ExecuteMsg::Deposit {
            lp_token: config.lp_token_address.clone(),
            amount: amount.into(),
        })?,
        funds: vec![],
    }))
}

/// @dev Returns CosmosMsg struct to unstake LP Tokens from the Generator contract
/// @param config : Configuration
/// @param lp_shares_to_unstake : LP tokens to be unstaked from generator  
pub fn build_unstake_from_generator_msg(
    config: &Config,
    lp_shares_to_unstake: Uint256,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.generator_contract.to_string(),
        msg: to_binary(&astroport::generator::ExecuteMsg::Withdraw {
            lp_token: config.lp_token_address.clone(),
            amount: lp_shares_to_unstake.into(),
        })?,
        funds: vec![],
    }))
}

/// @dev Helper function. Returns CosmosMsg struct to facilitate liquidity provision to the Astroport LP Pool
/// @param config : Configuration
/// @param state : Contract state
/// @param slippage_tolerance_ : Optional slippage parameter
fn build_provide_liquidity_to_lp_pool_msg(
    deps: Deps,
    config: &Config,
    state: &State,
    slippage_tolerance_: Option<Decimal>,
) -> StdResult<CosmosMsg> {
    let uusd_denom = "uusd".to_string();
    let uust_to_deposit = Uint128::from(state.total_ust_deposited);
    let uust = Coin {
        denom: uusd_denom.clone(),
        amount: uust_to_deposit,
    };
    let tax_amount = compute_tax(deps, &uust)?;

    // ASSET DEFINATION
    let astro_asset = Asset {
        info: AssetInfo::Token {
            contract_addr: deps
                .api
                .addr_validate(&config.astro_token_address.to_string())?,
        },
        amount: state.total_astro_deposited.into(),
    };
    let ust_asset = Asset {
        info: AssetInfo::NativeToken {
            denom: uusd_denom.clone(),
        },
        amount: uust_to_deposit - tax_amount,
    };
    let assets_ = [astro_asset, ust_asset];

    let uusd_to_send = Coin {
        denom: uusd_denom,
        amount: uust_to_deposit - tax_amount,
    };

    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.astroport_lp_pool.to_string(),
        funds: vec![uusd_to_send],
        msg: to_binary(&astroport::pair::ExecuteMsg::ProvideLiquidity {
            assets: assets_,
            slippage_tolerance: slippage_tolerance_,
            auto_stack: Some(false),
        })?,
    }))
}

/// @dev Helper function. Returns CosmosMsg struct to activate ASTRO tokens claim from the lockdrop contract
/// @param lockdrop_contract_address : Lockdrop contract address
fn build_activate_claims_lockdrop_msg(lockdrop_contract_address: Addr) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lockdrop_contract_address.to_string(),
        msg: to_binary(&LockdropEnableClaims {})?,
        funds: vec![],
    }))
}

/// @dev Helper function. Returns CosmosMsg struct to activate ASTRO tokens claim from the airdrop contract
/// @param airdrop_contract_address : Airdrop contract address
fn build_activate_claims_airdrop_msg(airdrop_contract_address: Addr) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: airdrop_contract_address.to_string(),
        msg: to_binary(&AirdropEnableClaims {})?,
        funds: vec![],
    }))
}

/// @dev Helper function. Returns CosmosMsg struct to claim ASTRO tokens claim from the generator contract
/// @param recepient_address : Address to which claimed rewards are to be sent
/// @param lp_token_contract : LP token Address for which rewards are to be claimed
/// @param generator_contract : Generator contract address
fn build_claim_astro_rewards(
    recepient_address: Addr,
    lp_token_contract: Addr,
    generator_contract: Addr,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: generator_contract.to_string(),
        funds: vec![],
        msg: to_binary(&astroport::generator::ExecuteMsg::SendOrphanProxyReward {
            recipient: recepient_address.to_string(),
            lp_token: lp_token_contract.to_string(),
        })?,
    }))
}
