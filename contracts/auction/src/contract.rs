use std::ops::Div;

use cosmwasm_bignumber::Decimal256;
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

use astroport::asset::{Asset, AssetInfo, PairInfo};
use astroport::generator::{PendingTokenResponse, QueryMsg as GenQueryMsg};
use astroport::pair::QueryMsg as AstroportPairQueryMsg;

use crate::state::{Config, State, UserInfo, CONFIG, STATE, USERS};
use astroport::querier::query_token_balance;
use cw20::{Cw20QueryMsg, Cw20ReceiveMsg};

const UUSD_DENOM: &str = "uusd";

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
    let pair_info: PairInfo = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: msg.astro_ust_pair_address,
        msg: to_binary(&AstroportPairQueryMsg::Pair {})?,
    }))?;

    let config = Config {
        owner: deps.api.addr_validate(&msg.owner)?,
        astro_token_address: deps.api.addr_validate(&msg.astro_token_address)?,
        airdrop_contract_address: deps.api.addr_validate(&msg.airdrop_contract_address)?,
        lockdrop_contract_address: deps.api.addr_validate(&msg.lockdrop_contract_address)?,
        astro_ust_pool_address: deps.api.addr_validate(&msg.astro_ust_pair_address)?,
        astro_ust_lp_token_address: pair_info.liquidity_token,
        generator_contract: deps.api.addr_validate(&msg.generator_contract_address)?,
        astro_rewards: msg.astro_rewards,
        astro_vesting_duration: msg.astro_vesting_duration,
        lp_tokens_vesting_duration: msg.lp_tokens_vesting_duration,
        init_timestamp: msg.init_timestamp,
        deposit_window: msg.deposit_window,
        withdrawal_window: msg.withdrawal_window,
    };

    CONFIG.save(deps.storage, &config)?;
    STATE.save(deps.storage, &State::default())?;

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
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),

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
    if cw20_msg.sender != config.airdrop_contract_address
        && cw20_msg.sender != config.lockdrop_contract_address
    {
        return Err(StdError::generic_err("Unauthorized"));
    }

    // CHECK ::: Amount needs to be valid
    if cw20_msg.amount.is_zero() {
        return Err(StdError::generic_err("Amount must be greater than 0"));
    }

    match from_binary(&cw20_msg.msg)? {
        Cw20HookMsg::DelegateAstroTokens { user_address } => {
            handle_delegate_astro_tokens(deps, env, user_address, cw20_msg.amount.into())
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
    if let Some(owner) = new_config.owner {
        config.owner = deps.api.addr_validate(&owner)?;
    }

    if let Some(generator_contract) = new_config.generator_contract {
        config.boostrap_auction_address = deps.api.addr_validate(&generator_contract)?;
    }

    if let Some(astro_rewards) = new_config.astro_rewards {
        config.astro_rewards = astro_rewards;
    }

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "Auction::ExecuteMsg::UpdateConfig"))
}

/// @dev Delegates ASTRO tokens to be used for the LP Bootstrapping via auction. Callable only by Airdrop / Lockdrop contracts
/// @param user_address : User address who is delegating the ASTRO tokens for LP Pool bootstrap via auction
/// @param amount : Number of ASTRO Tokens being delegated
pub fn handle_delegate_astro_tokens(
    deps: DepsMut,
    env: Env,
    user_address: Addr,
    amount: Uint128,
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;

    // CHECK :: Lockdrop deposit window open
    if !is_deposit_open(env.block.time.seconds(), &config) {
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
    env: Env,
    info: MessageInfo,
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;

    // CHECK :: Lockdrop deposit window open
    if !is_deposit_open(env.block.time.seconds(), &config) {
        return Err(StdError::generic_err("Deposit window closed"));
    }

    let mut state = STATE.load(deps.storage)?;
    let mut user_info = USERS
        .may_load(deps.storage, &info.sender)?
        .unwrap_or_default();

    // Retrieve UST sent by the user
    if info.funds.len() > 1 {
        return Err(StdError::generic_err("Trying to deposit several coins"));
    }

    let native_token = info.funds.first().unwrap();
    if native_token.denom != String::from(UUSD_DENOM) {
        return Err(StdError::generic_err("Invalid native token denom"));
    }

    // UPDATE STATE
    state.total_ust_delegated += native_token.amount;
    user_info.ust_deposited += native_token.amount;

    // SAVE UPDATED STATE
    STATE.save(deps.storage, &state)?;
    USERS.save(deps.storage, &depositor_address, &user_info)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "Auction::ExecuteMsg::DepositUst"),
        attr("user", info.sender.to_string()),
        attr("ust_deposited", native_token.amount),
    ]))
}

/// true if deposits are allowed
fn is_deposit_open(current_timestamp: u64, config: &Config) -> bool {
    current_timestamp >= config.init_timestamp
        && current_timestamp <= config.init_timestamp + config.deposit_window
}

/// @dev Facilitates UST withdrawals by users from their deposit positions
/// @param amount : UST amount being withdrawn
pub fn handle_withdraw_ust(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    let user_address = info.sender;

    let mut user_info = USERS
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    // CHECK :: Has the user already withdrawn during the current window
    if user_info.ust_withdrawn {
        return Err(StdError::generic_err("Max 1 withdrawal allowed"));
    }

    // Check :: Amount should be within the allowed withdrawal limit bounds
    let max_withdrawal_percent = allowed_withdrawal_percent(env.block.time.seconds(), &config);
    // TODO: check these two functions calculation and logic behind it
    let max_withdrawal_allowed = user_info.ust_deposited * max_withdrawal_percent;

    if amount > max_withdrawal_allowed {
        return Err(StdError::generic_err(
            "Amount exceeds maximum allowed withdrawal limit",
        ));
    }

    // TODO: ????? what is going on here
    // Set user's withdrawl_counter to true incase no further withdrawals are allowed for the user
    if max_withdrawal_percent <= Decimal256::from_ratio(50u32, 100u32) {
        user_info.ust_withdrawn = true;
    }

    // UPDATE STATE
    state.total_ust_delegated = state.total_ust_delegated - amount;
    user_info.ust_deposited = user_info.ust_deposited - amount; // TODO: calculation on line 300 is counting on this value

    // SAVE UPDATED STATE
    STATE.save(deps.storage, &state)?;
    USERS.save(deps.storage, &user_address, &user_info)?;

    // Transfer UST to the user
    let transfer_ust = Asset {
        amount: amount.clone(),
        info: AssetInfo::NativeToken {
            denom: String::from(UUSD_DENOM),
        },
    }
    .into_msg(&deps.querier, user_address)?;

    Ok(Response::new()
        .add_message(transfer_ust)
        .add_attributes(vec![
            attr("action", "Auction::ExecuteMsg::WithdrawUst"),
            attr("user", user_address.to_string()),
            attr("ust_withdrawn", amount),
        ]))
}

///  @dev Helper function to calculate maximum % of their total UST deposited that can be withdrawn
/// Returns % UST that can be withdrawn and 'more_withdrawals_allowed' boolean which indicates whether more withdrawls by the user
/// will be allowed or not
fn allowed_withdrawal_percent(current_timestamp: u64, config: &Config) -> Decimal256 {
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

    Decimal256::from_ratio(0u32, 100u32)
}

/// @dev Admin function to bootstrap the ASTRO-UST Liquidity pool by depositing all ASTRO, UST tokens deposited to the Astroport pool
/// @param slippage Optional, to handle slippage that may be there when adding liquidity to the pool
pub fn handle_add_liquidity_to_astroport_pool(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    slippage: Option<Decimal>,
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // CHECK :: Only admin can call this function
    if info.sender != config.owner {
        return Err(StdError::generic_err("Unauthorized"));
    }

    if state.is_pool_created {
        return Err(StdError::generic_err("Liquidity already provided to pool"));
    }

    // CHECK :: Deposit / withdrawal windows need to be over
    let window_end = config.init_timestamp + config.deposit_window + config.withdrawal_window;
    if env.block.time.seconds() < window_end {
        return Err(StdError::generic_err(
            "Deposit/withdrawal windows are still open",
        ));
    }

    let mut msgs = vec![];

    // QUERY CURRENT LP TOKEN BALANCE (FOR SAFETY - IN ANY CASE)
    let cur_lp_balance = query_token_balance(
        &deps.querier,
        config.lp_token_address.clone(),
        env.contract.address.clone(),
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

    let add_liquidity_msg = build_provide_liquidity_to_lp_pool_msg(
        deps.as_ref(),
        &config,
        state.total_ust_delegated,
        state.total_astro_delegated,
        slippage,
    )?;

    let update_state_msg = CallbackMsg::UpdateStateOnLiquidityAdditionToPool {
        prev_lp_balance: cur_lp_balance.into(),
    }
    .to_cosmos_msg(&_env.contract.address)?;

    msgs.push(approve_astro_msg);
    msgs.push(add_liquidity_msg);
    msgs.push(update_state_msg);

    state.is_pool_created = true;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new().add_messages(msgs).add_attributes(vec![
        attr("action", "Auction::ExecuteMsg::AddLiquidityToAstroportPool"),
        attr("astro_deposited", state.total_astro_delegated),
        attr("ust_deposited", state.total_ust_delegated),
    ]))
}

/// @dev Helper function. Returns CosmosMsg struct to facilitate liquidity provision to the Astroport LP Pool
fn build_provide_liquidity_to_lp_pool_msg(
    deps: Deps,
    config: &Config,
    ust_amount: Uint128,
    astro_amount: Uint128,
    slippage_tolerance: Option<Decimal>,
) -> StdResult<CosmosMsg> {
    let astro = Asset {
        amount: astro_amount,
        info: AssetInfo::Token {
            contract_address: config.astro_token_address.clone(),
        },
    };

    let mut ust = Asset {
        amount: ust_amount,
        info: AssetInfo::NativeToken {
            denom: String::from(UUSD_DENOM),
        },
    };

    // Deduct tax
    ust.amount = ust.amount.checked_sub(ust.compute_tax(&deps.querier)?)?;

    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.astro_ust_pool_address.to_string(),
        funds: vec![Coin {
            denom: String::from(UUSD_DENOM),
            amount: ust.amount,
        }],
        msg: to_binary(&astroport::pair::ExecuteMsg::ProvideLiquidity {
            assets: [ust, astro],
            slippage_tolerance,
        })?,
    }))
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

    // CHECK :: Can be staked only once
    if state.is_lp_staked {
        return Err(StdError::generic_err("Already staked"));
    }

    //COSMOS MSG :: To stake LP Tokens to the Astroport generator contract
    let stake_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.generator_contract.to_string(),
        msg: to_binary(&astroport::generator::ExecuteMsg::Deposit {
            lp_token: config.astro_ust_lp_token_address.clone(),
            amount: state.lp_shares_minted,
        })?,
        funds: vec![],
    });

    state.is_lp_staked = true;
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

//----------------------------------------------------------------------------------------
// Handle::Callback functions
//----------------------------------------------------------------------------------------

// CALLBACK :: CALLED AFTER ASTRO, UST LIQUIDITY IS ADDED TO THE LP POOL
pub fn update_state_on_liquidity_addition_to_pool(
    deps: DepsMut,
    env: Env,
    prev_lp_balance: Uint128,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // QUERY CURRENT LP TOKEN BALANCE :: NEWLY MINTED LP TOKENS
    let cur_lp_balance = cw20_get_balance(
        &deps.querier,
        config.astro_ust_lp_token_address.clone(),
        env.contract.address,
    )?;

    // STATE :: UPDATE --> SAVE
    state.lp_shares_minted = cur_lp_balance - prev_lp_balance; // TODO: this callback can run only once or will break maths
    STATE.save(deps.storage, &state)?;

    // Activate lockdrop and airdrop claims
    let cosmos_msgs = vec![
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.lockdrop_contract_address.to_string(),
            msg: to_binary(&LockdropEnableClaims {})?,
            funds: vec![],
        }),
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.airdrop_contract_address.to_string(),
            msg: to_binary(&AirdropEnableClaims {})?,
            funds: vec![],
        }),
    ];

    Ok(Response::new().add_attributes(vec![
        (
            "action",
            "Auction::CallbackMsg::UpdateStateOnLiquidityAddition",
        ),
        // ("maUST_minted", m_ust_minted.to_string().as_str()),
    ]))
}
