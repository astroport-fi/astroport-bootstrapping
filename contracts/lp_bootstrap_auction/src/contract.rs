use std::ops::Div;

use cosmwasm_bignumber::{Decimal256, Uint256};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, from_binary, to_binary, Addr, Binary, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env,
    MessageInfo, QueryRequest, Response, StdError, StdResult, Uint128, WasmMsg, WasmQuery,
};

use astroport_periphery::airdrop::ExecuteMsg::EnableClaims as AirdropEnableClaims;
use astroport_periphery::helpers::{
    build_approve_cw20_msg, build_send_native_asset_msg, build_transfer_cw20_token_msg,
    cw20_get_balance, get_denom_amount_from_coins, option_string_to_addr, zero_address,
};
use astroport_periphery::lockdrop::ExecuteMsg::EnableClaims as LockdropEnableClaims;
use astroport_periphery::lp_bootstrap_auction::{
    CallbackMsg, ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, StateResponse,
    UpdateConfigMsg, UserInfoResponse,
};
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
        astro_vesting_schedule: msg.astro_vesting_schedule,
        lp_tokens_vesting_schedule: msg.lp_tokens_vesting_schedule,
        init_timestamp: msg.init_timestamp,
        deposit_window: msg.deposit_window,
        withdrawal_window: msg.withdrawal_window,
    };

    let state = State {
        total_astro_delegated: Uint256::zero(),
        total_ust_deposited: Uint256::zero(),
        lp_shares_minted: Uint256::zero(),
        lp_shares_claimed: Uint256::zero(),
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

    // CHECK :: Delegation can happen only via airdrop / lockdrop contracts
    if config.airdrop_contract_address != info.sender
        && config.lockdrop_contract_address != info.sender
    {
        return Err(StdError::generic_err("Unauthorized"));
    }

    let amount = cw20_msg.amount;
    // CHECK ::: Amount needs to be valid
    if amount > Uint128::zero() {
        return Err(StdError::generic_err("Amount must be greater than 0"));
    }

    match from_binary(&cw20_msg.msg)? {
        Cw20HookMsg::DelegateAstroTokens { user_address } => {
            handle_delegate_astro_tokens(deps, env, info, user_address, amount.into())
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

/// @dev Delegates ASTRO tokens to be used for the LP Bootstrapping via auction. Callable only by Airdrop / Lockdrop contracts
/// @param user_address : User address who is delegating the ASTRO tokens for LP Pool bootstrap via auction
/// @param amount : Number of ASTRO Tokens being delegated
pub fn handle_delegate_astro_tokens(
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
    state.total_astro_delegated += amount;
    user_info.astro_delegated += amount;

    // SAVE UPDATED STATE
    STATE.save(deps.storage, &state)?;
    USERS.save(deps.storage, &user_address, &user_info)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "Auction::ExecuteMsg::DelegateAstroTokens"),
        attr("user", user_address.to_string()),
        attr("astro_delegated", amount),
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
    let depositor_address = info.sender;
    let mut user_info = USERS
        .may_load(deps.storage, &depositor_address)?
        .unwrap_or_default();

    // Retrieve UST sent by the user
    let deposit_amount = get_denom_amount_from_coins(&info.funds, &"uusd".to_string());

    // UPDATE STATE
    state.total_ust_deposited += deposit_amount;
    user_info.ust_deposited += deposit_amount;

    // SAVE UPDATED STATE
    STATE.save(deps.storage, &state)?;
    USERS.save(deps.storage, &depositor_address, &user_info)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "Auction::ExecuteMsg::DepositUst"),
        attr("user", depositor_address.to_string()),
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
        return Err(StdError::generic_err("Max 1 withdrawal allowed"));
    }

    // Check :: Amount should be within the allowed withdrawal limit bounds
    let max_withdrawal_percent =
        calculate_max_withdrawal_percent_allowed(_env.block.time.seconds(), &config);
    let max_withdrawal_allowed = user_info.ust_deposited * max_withdrawal_percent;
    if amount > max_withdrawal_allowed {
        return Err(StdError::generic_err(
            "Amount exceeds maximum allowed withdrawal limit",
        ));
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
    // :: 5. Update Claims on Airdrop Contract
    let approve_astro_msg = build_approve_cw20_msg(
        config.astro_token_address.to_string(),
        config.astroport_lp_pool.to_string(),
        state.total_astro_delegated.into(),
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
        attr("astro_deposited", state.total_astro_delegated),
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

    // CHECK :: Only admin can call this function
    if state.are_staked {
        return Err(StdError::generic_err("Already staked"));
    }

    //COSMOS MSG :: To stake LP Tokens to the Astroport generator contract
    let stake_msg = build_stake_with_generator_msg(&config, state.lp_shares_minted)?;
    state.are_staked = true;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new().add_message(stake_msg).add_attributes(vec![
        attr("action", "Auction::ExecuteMsg::StakeLPTokens"),
        attr("amount", state.lp_shares_minted.to_string()),
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
    let depositor_address = info.sender;
    let user_info = USERS
        .may_load(deps.storage, &depositor_address)?
        .unwrap_or_default();

    // CHECK :: Deposit / withdrawal windows need to be over
    if !are_windows_closed(_env.block.time.seconds(), &config) {
        return Err(StdError::generic_err("Deposit/withdrawal windows are open"));
    }

    // CHECK :: User has valid delegation / deposit balances
    if user_info.astro_delegated == Uint256::zero() && user_info.ust_deposited == Uint256::zero() {
        return Err(StdError::generic_err("Invalid request"));
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
        user_address: depositor_address.clone(),
        prev_astro_balance: astro_balance.into(),
    }
    .to_cosmos_msg(&_env.contract.address)?;
    cosmos_msgs.push(update_state_msg);

    Ok(Response::new()
        .add_messages(cosmos_msgs)
        .add_attributes(vec![
            attr("action", "Auction::ExecuteMsg::ClaimRewards"),
            attr("user", depositor_address.to_string()),
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
    if user_info.astro_delegated == Uint256::zero() && user_info.ust_deposited == Uint256::zero() {
        return Err(StdError::generic_err(
            "Invalid request. No LP Tokens to claim",
        ));
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
    user_info.claimed_lp_shares += lp_shares_to_withdraw;
    state.lp_shares_claimed += lp_shares_to_withdraw;
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
    STATE.save(deps.storage, &state)?;

    let mut cosmos_msgs = vec![];
    let activate_claims_lockdrop =
        build_activate_claims_lockdrop_msg(config.lockdrop_contract_address)?;
    let activate_claims_airdrop =
        build_activate_claims_airdrop_msg(config.airdrop_contract_address)?;
    cosmos_msgs.push(activate_claims_lockdrop);
    cosmos_msgs.push(activate_claims_airdrop);

    Ok(Response::new().add_attributes(vec![
        (
            "action",
            "Auction::CallbackMsg::UpdateStateOnLiquidityAddition",
        ),
        // ("maUST_minted", m_ust_minted.to_string().as_str()),
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

    // QUERY CURRENT LP TOKEN BALANCE :: NEWLY MINTED LP TOKENS
    let cur_astro_balance = cw20_get_balance(
        &deps.querier,
        config.astro_token_address.clone(),
        _env.contract.address,
    )?;
    let astro_claimed = Uint256::from(cur_astro_balance) - prev_astro_balance;

    let mut user_astro_rewards = Uint256::zero();

    // ASTRO INCENTIVES :: Calculates ASTRO rewards for auction participation for a user if not already done
    let mut staking_reward = Uint256::zero();
    if user_info.total_auction_incentives == Uint256::zero() {
        user_info.total_auction_incentives =
            calculate_auction_reward_for_user(&state, &user_info, config.astro_rewards);
    }

    // ASTRO Incentives :: Calculate the unvested amount which can be claimed by the user
    user_astro_rewards += calculate_claimable_auction_reward_for_user(
        _env.block.time.seconds(),
        &config,
        &state,
        &user_info,
    );
    user_info.claimed_auction_incentives += user_astro_rewards;

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
        lp_token_address: config.lp_token_address.to_string(),
        lockdrop_contract_address: config.lockdrop_contract_address.to_string(),
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
        total_astro_delegated: state.total_astro_delegated,
        total_ust_deposited: state.total_ust_deposited,
        lp_shares_minted: state.lp_shares_minted,
        lp_shares_claimed: state.lp_shares_claimed,
        are_staked: state.are_staked,
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

    if user_info.total_auction_incentives == Uint256::zero() {
        user_info.total_auction_incentives =
            calculate_auction_reward_for_user(&state, &user_info, config.astro_rewards);
    }
    let claimable_lp_shares =
        calculate_withdrawable_lp_shares(_env.block.time.seconds(), &config, &state, &user_info);
    let claimable_auction_reward = calculate_claimable_auction_reward_for_user(
        _env.block.time.seconds(),
        &config,
        &state,
        &user_info,
    );
    let mut claimable_staking_reward = Uint256::zero();

    if state.are_staked {
        let unclaimed_rewards_response: PendingTokenResponse = deps
            .querier
            .query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: config.generator_contract.to_string(),
                msg: to_binary(&GenQueryMsg::PendingToken {
                    lp_token: config.lp_token_address,
                    user: _env.contract.address,
                })
                .unwrap(),
            }))
            .unwrap();
        if unclaimed_rewards_response.pending > Uint128::zero() {
            update_astro_rewards_index(&mut state, unclaimed_rewards_response.pending.into());
            claimable_staking_reward = compute_user_accrued_reward(&state, &mut user_info);
        }
    }

    Ok(UserInfoResponse {
        astro_delegated: user_info.astro_delegated,
        ust_deposited: user_info.ust_deposited,
        lp_shares: user_info.lp_shares,
        claimed_lp_shares: user_info.claimed_lp_shares,
        claimable_lp_shares,
        total_auction_incentives: user_info.total_auction_incentives,
        claimed_auction_incentives: user_info.claimed_auction_incentives,
        claimable_auction_incentives: claimable_auction_reward,
        user_reward_index: user_info.user_reward_index,
        claimable_staking_incentives: claimable_staking_reward,
    })
}

//----------------------------------------------------------------------------------------
// HELPERS
//----------------------------------------------------------------------------------------

/// Calculates ASTRO rewards for participation in the auction for a user
fn calculate_auction_reward_for_user(
    state: &State,
    user_info: &UserInfo,
    astro_rewards_alloc: Uint256,
) -> Uint256 {
    // In-case ASTRO incentives for participation in the auction are already claimed or total ASTRO delegated / UST deposited is currently 0
    if user_info.total_auction_incentives > Uint256::zero()
        || state.total_astro_delegated == Uint256::zero()
        || state.total_ust_deposited == Uint256::zero()
    {
        return Uint256::zero();
    }

    let astro_rewards_alloc_half = astro_rewards_alloc.div(Decimal256::from_ratio(2u32, 1u32));
    let mut total_astro_rewards = Uint256::zero();

    // Calculate rewards for ASTRO Allocation by user
    if user_info.astro_delegated > Uint256::zero() {
        total_astro_rewards += astro_rewards_alloc_half
            * Decimal256::from_ratio(user_info.astro_delegated, state.total_astro_delegated);
    }
    // Calculate rewards for UST provided by user
    if user_info.ust_deposited > Uint256::zero() {
        total_astro_rewards += astro_rewards_alloc_half
            * Decimal256::from_ratio(user_info.ust_deposited, state.total_ust_deposited);
    }
    total_astro_rewards
}

// Accrue ASTRO rewards by updating the reward index
fn update_astro_rewards_index(state: &mut State, astro_accured: Uint256) {
    if !state.are_staked {
        return;
    }
    let astro_rewards_index_increment =
        Decimal256::from_ratio(astro_accured, state.lp_shares_minted);
    state.global_reward_index += astro_rewards_index_increment;
}

// Accrue ASTRO reward for the user by updating the user reward index and adding rewards to the pending rewards
fn compute_user_accrued_reward(state: &State, user_info: &mut UserInfo) -> Uint256 {
    if !state.are_staked {
        return Uint256::zero();
    }
    let pending_user_rewards = (user_info.lp_shares * state.global_reward_index)
        - (user_info.lp_shares * user_info.user_reward_index);
    user_info.user_reward_index = state.global_reward_index;
    pending_user_rewards
}

/// dev Helper function. Returns true if the deposit & withdrawal windows are closed, else reuturns false
fn are_windows_closed(current_timestamp: u64, config: &Config) -> bool {
    let opened_till = config.init_timestamp + config.deposit_window + config.withdrawal_window;
    (current_timestamp > opened_till) || (current_timestamp < config.init_timestamp)
}

/// true if deposits are allowed
fn is_deposit_open(current_timestamp: u64, config: &Config) -> bool {
    let deposits_opened_till = config.init_timestamp + config.deposit_window;
    (config.init_timestamp <= current_timestamp) && (current_timestamp <= deposits_opened_till)
}

///  @dev Helper function to calculate maximum % of their total UST deposited that can be withdrawn
/// Returns % UST that can be withdrawn and 'more_withdrawals_allowed' boolean which indicates whether more withdrawls by the user
/// will be allowed or not
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

/// Returns LP Balance  that a user can withdraw based on a vesting schedule
pub fn calculate_withdrawable_lp_shares(
    cur_timestamp: u64,
    config: &Config,
    state: &State,
    user_info: &UserInfo,
) -> Uint256 {
    let time_elapsed = cur_timestamp - state.pool_init_timestamp;
    if time_elapsed >= config.lp_tokens_vesting_schedule {
        return user_info.lp_shares - user_info.claimed_lp_shares;
    }

    let withdrawable_lp_balance = user_info.lp_shares
        * Decimal256::from_ratio(time_elapsed, config.lp_tokens_vesting_schedule);
    withdrawable_lp_balance - user_info.claimed_lp_shares
}

/// Returns ASTRO auction incentives that a user can withdraw based on a vesting schedule
pub fn calculate_claimable_auction_reward_for_user(
    cur_timestamp: u64,
    config: &Config,
    state: &State,
    user_info: &UserInfo,
) -> Uint256 {
    if user_info.claimed_auction_incentives == user_info.total_auction_incentives
        || state.pool_init_timestamp == 0u64
    {
        return Uint256::zero();
    }
    let time_elapsed = cur_timestamp - state.pool_init_timestamp;
    if time_elapsed >= config.astro_vesting_schedule {
        return user_info.total_auction_incentives - user_info.claimed_auction_incentives;
    }
    let withdrawable_auction_incentives = user_info.total_auction_incentives
        * Decimal256::from_ratio(time_elapsed, config.astro_vesting_schedule);
    withdrawable_auction_incentives - user_info.claimed_auction_incentives
}

/// Returns CosmosMsg struct to stake LP Tokens with the Generator contract
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

/// Returns CosmosMsg struct to withdraw staked LP Tokens from the Generator contract
pub fn build_unstake_from_generator_msg(
    config: &Config,
    lp_shares_to_withdraw: Uint256,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.generator_contract.to_string(),
        msg: to_binary(&astroport::generator::ExecuteMsg::Withdraw {
            lp_token: config.lp_token_address.clone(),
            amount: lp_shares_to_withdraw.into(),
        })?,
        funds: vec![],
    }))
}

/// @dev Queries pending rewards to be claimed from the generator contract for the 'contract_addr'
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

/// @dev Helper function. Returns CosmosMsg struct to facilitate liquidity provision to the Astroport LP Pool
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
        amount: state.total_astro_delegated.into(),
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
        })?,
    }))
}

fn build_activate_claims_lockdrop_msg(lockdrop_contract_address: Addr) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lockdrop_contract_address.to_string(),
        msg: to_binary(&LockdropEnableClaims {})?,
        funds: vec![],
    }))
}

fn build_activate_claims_airdrop_msg(airdrop_contract_address: Addr) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: airdrop_contract_address.to_string(),
        msg: to_binary(&AirdropEnableClaims {})?,
        funds: vec![],
    }))
}

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

//----------------------------------------------------------------------------------------
// Helper functions
//----------------------------------------------------------------------------------------

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
//             amount:  Uint256::from(1000_u64)
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
//                         amount: Uint256::from(1000_u64),
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
//                                             claim_amount : Uint256::from(250000000_u64),
//                                             merkle_proof : vec!["7719b79a65e5aa0bbfd144cf5373138402ab1c374d9049e490b5b61c23d90065".to_string(),
//                                                                 "60368f2058e0fb961a7721a241f9b973c3dd6c57e10a627071cd81abca6aa490".to_string()],
//                                             root_index : 0
//                                         };
//         let mut claim_msg_wrong_amount = ClaimByTerraUser {
//                                             claim_amount : Uint256::from(210000000_u64),
//                                             merkle_proof : vec!["7719b79a65e5aa0bbfd144cf5373138402ab1c374d9049e490b5b61c23d90065".to_string(),
//                                                                 "60368f2058e0fb961a7721a241f9b973c3dd6c57e10a627071cd81abca6aa490".to_string()],
//                                             root_index : 0
//                                         };
//         let mut claim_msg_incorrect_proof = ClaimByTerraUser {
//                                                         claim_amount : Uint256::from(250000000_u64),
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
//                         amount: Uint256::from(250000000_u64),
//                     }).unwrap(),
//             }))]
//         );

//         is_claimed = check_user_claimed(deps.as_ref(), "terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp".to_string() ).unwrap();
//         assert_eq!(is_claimed.is_claimed, true );

//         // ** "Already claimed" Error should be returned **
//         claim_f = execute(deps.as_mut(), env.clone(), user_info_1.clone(), claim_msg.clone() );
//         assert_generic_error_message(claim_f,"Already claimed");

//         claim_msg = ClaimByTerraUser {
//                                             claim_amount : Uint256::from(1 as u64),
//                                             merkle_proof : vec!["7fd0f6ac4074cef9f89eedcf72459ad7b0891855f8084b54dc7de7569849d1c8".to_string(),
//                                                                 "4fab6b0ef8d988835ad968d03d61de408772d033e9ce734394bb623309c5d7fc".to_string()],
//                                             root_index : 0
//                                         };
//         claim_msg_wrong_amount = ClaimByTerraUser {
//                                             claim_amount : Uint256::from(2 as u64),
//                                             merkle_proof : vec!["7fd0f6ac4074cef9f89eedcf72459ad7b0891855f8084b54dc7de7569849d1c8".to_string(),
//                                                                 "4fab6b0ef8d988835ad968d03d61de408772d033e9ce734394bb623309c5d7fc".to_string()],
//                                             root_index : 0
//                                         };
//         claim_msg_incorrect_proof = ClaimByTerraUser {
//                                             claim_amount : Uint256::from(1 as u64),
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
//                         amount: Uint256::from(1 as u64),
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
//                                             claim_amount : Uint256::from(user_info_claim_amount as u64),
//                                             merkle_proof : vec!["0a3419fc5fa4cb0ecb878dc3aaf01fa00782e5d79b02fbb4097dc8df8f191c60".to_string(),
//                                                                 "45cc757ac5eda8bcd1a45a7bd2cb23f4af5147683f120fa287b99617834b83aa".to_string()],
//                                             root_index : 0,
//                                             signature : user_info_signature.to_string(),
//                                             signed_msg_hash : user_info_signed_msg_hash.to_string()
//                                         };
//         let claim_msg_wrong_amount = ClaimByEvmUser {
//                                             eth_address : user_info_evm_address.to_string() ,
//                                             claim_amount : Uint256::from(150000000_u64),
//                                             merkle_proof : vec!["0a3419fc5fa4cb0ecb878dc3aaf01fa00782e5d79b02fbb4097dc8df8f191c60".to_string(),
//                                                                 "45cc757ac5eda8bcd1a45a7bd2cb23f4af5147683f120fa287b99617834b83aa".to_string()],
//                                             root_index : 0,
//                                             signature : user_info_signature.to_string(),
//                                             signed_msg_hash : user_info_signed_msg_hash.to_string()
//                                         };
//         let claim_msg_incorrect_proof = ClaimByEvmUser {
//                                             eth_address : user_info_evm_address.to_string() ,
//                                             claim_amount : Uint256::from(user_info_claim_amount as u64),
//                                             merkle_proof : vec!["0b3419fc5fa4cb0ecb878dc3aaf01fa00782e5d79b02fbb4097dc8df8f191c60".to_string(),
//                                                                 "45cc757ac5eda8bcd1a45a7bd2cb23f4af5147683f120fa287b99617834b83aa".to_string()],
//                                             root_index : 0,
//                                             signature : user_info_signature.to_string(),
//                                             signed_msg_hash : user_info_signed_msg_hash.to_string()
//                                         };
//         let claim_msg_incorrect_msg_hash = ClaimByEvmUser {
//                                             eth_address : user_info_evm_address.to_string() ,
//                                             claim_amount : Uint256::from(user_info_claim_amount as u64),
//                                             merkle_proof : vec!["0a3419fc5fa4cb0ecb878dc3aaf01fa00782e5d79b02fbb4097dc8df8f191c60".to_string(),
//                                                                 "45cc757ac5eda8bcd1a45a7bd2cb23f4af5147683f120fa287b99617834b83aa".to_string()],
//                                             root_index : 0,
//                                             signature : user_info_signature.to_string(),
//                                             signed_msg_hash : "11f879f53729f18888d74aa10ea7737d629e36a1675bce35e1fb1be9065501df".to_string()
//                                         };
//         let claim_msg_incorrect_signature = ClaimByEvmUser {
//                                             eth_address : user_info_evm_address.to_string() ,
//                                             claim_amount : Uint256::from(user_info_claim_amount as u64),
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
//                         amount: Uint256::from(user_info_claim_amount as u64),
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
