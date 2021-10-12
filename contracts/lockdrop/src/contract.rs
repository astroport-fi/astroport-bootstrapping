use std::vec;

use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::{
    entry_point, from_binary, to_binary, Addr, Binary, Coin, CosmosMsg, Deps, DepsMut, Env,
    MessageInfo, QuerierWrapper, QueryRequest, Response, StdError, StdResult, Uint128, WasmMsg,
    WasmQuery,
};

use astroport_periphery::helpers::{
    build_approve_cw20_msg, build_send_cw20_token_msg, build_transfer_cw20_from_user_msg,
    build_transfer_cw20_token_msg, cw20_get_balance, is_str_present_in_vec, option_string_to_addr,
    zero_address,
};
use astroport_periphery::lockdrop::{
    CallbackMsg, ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, LockUpInfoResponse,
    PoolResponse, QueryMsg, StateResponse, UpdateConfigMsg, UserInfoResponse,
};

use astroport::generator::{PendingTokenResponse, QueryMsg as GenQueryMsg};
use astroport_periphery::asset::{Cw20Asset, LiquidityPool, NativeAsset};
use astroport_periphery::lp_bootstrap_auction::Cw20HookMsg::DelegateAstroTokens;
use astroport_periphery::tax::deduct_tax;
use terraswap::asset::Asset as terraswapAsset;

use crate::state::{
    Config, LockupInfo, PoolInfo, State, UserInfo, ASSET_POOLS, CONFIG, LOCKUP_INFO, STATE,
    USER_INFO,
};
use cw20::Cw20ReceiveMsg;

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

    // CHECK :: min_lock_duration , max_lock_duration need to be valid (min_lock_duration < max_lock_duration)
    if msg.max_duration <= msg.min_duration {
        return Err(StdError::generic_err("Invalid Lockup durations"));
    }

    let config = Config {
        owner: deps.api.addr_validate(&msg.owner)?,
        auction_contract_address: option_string_to_addr(
            deps.api,
            msg.auction_contract_address,
            zero_address(),
        )?,
        generator_address: option_string_to_addr(deps.api, msg.generator_address, zero_address())?,
        astro_token_address: option_string_to_addr(
            deps.api,
            msg.astro_token_address,
            zero_address(),
        )?,
        init_timestamp: msg.init_timestamp,
        deposit_window: msg.deposit_window,
        withdrawal_window: msg.withdrawal_window,
        min_lock_duration: msg.min_duration,
        max_lock_duration: msg.max_duration,
        seconds_per_week: msg.seconds_per_week,
        weekly_multiplier: msg.weekly_multiplier,
        lockdrop_incentives: msg.lockdrop_incentives,
    };

    let state = State {
        total_astro_delegated: Uint256::zero(),
        total_astro_returned_available: Uint256::zero(),
        are_claims_allowed: false,
        supported_pairs_list: vec![],
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
        ExecuteMsg::Receive(msg) => receive_cw20(deps, _env, info, msg),

        ExecuteMsg::UpdateConfig { new_config } => handle_update_config(deps, info, new_config),
        ExecuteMsg::InitializePool {
            terraswap_pool,
            incentives_percent,
        } => handle_initialize_pool(deps, _env, info, terraswap_pool, incentives_percent),
        ExecuteMsg::UpdatePool {
            pool_identifier,
            incentives_percent,
        } => handle_update_pool(deps, info, pool_identifier, incentives_percent),

        ExecuteMsg::MigrateLiquidity {
            pool_identifer,
            astroport_pool_address,
            astroport_lp_address,
        } => handle_migrate_liquidity(
            deps,
            _env,
            info,
            pool_identifer,
            astroport_pool_address,
            astroport_lp_address,
        ),

        ExecuteMsg::StakeLpTokens { pool_identifer } => {
            handle_stake_lp_tokens(deps, _env, info, pool_identifer)
        }
        ExecuteMsg::EnableClaims {} => handle_enable_claims(deps, info),
        ExecuteMsg::TransferReturnedAstro { recepient, amount } => {
            handle_tranfer_returned_astro(deps, info, recepient, amount)
        }

        ExecuteMsg::DelegateAstroToAuction { amount } => {
            handle_delegate_astro_to_auction(deps, _env, info, amount)
        }
        ExecuteMsg::WithdrawFromLockup {
            pool_identifer,
            duration,
            amount,
        } => handle_withdraw_from_lockup(deps, _env, info, pool_identifer, duration, amount),
        ExecuteMsg::ClaimRewardsForLockup {
            pool_identifer,
            duration,
        } => handle_claim_rewards_for_lockup(deps, _env, info, pool_identifer, duration),
        ExecuteMsg::UnlockPosition {
            pool_identifer,
            duration,
        } => handle_unlock_position(deps, _env, info, pool_identifer, duration),
        ExecuteMsg::ForceUnlockPosition {
            pool_identifer,
            duration,
        } => handle_force_unlock_position(deps, _env, info, pool_identifer, duration),

        ExecuteMsg::Callback(msg) => _handle_callback(deps, _env, info, msg),
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, StdError> {
    let user_address_ = deps.api.addr_validate(&cw20_msg.sender)?;
    let amount = cw20_msg.amount;

    match from_binary(&cw20_msg.msg)? {
        Cw20HookMsg::IncreaseLockup { duration } => {
            handle_increase_lockup(deps, env, info, user_address_, duration, amount.into())
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
        CallbackMsg::UpdatePoolOnDualRewardsClaim {
            pool_identifer,
            prev_astro_balance,
            prev_dual_reward_balance,
        } => update_pool_on_dual_rewards_claim(
            deps,
            env,
            pool_identifer,
            prev_astro_balance,
            prev_dual_reward_balance,
        ),
        CallbackMsg::WithdrawUserLockupRewardsCallback {
            user_address,
            pool_identifer,
            duration,
            withdraw_lp_stake,
            force_unlock,
        } => callback_withdraw_user_rewards_for_lockup_optional_withdraw(
            deps,
            env,
            user_address,
            pool_identifer,
            duration,
            withdraw_lp_stake,
            force_unlock,
        ),
        CallbackMsg::WithdrawLiquidityFromTerraswapCallback {
            pool_identifer,
            native_asset,
            native_asset_balance,
            cw20_asset,
            cw20_asset_balance,
            astroport_pool,
        } => callback_deposit_liquidity_in_astroport(
            deps,
            env,
            info,
            pool_identifer,
            native_asset,
            native_asset_balance,
            cw20_asset,
            cw20_asset_balance,
            astroport_pool,
        ),
        CallbackMsg::UpdateStateLiquidityMigrationCallback {
            pool_identifer,
            astroport_pool,
            astroport_lp_balance,
        } => callback_update_pool_state_after_migration(
            deps,
            env,
            info,
            pool_identifer,
            astroport_pool,
            astroport_lp_balance,
        ),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State {} => to_binary(&query_state(deps)?),
        QueryMsg::Pool { pool_identifier } => to_binary(&query_pool(deps, pool_identifier)?),
        QueryMsg::UserInfo { address } => to_binary(&query_user_info(deps, _env, address)?),
        QueryMsg::LockUpInfo {
            user_address,
            pool_identifier,
            duration,
        } => to_binary(&query_lockup_info(
            deps,
            user_address,
            pool_identifier,
            duration,
        )?),
        QueryMsg::LockUpInfoWithId { lockup_id } => {
            to_binary(&query_lockup_info_with_id(deps, lockup_id)?)
        }
    }
}

//----------------------------------------------------------------------------------------
// Handle Functions
//----------------------------------------------------------------------------------------

/// @dev Admin function to update Configuration parameters
/// @param new_config : Same as UpdateConfigMsg struct
pub fn handle_update_config(
    deps: DepsMut,
    info: MessageInfo,
    new_config: UpdateConfigMsg,
) -> StdResult<Response> {
    let mut config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;

    // CHECK :: Only owner can call this function
    if info.sender != config.owner {
        return Err(StdError::generic_err("Unauthorized"));
    }

    // CHECK ::: Configuration can only be updated before claims are enabled
    if state.are_claims_allowed {
        return Err(StdError::generic_err(
            "ASTRO tokens are live. Incentives % cannot be updated now",
        ));
    }

    // UPDATE :: ADDRESSES IF PROVIDED
    config.owner = option_string_to_addr(deps.api, new_config.owner, config.owner)?;
    config.auction_contract_address = option_string_to_addr(
        deps.api,
        new_config.auction_contract_address,
        config.auction_contract_address,
    )?;
    config.generator_address = option_string_to_addr(
        deps.api,
        new_config.generator_address,
        config.generator_address,
    )?;
    config.astro_token_address = option_string_to_addr(
        deps.api,
        new_config.astro_token_address,
        config.astro_token_address,
    )?;

    // UPDATE :: LOCKDROP INCENTIVES IF PROVIDED
    config.lockdrop_incentives = new_config
        .lockdrop_incentives
        .unwrap_or(config.lockdrop_incentives);

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "lockdrop::ExecuteMsg::UpdateConfig"))
}

/// @dev Admin function to initialize new LP Pool
/// @param terraswap_pool : LiquidityPool struct providing config info regarding the terraswap pool (Lp token addres, pair address, amount)
/// @param incentives_percent : Optional parameter defining how much % of total ASTRO incentives are allocated for this pool
pub fn handle_initialize_pool(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    terraswap_pool: LiquidityPool,
    incentives_percent: Option<Decimal256>,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // CHECK ::: Only owner can call this function
    if info.sender != config.owner {
        return Err(StdError::generic_err("Unauthorized"));
    }

    // CHECK :: Is lockdrop deposit window open
    if !is_deposit_open(env.block.time.seconds(), &config) {
        return Err(StdError::generic_err(
            "Pools cannot be added post deposit window closure",
        ));
    }

    // CHECK ::: Is LP Token Pool already initialized
    if is_str_present_in_vec(
        state.supported_pairs_list.clone(),
        terraswap_pool.lp_token_addr.clone().to_string(),
    ) {
        return Err(StdError::generic_err("Already supported"));
    }

    // POOL INFO :: Initialize new pool
    let mut pool_info = ASSET_POOLS
        .may_load(
            deps.storage,
            &terraswap_pool.lp_token_addr.clone().as_bytes(),
        )?
        .unwrap_or_default();

    pool_info.terraswap_pair = terraswap_pool.clone();
    pool_info.incentives_percent = incentives_percent.unwrap_or(Decimal256::zero());

    // QUERY :: Query terraswap pair to to fetch pool's trading Asset Pairs
    let pool_assets =
        query_terraswap_pair_assets(&deps.querier, terraswap_pool.pair_addr.clone().to_string())?;

    // Update PoolInfo with the pool assets
    for asset_info in pool_assets {
        match asset_info.info {
            terraswap::asset::AssetInfo::NativeToken { denom } => {
                pool_info.native_asset.denom = denom;
            }
            terraswap::asset::AssetInfo::Token { contract_addr } => {
                pool_info.cw20_asset.contract_addr = contract_addr;
            }
        }
    }

    // STATE UPDATE :: Save state and PoolInfo
    state
        .supported_pairs_list
        .push(terraswap_pool.lp_token_addr.clone().to_string());
    ASSET_POOLS.save(
        deps.storage,
        &terraswap_pool.lp_token_addr.as_bytes(),
        &pool_info,
    )?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new().add_attributes(vec![
        ("action", "lockdrop::ExecuteMsg::InitializePool"),
        ("pool_identifer", &terraswap_pool.lp_token_addr.to_string()),
        ("pool_addr", &terraswap_pool.pair_addr.to_string()),
        (
            "cw20_asset_addr",
            pool_info.cw20_asset.contract_addr.to_string().as_str(),
        ),
        ("denom", pool_info.native_asset.denom.as_str()),
    ]))
}

/// @dev Admin function to update LP Pool Configuration
/// @param pool_identifier : Parameter to identify the pool. Equals pool's terraswap Lp token address
/// @param incentives_percent : Decimal value defining how much % of total ASTRO incentives are allocated for this pool
pub fn handle_update_pool(
    deps: DepsMut,
    info: MessageInfo,
    pool_identifier: String,
    incentives_percent: Decimal256,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;

    // CHECK ::: Only owner can call this function
    if info.sender != config.owner {
        return Err(StdError::generic_err("Unauthorized"));
    }

    // CHECK ::: Pool configuration etc can only be updated before claims are enabled
    if state.are_claims_allowed {
        return Err(StdError::generic_err(
            "ASTRO tokens are live. Incentives % cannot be updated now",
        ));
    }

    // CHECK ::: Is LP Token Pool  initialized
    if !is_str_present_in_vec(state.supported_pairs_list.clone(), pool_identifier.clone()) {
        return Err(StdError::generic_err("Pool not supported"));
    }

    let mut total_incentives = Decimal256::from_ratio(0u64, 100u64);
    for pool_identifier_ in state.supported_pairs_list {
        if pool_identifier_ == pool_identifier {
            total_incentives = total_incentives + incentives_percent;
        } else {
            let cur_pool_info =
                ASSET_POOLS.load(deps.storage, &pool_identifier_.clone().as_bytes())?;
            total_incentives = total_incentives + cur_pool_info.incentives_percent;
        }
    }

    // CHECK ::: total_incentives % cannot exceed 100
    if total_incentives > Decimal256::from_ratio(1u64, 1u64) {
        return Err(StdError::generic_err(
            "Total Incentives % cannot exceed 100",
        ));
    }

    // Update Pool Incentives
    let mut pool_info = ASSET_POOLS.load(deps.storage, &pool_identifier.clone().as_bytes())?;
    pool_info.incentives_percent = incentives_percent;
    ASSET_POOLS.save(deps.storage, &pool_identifier.as_bytes(), &pool_info)?;

    Ok(Response::new().add_attributes(vec![
        ("action", "lockdrop::ExecuteMsg::UpdatePool"),
        ("pool_identifer", &pool_identifier.to_string()),
        ("incentives_percent", &incentives_percent.to_string()),
    ]))
}

/// @dev Admin function to facilitate ASTRO tokens transfer which were returned by the users to forcefully unlock their positions
/// @param recepient : Addresses to transfer ASTRO tokens to
/// @param amount : Number of ASTRO tokens to transfer
pub fn handle_tranfer_returned_astro(
    deps: DepsMut,
    info: MessageInfo,
    recepient: String,
    amount: Uint256,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // CHECK ::: Only owner can call this function
    if info.sender != config.owner {
        return Err(StdError::generic_err("Unauthorized"));
    }

    // CHECK ::: Amount needs to be less than returned ASTRO balance available with the contract
    if state.total_astro_returned_available > amount {
        return Err(StdError::generic_err(format!(
            "Amount needs to be less than {}, which is the current returned ASTRO balance available with the contract",
            state.total_astro_returned_available
        )));
    }

    // COSMOS_MSG ::TRANSFER ASTRO Tokens
    let send_cw20_msg = build_transfer_cw20_token_msg(
        deps.api.addr_validate(&recepient.clone())?,
        config.astro_token_address.to_string(),
        amount.into(),
    )?;

    // Update State
    state.total_astro_returned_available = state.total_astro_returned_available - amount;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_message(send_cw20_msg)
        .add_attributes(vec![
            ("action", "lockdrop::ExecuteMsg::TransferReturnedAstro"),
            ("recepient", &recepient),
            ("amount", &amount.to_string()),
        ]))
}

/// @dev Admin function to enable ASTRO Claims by users. Called along-with Bootstrap Auction contract's LP Pool provide liquidity tx
pub fn handle_enable_claims(deps: DepsMut, info: MessageInfo) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // CHECK :: ONLY AUCTION CONTRACT CAN CALL THIS FUNCTION
    if info.sender != config.auction_contract_address {
        return Err(StdError::generic_err("Unauthorized"));
    }

    // CHECK ::: Claims are only enabled once
    if state.are_claims_allowed {
        return Err(StdError::generic_err("Already allowed"));
    }
    state.are_claims_allowed = true;

    STATE.save(deps.storage, &state)?;
    Ok(Response::new().add_attribute("action", "Lockdrop::ExecuteMsg::EnableClaims"))
}

/// @dev Admin function to migrate Liquidity from Terraswap to Astroport
/// @param pool_identifer : Parameter to identify the pool. Equals pool's terraswap Lp token address
/// @param astroport_pool_address : Astroport Pool address to which the liquidity is to be migrated
/// @param astroport_lp_address : Astroport Pool LP Token address which will be minted upon liquidity migration to the Astroport pool
pub fn handle_migrate_liquidity(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    pool_identifer: String,
    astroport_pool_address: String,
    astroport_lp_address: String,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;

    // Only owner can call this function
    if info.sender != config.owner {
        return Err(StdError::generic_err("Unauthorized"));
    }

    // CHECK :: has the liquidity already been migrated or not ?
    if env.block.time.seconds()
        < (config.init_timestamp + config.deposit_window + config.withdrawal_window)
    {
        return Err(StdError::generic_err(
            "Deposit / Withdrawal windows not closed",
        ));
    }

    // CHECK ::: Pool Liquidity needs to be migrated before claims are enabled
    if state.are_claims_allowed {
        return Err(StdError::generic_err(
            "ASTRO-UST pair live. Liquidity migration window closed",
        ));
    }

    let pool_info = ASSET_POOLS.load(deps.storage, &pool_identifer.clone().as_bytes())?;

    // CHECK :: has the liquidity already been migrated or not ?
    if pool_info.is_migrated {
        return Err(StdError::generic_err("Liquidity already migrated"));
    }

    // COSMOS MSG :: WITHDRAW LIQUIDITY FROM TERRASWAP
    let withdraw_msg_ = to_binary(&terraswap::pair::Cw20HookMsg::WithdrawLiquidity {})?;
    let withdraw_liquidity_msg = build_send_cw20_token_msg(
        pool_info.terraswap_pair.pair_addr.to_string(),
        pool_info.terraswap_pair.lp_token_addr.to_string(),
        pool_info.terraswap_pair.amount.into(),
        withdraw_msg_,
    )?;

    // QUERY :: Get current Asset and uusd balances, passed to callback function to be subtracted from asset balances post liquidity withdrawal from terraswap
    let asset_balance = cw20_get_balance(
        &deps.querier,
        deps.api
            .addr_validate(&pool_info.cw20_asset.contract_addr)?,
        env.contract.address.clone(),
    )?;
    let native_balance_response = deps.querier.query_balance(
        env.contract.address.clone(),
        pool_info.native_asset.denom.to_string(),
    )?;

    // COSMOS MSG :: CALLBACK AFTER LIQUIDITY WITHDRAWAL
    let update_state_msg = CallbackMsg::WithdrawLiquidityFromTerraswapCallback {
        pool_identifer: pool_identifer.clone(),
        native_asset: NativeAsset {
            denom: pool_info.native_asset.denom.to_string(),
        },
        native_asset_balance: native_balance_response.amount,
        cw20_asset: Cw20Asset {
            contract_addr: pool_info.cw20_asset.contract_addr.to_string(),
        },
        cw20_asset_balance: asset_balance,
        astroport_pool: LiquidityPool {
            lp_token_addr: deps.api.addr_validate(&astroport_lp_address)?,
            pair_addr: deps.api.addr_validate(&astroport_pool_address)?,
            amount: Uint256::zero(),
        },
    }
    .to_cosmos_msg(&env.contract.address)?;

    Ok(Response::new()
        .add_messages([withdraw_liquidity_msg, update_state_msg])
        .add_attributes(vec![
            ("action", "lockdrop::ExecuteMsg::MigrateLiquidity"),
            ("pool_identifer", &pool_identifer),
        ]))
}

/// @dev Function to stake one of the supported LP Tokens with the Generator contract
/// @params pool_identifer : Pool's terraswap LP token address whose Astroport LP tokens are to be staked
pub fn handle_stake_lp_tokens(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    pool_identifer: String,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;

    // CHECK ::: Only owner can call this function
    if info.sender != config.owner {
        return Err(StdError::generic_err("Unauthorized"));
    }

    // CHECK ::: Is LP Token Pool supported or not ?
    if is_str_present_in_vec(
        state.supported_pairs_list.clone(),
        pool_identifer.clone().to_string(),
    ) {
        return Err(StdError::generic_err("Already supported"));
    }
    let mut pool_info = ASSET_POOLS.load(deps.storage, &pool_identifer.clone().as_bytes())?;

    // CHECK :: Liquidity needs to be migrated to astroport before staking tokens with the generator contract
    if !pool_info.is_migrated {
        return Err(StdError::generic_err(
            "Only Astroport LP Tokens can be staked with generator",
        ));
    }

    //  COSMOSMSG :: If LP Tokens are migrated, used LP after migration balance else use LP before migration balance
    let stake_lp_msg = build_stake_with_generator_msg(
        config.generator_address.to_string().clone(),
        pool_info.astroport_pair.lp_token_addr.clone(),
        pool_info.astroport_pair.amount.into(),
    )?;

    // UPDATE STATE & SAVE
    pool_info.is_staked = true;
    ASSET_POOLS.save(deps.storage, &pool_identifer.clone().as_bytes(), &pool_info)?;

    Ok(Response::new()
        .add_message(stake_lp_msg)
        .add_attributes(vec![
            ("action", "lockdrop::ExecuteMsg::StakeLPTokens"),
            ("lp_token_addr", &pool_identifer.clone()),
            (
                "staked_amount",
                pool_info.astroport_pair.amount.to_string().as_str(),
            ),
        ]))
}

/// @dev ReceiveCW20 Hook function to increase Lockup position size when any of the supported LP Tokens are sent to the contract by the user
/// @param user_address : User which sent the following LP token
/// @param duration : Number of weeks the LP token is locked for (lockup period begins post the withdrawal window closure)
/// @param amount : Number of LP tokens sent by the user
pub fn handle_increase_lockup(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    user_address: Addr,
    duration: u64,
    amount: Uint256,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;
    let pool_identifer = info.sender.to_string().clone();

    // CHECK ::: LP Token supported or not ?
    if !is_str_present_in_vec(state.supported_pairs_list, pool_identifer.clone()) {
        return Err(StdError::generic_err("Unsupported LP Token"));
    }

    // CHECK :: Lockdrop deposit window open
    if !is_deposit_open(env.block.time.seconds(), &config) {
        return Err(StdError::generic_err("Deposit window closed"));
    }

    // CHECK :: Valid Lockup Duration
    if duration > config.max_lock_duration || duration < config.min_lock_duration {
        return Err(StdError::generic_err(format!(
            "Lockup duration needs to be between {} and {}",
            config.min_lock_duration, config.max_lock_duration
        )));
    }

    // CHECK ::: Amount needs to be valid
    if amount > Uint256::zero() {
        return Err(StdError::generic_err("Amount must be greater than 0"));
    }

    // ASSET POOL :: RETRIEVE --> UPDATE
    let mut pool_info = ASSET_POOLS.load(deps.storage, &pool_identifer.clone().as_bytes())?;
    pool_info.terraswap_pair.amount += amount;
    pool_info.weighted_amount += calculate_weight(amount, duration, config.weekly_multiplier);

    // LOCKUP INFO :: RETRIEVE --> UPDATE
    let lockup_id =
        user_address.to_string().clone() + &pool_identifer.clone() + &duration.to_string();
    let mut lockup_info = LOCKUP_INFO
        .may_load(deps.storage, lockup_id.clone().as_bytes())?
        .unwrap_or_default();
    if lockup_info.lp_units_locked == Uint256::zero() {
        lockup_info.pool_identifier = pool_identifer.clone();
        lockup_info.duration = duration;
        lockup_info.unlock_timestamp = calculate_unlock_timestamp(&config, duration);
    }
    lockup_info.lp_units_locked += amount;

    // USER INFO :: RETRIEVE --> UPDATE
    let mut user_info = USER_INFO
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();
    if !is_str_present_in_vec(user_info.lockup_positions.clone(), lockup_id.clone()) {
        user_info.lockup_positions.push(lockup_id.clone());
    }

    // SAVE UPDATED STATE
    ASSET_POOLS.save(deps.storage, &pool_identifer.as_bytes(), &pool_info)?;
    LOCKUP_INFO.save(deps.storage, lockup_id.clone().as_bytes(), &lockup_info)?;
    USER_INFO.save(deps.storage, &user_address, &user_info)?;

    Ok(Response::new().add_attributes(vec![
        ("action", "lockdrop::ExecuteMsg::IncreaseLockupPosition"),
        ("user", &user_address.to_string()),
        ("lp_token", &pool_identifer),
        ("duration", duration.to_string().as_str()),
        ("amount", amount.to_string().as_str()),
    ]))
}

/// @dev Function to withdraw LP Tokens from an existing Lockup position
/// @param pool_identifer : Pool identifier (Terraswap Lp token address) to identify the LP pool against which withdrawal has to be made
/// @param duration : Duration of the lockup position from which withdrawal is to be made
/// @param amount : Number of LP tokens to be withdrawn
pub fn handle_withdraw_from_lockup(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    pool_identifer: String,
    duration: u64,
    amount: Uint256,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;

    // CHECK :: Valid Withdraw Amount
    if amount == Uint256::zero() {
        return Err(StdError::generic_err("Invalid withdrawal request"));
    }

    // CHECK ::: LP Token supported or not ?
    if !is_str_present_in_vec(state.supported_pairs_list, pool_identifer.clone()) {
        return Err(StdError::generic_err("Unsupported LP Token"));
    }

    // Retrieve Lockup position
    let user_address = info.sender.clone();
    let lockup_id =
        user_address.to_string().clone() + &pool_identifer.clone() + &duration.to_string();
    let mut lockup_info = LOCKUP_INFO
        .may_load(deps.storage, lockup_id.clone().as_bytes())?
        .unwrap_or_default();

    // CHECK :: Has user already withdrawn LP tokens once post the deposit window closure state
    if lockup_info.withdrawal_counter {
        return Err(StdError::generic_err(
            "Maximum Withdrawal limit reached. No more withdrawals accepted",
        ));
    }

    // Check :: Amount should be within the allowed withdrawal limit bounds
    let max_withdrawal_percent =
        calculate_max_withdrawal_percent_allowed(env.block.time.seconds(), &config);
    let max_withdrawal_allowed = lockup_info.lp_units_locked * max_withdrawal_percent;
    if amount > max_withdrawal_allowed {
        return Err(StdError::generic_err(format!(
            "Amount exceeds maximum allowed withdrawal limit of {} ",
            max_withdrawal_allowed
        )));
    }

    // Update withdrawal counter if the max_withdrawal_percent <= 50% ::: as it is being processed post the deposit window closure
    if max_withdrawal_percent <= Decimal256::from_ratio(50u64, 100u64) {
        lockup_info.withdrawal_counter = true;
    }

    let mut pool_info = ASSET_POOLS.load(deps.storage, &pool_identifer.clone().as_bytes())?;

    // STATE :: RETRIEVE --> UPDATE
    lockup_info.lp_units_locked = lockup_info.lp_units_locked - amount;
    pool_info.terraswap_pair.amount = pool_info.terraswap_pair.amount - amount;
    pool_info.weighted_amount =
        pool_info.weighted_amount - calculate_weight(amount, duration, config.weekly_multiplier);

    // Remove Lockup position from the list of user positions if Lp_Locked balance == 0
    if lockup_info.lp_units_locked == Uint256::zero() {
        let mut user_info = USER_INFO.load(deps.storage, &user_address.clone())?;
        remove_lockup_pos_from_user_info(&mut user_info, lockup_id.clone());
        USER_INFO.save(deps.storage, &user_address, &user_info)?;
    }

    // SAVE Updated States
    LOCKUP_INFO.save(deps.storage, lockup_id.clone().as_bytes(), &lockup_info)?;
    ASSET_POOLS.save(deps.storage, &pool_identifer.as_bytes(), &pool_info)?;

    // COSMOS_MSG ::TRANSFER WITHDRAWN LP Tokens
    let send_cw20_msg = build_transfer_cw20_token_msg(
        user_address.clone(),
        pool_info.terraswap_pair.lp_token_addr.to_string(),
        amount.into(),
    )?;

    Ok(Response::new()
        .add_messages(vec![send_cw20_msg])
        .add_attributes(vec![
            ("action", "lockdrop::ExecuteMsg::WithdrawFromLockup"),
            ("user", &user_address.to_string()),
            (
                "lp_token_addr",
                &pool_info.terraswap_pair.lp_token_addr.to_string(),
            ),
            ("duration", duration.to_string().as_str()),
            ("amount", amount.to_string().as_str()),
        ]))
}

// @dev Function to delegate part of the ASTRO rewards to be used for LP Bootstrapping via auction
/// @param amount : Number of ASTRO to delegate
pub fn handle_delegate_astro_to_auction(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint256,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let user_address = info.sender.clone();

    // CHECK :: Have the deposit / withdraw windows concluded
    if env.block.time.seconds()
        < (config.init_timestamp + config.deposit_window + config.withdrawal_window)
    {
        return Err(StdError::generic_err(
            "Deposit / withdraw windows not closed yet",
        ));
    }

    // CHECK :: Can users withdraw their ASTRO tokens ? -> if so, then delegation is no longer allowed
    if state.are_claims_allowed {
        return Err(StdError::generic_err("Delegation window over"));
    }

    let mut user_info = USER_INFO
        .may_load(deps.storage, &user_address.clone())?
        .unwrap_or_default();

    // CHECK :: User needs to have atleast 1 lockup position
    if user_info.lockup_positions.len() == 0 {
        return Err(StdError::generic_err("No valid lockup positions"));
    }

    // If user's total ASTRO rewards == 0 :: We update all of the user's lockup positions to calculate ASTRO rewards and for each alongwith their equivalent Astroport LP Shares
    if user_info.total_astro_rewards == Uint256::zero() {
        user_info.total_astro_rewards = update_user_lockup_positions_calc_rewards_and_migrate(
            deps.branch(),
            &config,
            user_info.clone(),
        )?;
    }

    // CHECK :: ASTRO to delegate cannot exceed user's unclaimed ASTRO balance
    if amount > (user_info.total_astro_rewards - user_info.delegated_astro_rewards) {
        return Err(StdError::generic_err(format!("ASTRO to delegate cannot exceed user's unclaimed ASTRO balance. ASTRO to delegate = {}, Max delegatable ASTRO = {} ",amount, (user_info.total_astro_rewards - user_info.delegated_astro_rewards))));
    }

    // UPDATE STATE
    user_info.delegated_astro_rewards += amount;
    state.total_astro_delegated += amount;

    // SAVE UPDATED STATE
    STATE.save(deps.storage, &state)?;
    USER_INFO.save(deps.storage, &user_address, &user_info)?;

    // COSMOS_MSG ::Delegate ASTRO to the LP Bootstrapping via Auction contract
    let msg_ = to_binary(&DelegateAstroTokens {
        user_address: info.sender.clone(),
    })?;
    let delegate_msg = build_send_cw20_token_msg(
        config.auction_contract_address.to_string(),
        config.astro_token_address.to_string(),
        amount.into(),
        msg_,
    )?;

    Ok(Response::new()
        .add_messages(vec![delegate_msg])
        .add_attributes(vec![
            ("action", "lockdrop::ExecuteMsg::DelegateAstroToAuction"),
            ("user", &user_address.to_string()),
            ("amount", amount.to_string().as_str()),
        ]))
}

// @dev Function to claim user Rewards for a particular Lockup position
// @param pool_identifer : Pool identifier to identify the LP pool whose Token is locked in the lockup position
// @param duration : Lockup duration (number of weeks)
pub fn handle_claim_rewards_for_lockup(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    pool_identifer: String,
    duration: u64,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;
    let user_address = info.sender.clone();

    // CHECK ::: Is LP Token Pool supported or not ?
    if is_str_present_in_vec(
        state.supported_pairs_list.clone(),
        pool_identifer.clone().to_string(),
    ) {
        return Err(StdError::generic_err("Already supported"));
    }

    let pool_info = ASSET_POOLS.load(deps.storage, &pool_identifer.clone().as_bytes())?;

    let mut cosmos_msgs = vec![];

    // QUERY :: Check if there are any unclaimed staking rewards
    let unclaimed_rewards_response = query_unclaimed_staking_rewards_for_pool(
        deps.querier,
        config.generator_address.to_string(),
        pool_info.astroport_pair.lp_token_addr.clone(),
        _env.contract.address.clone(),
    );

    // --> If unclaimed rewards > 0
    if unclaimed_rewards_response.pending > Uint128::zero() {
        // Returns an array with 2 CosmosMsgs
        // 1.  Cosmos Msg to claim rewards from the generator contract
        // 2. Callback Cosmos Msg to update state after rewards are claimed
        let cosmos_msg_array = prepare_cosmos_msgs_to_claim_dual_rewards(
            deps.querier,
            &config,
            pool_identifer.clone(),
            deps.api
                .addr_validate(&pool_info.cw20_asset.contract_addr)?,
            pool_info.astroport_pair.lp_token_addr.clone(),
            _env.contract.address.clone(),
        )?;
        for msg_ in cosmos_msg_array {
            cosmos_msgs.push(msg_);
        }
    }
    // Callback Cosmos Msg :: To withdraw User's lockup rewards [withdraw_lp_stake = false as LP tokens wont be trasferred to the user]
    let withdraw_user_rewards_for_lockup_msg = CallbackMsg::WithdrawUserLockupRewardsCallback {
        user_address: user_address.clone(),
        pool_identifer: pool_identifer.clone(),
        duration: duration,
        withdraw_lp_stake: false,
        force_unlock: false,
    }
    .to_cosmos_msg(&_env.contract.address)?;
    cosmos_msgs.push(withdraw_user_rewards_for_lockup_msg);

    Ok(Response::new()
        .add_messages(cosmos_msgs)
        .add_attributes(vec![
            ("action", "lockdrop::ExecuteMsg::WithdrawUserRewardsForPool"),
            ("pool_identifer", &pool_identifer),
            ("user_address", &user_address.to_string()),
        ]))
}

/// @dev Function to unlock the Lockup position whose lockup duration has expired
// @param pool_identifer : Pool identifier to identify the LP pool whose Token is locked in the lockup position
// @param duration : Lockup duration (number of weeks)
pub fn handle_unlock_position(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    pool_identifer: String,
    duration: u64,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;
    let user_address = info.sender.clone();

    // CHECK ::: Is LP Token Pool supported or not ?
    if is_str_present_in_vec(
        state.supported_pairs_list.clone(),
        pool_identifer.clone().to_string(),
    ) {
        return Err(StdError::generic_err("Already supported"));
    }

    let pool_info = ASSET_POOLS.load(deps.storage, &pool_identifer.clone().as_bytes())?;

    let lockup_id = user_address.to_string().clone()
        + &pool_identifer.to_string().clone()
        + &duration.to_string();
    let lockup_info = LOCKUP_INFO
        .may_load(deps.storage, lockup_id.clone().as_bytes())?
        .unwrap_or_default();

    // CHECK :: Can the Lockup position be unlocked or not ?
    if env.block.time.seconds() < lockup_info.unlock_timestamp {
        return Err(StdError::generic_err(format!(
            "{} seconds to unlock",
            lockup_info.unlock_timestamp - env.block.time.seconds()
        )));
    }

    // CHECK :: Is the lockup position valid / already unlocked or not ?
    if lockup_info.astroport_lp_units == Uint256::zero() {
        return Err(StdError::generic_err("Invalid request"));
    }

    // Check is user's total ASTRO rewards have been calculated or not, and calculate and store them in case they are not calculated yet
    let mut user_info = USER_INFO
        .may_load(deps.storage, &user_address.clone())?
        .unwrap_or_default();

    // If user's total ASTRO rewards == 0 :: We update all of the user's lockup positions to calculate ASTRO rewards and for each alongwith their equivalent Astroport LP Shares
    if user_info.total_astro_rewards == Uint256::zero() {
        user_info.total_astro_rewards = update_user_lockup_positions_calc_rewards_and_migrate(
            deps.branch(),
            &config,
            user_info.clone(),
        )?;
        USER_INFO.save(deps.storage, &user_address, &user_info)?;
    }

    let mut cosmos_msgs = vec![];

    // QUERY :: Check if there are any unclaimed staking rewards
    let unclaimed_rewards_response = query_unclaimed_staking_rewards_for_pool(
        deps.querier,
        config.generator_address.to_string(),
        pool_info.astroport_pair.lp_token_addr.clone(),
        env.contract.address.clone(),
    );

    // --> If unclaimed rewards > 0
    if unclaimed_rewards_response.pending > Uint128::zero() {
        // Returns an array with 2 CosmosMsgs
        // 1.  Cosmos Msg to claim rewards from the generator contract
        // 2. Callback Cosmos Msg to update state after rewards are claimed
        let cosmos_msg_array = prepare_cosmos_msgs_to_claim_dual_rewards(
            deps.querier,
            &config,
            pool_identifer.clone(),
            deps.api
                .addr_validate(&pool_info.cw20_asset.contract_addr)?,
            pool_info.astroport_pair.lp_token_addr.clone(),
            env.contract.address.clone(),
        )?;
        for msg_ in cosmos_msg_array {
            cosmos_msgs.push(msg_);
        }
    }

    // Callback Cosmos Msg :: To withdraw User's lockup rewards [withdraw_lp_stake = true as LP tokens will be trasferred to the user]
    let withdraw_user_rewards_for_lockup_msg = CallbackMsg::WithdrawUserLockupRewardsCallback {
        user_address: user_address.clone(),
        pool_identifer: pool_identifer.clone(),
        duration: duration,
        withdraw_lp_stake: true,
        force_unlock: false,
    }
    .to_cosmos_msg(&env.contract.address)?;
    cosmos_msgs.push(withdraw_user_rewards_for_lockup_msg);

    Ok(Response::new()
        .add_messages(cosmos_msgs)
        .add_attributes(vec![
            ("action", "lockdrop::ExecuteMsg::UnlockLockupPosition"),
            ("pool_identifer", &pool_identifer),
            ("user_address", &user_address.to_string()),
            ("lockup_id", &lockup_id.to_string()),
            (
                "Astroport_LP_tokens_unlocked",
                &lockup_info.astroport_lp_units.to_string(),
            ),
        ]))
}

/// @dev Function to forcefully unlock a Lockup position whose lockup duration has not expired yet.
/// User needs to return the whole ASTRO rewards he received as part of this lockup position to forcefully unclock the position
/// @dev Function to unlock the Lockup position whose lockup duration has expired
/// @param pool_identifer : Pool identifier to identify the LP pool whose Token is locked in the lockup position
/// @param duration : Lockup duration (number of weeks)
pub fn handle_force_unlock_position(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    pool_identifer: String,
    duration: u64,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;
    let user_address = info.sender.clone();

    // CHECK ::: Is LP Token Pool supported or not ?
    if is_str_present_in_vec(
        state.supported_pairs_list.clone(),
        pool_identifer.clone().to_string(),
    ) {
        return Err(StdError::generic_err("Already supported"));
    }

    let pool_info = ASSET_POOLS.load(deps.storage, &pool_identifer.clone().as_bytes())?;

    let lockup_id = user_address.to_string().clone()
        + &pool_identifer.to_string().clone()
        + &duration.to_string();
    let lockup_info = LOCKUP_INFO
        .may_load(deps.storage, lockup_id.clone().as_bytes())?
        .unwrap_or_default();

    // CHECK :: Can the Lockup position be unlocked without force
    if env.block.time.seconds() > lockup_info.unlock_timestamp {
        return Err(StdError::generic_err(
            "Lockup can be unlocked without force",
        ));
    }

    // CHECK :: Is the lockup position valid / already unlocked or not ?
    if lockup_info.astroport_lp_units == Uint256::zero() {
        return Err(StdError::generic_err("Invalid request"));
    }

    // Check is user's total ASTRO rewards have been calculated or not, and calculate and store them in case they are not calculated yet
    let mut user_info = USER_INFO
        .may_load(deps.storage, &user_address.clone())?
        .unwrap_or_default();

    // If user's total ASTRO rewards == 0 :: We update all of the user's lockup positions to calculate ASTRO rewards and for each alongwith their equivalent Astroport LP Shares
    if user_info.total_astro_rewards == Uint256::zero() {
        user_info.total_astro_rewards = update_user_lockup_positions_calc_rewards_and_migrate(
            deps.branch(),
            &config,
            user_info.clone(),
        )?;
        USER_INFO.save(deps.storage, &user_address, &user_info)?;
    }

    let mut cosmos_msgs = vec![];

    // QUERY :: Check if there are any unclaimed staking rewards
    let unclaimed_rewards_response = query_unclaimed_staking_rewards_for_pool(
        deps.querier,
        config.generator_address.to_string(),
        pool_info.astroport_pair.lp_token_addr.clone(),
        env.contract.address.clone(),
    );

    // --> If unclaimed rewards > 0
    if unclaimed_rewards_response.pending > Uint128::zero() {
        // Returns an array with 2 CosmosMsgs
        // 1.  Cosmos Msg to claim rewards from the generator contract
        // 2. Callback Cosmos Msg to update state after rewards are claimed
        let cosmos_msg_array = prepare_cosmos_msgs_to_claim_dual_rewards(
            deps.querier,
            &config,
            pool_identifer.clone(),
            deps.api
                .addr_validate(&pool_info.cw20_asset.contract_addr)?,
            pool_info.astroport_pair.lp_token_addr.clone(),
            env.contract.address.clone(),
        )?;
        for msg_ in cosmos_msg_array {
            cosmos_msgs.push(msg_);
        }
    }

    // Callback Cosmos Msg :: To withdraw User's lockup rewards [withdraw_lp_stake = true, force_unlock = true as the lockup is forcefully unlocked]
    let withdraw_user_rewards_for_lockup_msg = CallbackMsg::WithdrawUserLockupRewardsCallback {
        user_address: user_address.clone(),
        pool_identifer: pool_identifer.clone(),
        duration: duration,
        withdraw_lp_stake: true,
        force_unlock: true,
    }
    .to_cosmos_msg(&env.contract.address)?;
    cosmos_msgs.push(withdraw_user_rewards_for_lockup_msg);

    Ok(Response::new()
        .add_messages(cosmos_msgs)
        .add_attributes(vec![
            (
                "action",
                "lockdrop::ExecuteMsg::ForcefullyUnlockLockupPosition",
            ),
            ("pool_identifer", &pool_identifer),
            ("user_address", &user_address.to_string()),
            ("lockup_id", &lockup_id.to_string()),
            (
                "Astroport_LP_tokens_unlocked",
                &lockup_info.astroport_lp_units.to_string(),
            ),
        ]))
}

//----------------------------------------------------------------------------------------
// Callback Functions
//----------------------------------------------------------------------------------------

/// @dev CALLBACK Function to update contract state after dual stakinf rewards are claimed from the generator contract
/// @param pool_identifer : Pool identifier to identify the LP pool whose rewards have been claimed
/// @param prev_astro_balance : Contract's ASTRO token balance before claim
/// @param prev_dual_reward_balance : Contract's DUAL token reward balance before claim
pub fn update_pool_on_dual_rewards_claim(
    deps: DepsMut,
    env: Env,
    pool_identifer: String,
    prev_astro_balance: Uint256,
    prev_dual_reward_balance: Uint256,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut pool_info = ASSET_POOLS.load(deps.storage, &pool_identifer.clone().as_bytes())?;

    // QUERY CURRENT ASTRO / DUAL REWARD TOKEN BALANCE :: Used to calculate claimed rewards
    let cur_astro_balance = cw20_get_balance(
        &deps.querier,
        config.astro_token_address.clone(),
        env.contract.address.clone(),
    )?;
    let cur_dual_reward_balance = cw20_get_balance(
        &deps.querier,
        deps.api
            .addr_validate(&pool_info.cw20_asset.contract_addr.clone())?,
        env.contract.address.clone(),
    )?;
    let astro_claimed = Uint256::from(cur_astro_balance) - prev_astro_balance;
    let dual_reward_claimed = Uint256::from(cur_dual_reward_balance) - prev_dual_reward_balance;

    // UPDATE ASTRO & DUAL REWARD INDEXED FOR THE CURRENT POOL
    update_pool_reward_indexes(&mut pool_info, astro_claimed, dual_reward_claimed);

    // SAVE UPDATED STATE OF THE POOL
    ASSET_POOLS.save(deps.storage, &pool_identifer.as_bytes(), &pool_info)?;

    Ok(Response::new().add_attributes(vec![
        ("action", "lockdrop::CallbackMsg::UpdatePoolIndexes"),
        ("lp_token_addr", pool_identifer.as_str()),
        ("astro_claimed", astro_claimed.to_string().as_str()),
        (
            "dual_reward_claimed",
            dual_reward_claimed.to_string().as_str(),
        ),
        (
            "astro_global_reward_index",
            pool_info.astro_global_reward_index.to_string().as_str(),
        ),
        (
            "asset_global_reward_index",
            pool_info.asset_global_reward_index.to_string().as_str(),
        ),
    ]))
}

/// @dev CALLBACK Function to withdraw user rewards and LP Tokens after claims / unlocks
/// @param user_address : User address who is claiming the rewards / unlocking his lockup position
/// @param pool_identifer : Pool identifier to identify the LP pool
/// @param duration : Duration of the lockup for which rewards have been claimed / position unlocked
/// @param withdraw_lp_stake : Boolean value indicating if the ASTRO LP Tokens are to be sent to the user or not
/// @param force_unlock : Boolean value indicating if Position is forcefully being unlocked or not
pub fn callback_withdraw_user_rewards_for_lockup_optional_withdraw(
    deps: DepsMut,
    env: Env,
    user_address: Addr,
    pool_identifer: String,
    duration: u64,
    withdraw_lp_stake: bool,
    force_unlock: bool,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut pool_info = ASSET_POOLS.load(deps.storage, &pool_identifer.clone().as_bytes())?;
    let lockup_id =
        user_address.to_string().clone() + &pool_identifer.clone() + &duration.to_string();
    let mut lockup_info = LOCKUP_INFO
        .may_load(deps.storage, lockup_id.clone().as_bytes())?
        .unwrap_or_default();

    // UPDATE ASTRO & DUAL REWARD INDEXED FOR THE CURRENT POOL
    let pending_astro_rewards =
        compute_lockup_position_accrued_astro_rewards(&pool_info, &mut lockup_info);
    let pending_dual_rewards =
        compute_lockup_position_accrued_dual_rewards(&pool_info, &mut lockup_info);

    // COSMOS MSG :: Transfer pending ASTRO / DUAL Rewards
    let mut cosmos_msgs = vec![];
    if pending_astro_rewards > Uint256::zero() {
        cosmos_msgs.push(build_transfer_cw20_token_msg(
            user_address.clone(),
            config.astro_token_address.clone().to_string(),
            pending_astro_rewards.into(),
        )?);
    }
    if pending_dual_rewards > Uint256::zero() {
        cosmos_msgs.push(build_transfer_cw20_token_msg(
            user_address.clone(),
            pool_info.cw20_asset.contract_addr.clone().to_string(),
            pending_dual_rewards.into(),
        )?);
    }

    if withdraw_lp_stake {
        let mut user_info = USER_INFO.load(deps.storage, &user_address.clone())?;

        // COSMOS MSG :: Transfers ASTRO (that user received as rewards for this lockup position) from user to itself
        if force_unlock {
            let mut state = STATE.load(deps.storage)?;
            let transfer_astro_msg = build_transfer_cw20_from_user_msg(
                config.astro_token_address.clone().to_string(),
                user_address.clone().to_string(),
                env.contract.address.to_string(),
                lockup_info.astro_rewards,
            )?;
            state.total_astro_returned_available += lockup_info.astro_rewards;
            STATE.save(deps.storage, &state)?;
            cosmos_msgs.push(transfer_astro_msg);
        }

        //  COSMOSMSG :: If LP Tokens are staked, we unstake the amount which needs to be returned to the user
        if pool_info.is_staked {
            let unstake_lp_msg = build_unstake_from_generator_msg(
                config.generator_address.clone().to_string(),
                pool_info.astroport_pair.lp_token_addr.clone(),
                lockup_info.astroport_lp_units,
            )?;
            cosmos_msgs.push(unstake_lp_msg);
        }
        // COSMOSMSG :: Returns LP units locked by the user in the current lockup position
        let transfer_lp_msg = build_transfer_cw20_token_msg(
            user_address.clone(),
            pool_info.astroport_pair.lp_token_addr.clone().to_string(),
            lockup_info.astroport_lp_units.into(),
        )?;
        cosmos_msgs.push(transfer_lp_msg);

        // UPDATE STATE :: Lockup, state, pool, user
        // Remove lockup position from user's lockup position array
        lockup_info.astroport_lp_units = Uint256::zero();
        remove_lockup_pos_from_user_info(&mut user_info, lockup_id.clone());
        pool_info.astroport_pair.amount =
            pool_info.astroport_pair.amount - lockup_info.lp_units_locked;

        // Save updated pool state & user info
        USER_INFO.save(deps.storage, &user_address, &user_info)?;
        ASSET_POOLS.save(deps.storage, &pool_identifer.as_bytes(), &pool_info)?;
    }

    // Save updated state
    LOCKUP_INFO.save(deps.storage, lockup_id.clone().as_bytes(), &lockup_info)?;

    Ok(Response::new()
        .add_messages(cosmos_msgs)
        .add_attributes(vec![
            (
                "action",
                "lockdrop::CallbackMsg::WithdrawPendingRewards_UnlockedPosition",
            ),
            ("lp_token_addr", pool_identifer.as_str()),
            ("user_address", user_address.to_string().as_str()),
            ("duration", duration.to_string().as_str()),
            (
                "pending_astro_rewards",
                pending_astro_rewards.to_string().as_str(),
            ),
            (
                "pending_dual_rewards",
                pending_dual_rewards.to_string().as_str(),
            ),
            ("lockup_id", lockup_id.to_string().as_str()),
            (
                "astroport_lp_units_unlocked",
                lockup_info.astroport_lp_units.to_string().as_str(),
            ),
            ("unlock", withdraw_lp_stake.to_string().as_str()),
            ("forced_unlock", force_unlock.to_string().as_str()),
        ]))
}

/// @dev CALLBACK Function to deposit Liquidity in Astroport after its withdrawn from terraswap
/// @param pool_identifer : Pool identifier to identify the LP pool
/// @param native_asset : Native asset (uusd / uluna) of the pool
/// @param prev_native_asset_balance : Contract's native asset balance before liquidity was withdrawn from terraswap
/// @param cw20_asset :  CW20 asset of the pool
/// @param prev_cw20_asset_balance : Contract's CW20 asset balance before liquidity was withdrawn from terraswap
/// @param astroport_pool : Astroport Pool details to which the liquidity is to be migrated
pub fn callback_deposit_liquidity_in_astroport(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    pool_identifer: String,
    native_asset: NativeAsset,
    prev_native_asset_balance: Uint128,
    cw20_asset: Cw20Asset,
    prev_cw20_asset_balance: Uint128,
    astroport_pool: LiquidityPool,
) -> StdResult<Response> {
    let pool_info = ASSET_POOLS.load(deps.storage, &pool_identifer.clone().as_bytes())?;

    // QUERY :: Get current Asset and uusd balances to calculate how much liquidity was withdrawn from the terraswap pool
    let astroport_lp_balance = cw20_get_balance(
        &deps.querier,
        astroport_pool.lp_token_addr.clone(),
        env.contract.address.clone(),
    )?;
    let asset_balance = cw20_get_balance(
        &deps.querier,
        deps.api
            .addr_validate(&pool_info.cw20_asset.contract_addr)?,
        env.contract.address.clone(),
    )?;
    let native_balance_response = deps.querier.query_balance(
        env.contract.address.clone(),
        pool_info.native_asset.denom.to_string(),
    )?;
    let native_balance = native_balance_response.amount;

    // Calculate cw20 / native tokens withdrawn from the terraswap pool
    let asset_balance_withdrawn = asset_balance - prev_cw20_asset_balance;
    let native_balance_withdrawn = native_balance - prev_native_asset_balance;

    // ASSET DEFINATIONS
    let cw20_asset_ = astroport::asset::Asset {
        info: astroport::asset::AssetInfo::Token {
            contract_addr: deps
                .api
                .addr_validate(&cw20_asset.contract_addr.to_string())?,
        },
        amount: asset_balance_withdrawn.into(),
    };
    let native_asset_ = astroport::asset::Asset {
        info: astroport::asset::AssetInfo::NativeToken {
            denom: native_asset.denom.clone(),
        },
        amount: native_balance_withdrawn.into(),
    };
    let assets_ = [cw20_asset_, native_asset_];

    // COSMOS MSGS
    // :: 1.  APPROVE CW20 Token WITH ASTROPORT POOL ADDRESS AS BENEFICIARY
    // :: 2.  ADD LIQUIDITY
    // :: 3. CallbackMsg :: Update state on liquidity addition to LP Pool
    let approve_cw20_msg = build_approve_cw20_msg(
        cw20_asset.contract_addr,
        astroport_pool.pair_addr.to_string(),
        asset_balance_withdrawn.into(),
    )?;
    let add_liquidity_msg = build_provide_liquidity_to_lp_pool_msg(
        deps.as_ref(),
        assets_,
        astroport_pool.pair_addr.clone(),
        native_asset.denom,
        native_balance_withdrawn,
    )?;
    let update_state_msg = CallbackMsg::UpdateStateLiquidityMigrationCallback {
        pool_identifer: pool_identifer.clone(),
        astroport_pool: astroport_pool.clone(),
        astroport_lp_balance: astroport_lp_balance,
    }
    .to_cosmos_msg(&env.contract.address)?;

    Ok(Response::new()
        .add_messages([approve_cw20_msg, add_liquidity_msg, update_state_msg])
        .add_attributes(vec![
            ("action", "lockdrop::CallbackMsg::AddLiquidityToAstroport"),
            ("pool_identifer", &pool_identifer),
            ("asset_balance", &asset_balance.to_string()),
            ("native_balance", &native_balance.to_string()),
        ]))
}

/// @dev CALLBACK Function to update contract state after Liquidity in added to the Astroport Pool
/// @param pool_identifer : Pool identifier to identify the LP pool
/// @param astroport_pool : Astroport Pool details to which the liquidity is to be migrated
/// @param prev_astroport_lp_balance : Contract's Astroport LP token balance before liquidity was added to the pool
pub fn callback_update_pool_state_after_migration(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    pool_identifer: String,
    astroport_pool: LiquidityPool,
    prev_astroport_lp_balance: Uint128,
) -> StdResult<Response> {
    let mut pool_info = ASSET_POOLS.load(deps.storage, &pool_identifer.as_bytes())?;

    // QUERY :: Get Astroport LP Balance to calculate how many were minted upon liquidity addition
    let astroport_lp_balance = cw20_get_balance(
        &deps.querier,
        astroport_pool.lp_token_addr.clone(),
        env.contract.address.clone(),
    )?;
    let astroport_lp_minted = astroport_lp_balance - prev_astroport_lp_balance;

    // POOL INFO :: UPDATE STATE --> SAVE
    pool_info.astroport_pair.lp_token_addr = astroport_pool.lp_token_addr.clone();
    pool_info.astroport_pair.pair_addr = astroport_pool.pair_addr;
    pool_info.astroport_pair.amount = astroport_lp_minted.into();
    pool_info.is_migrated = true;
    ASSET_POOLS.save(deps.storage, &pool_identifer.clone().as_bytes(), &pool_info)?;

    Ok(Response::new().add_attributes(vec![
        ("action", "lockdrop::CallbackMsg::UpdateStateAfterMigration"),
        ("pool_identifer", &pool_identifer),
        ("astroport_lp_minted", &astroport_lp_minted.to_string()),
    ]))
}

// //----------------------------------------------------------------------------------------
// // Query Functions
// //----------------------------------------------------------------------------------------

/// @dev Returns the contract's configuration
pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;

    Ok(ConfigResponse {
        owner: config.owner.to_string(),
        auction_contract_address: config.auction_contract_address.to_string(),
        generator_address: config.generator_address.to_string(),
        astro_token_address: config.astro_token_address.to_string(),
        init_timestamp: config.init_timestamp,
        deposit_window: config.deposit_window,
        withdrawal_window: config.withdrawal_window,
        min_lock_duration: config.min_lock_duration,
        max_lock_duration: config.max_lock_duration,
        seconds_per_week: config.seconds_per_week,
        weekly_multiplier: config.weekly_multiplier,
        lockdrop_incentives: config.lockdrop_incentives,
    })
}

/// @dev Returns the contract's State
pub fn query_state(deps: Deps) -> StdResult<StateResponse> {
    let state: State = STATE.load(deps.storage)?;
    Ok(StateResponse {
        total_astro_delegated: state.total_astro_delegated,
        total_astro_returned_available: state.total_astro_returned_available,
        are_claims_allowed: state.are_claims_allowed,
        supported_pairs_list: state.supported_pairs_list,
    })
}

/// @dev Returns the pool's State
pub fn query_pool(deps: Deps, pool_identifer: String) -> StdResult<PoolResponse> {
    let pool_info: PoolInfo = ASSET_POOLS.load(deps.storage, &pool_identifer.as_bytes())?;
    Ok(PoolResponse {
        terraswap_pair: pool_info.terraswap_pair,
        astroport_pair: pool_info.astroport_pair,
        cw20_asset: pool_info.cw20_asset,
        native_asset: pool_info.native_asset,
        incentives_percent: pool_info.incentives_percent,
        is_staked: pool_info.is_staked,
        is_migrated: pool_info.is_migrated,
        weighted_amount: pool_info.weighted_amount,
        astro_global_reward_index: pool_info.astro_global_reward_index,
        asset_global_reward_index: pool_info.asset_global_reward_index,
    })
}

/// @dev Returns summarized details regarding the user
pub fn query_user_info(deps: Deps, _env: Env, user: String) -> StdResult<UserInfoResponse> {
    let user_address = deps.api.addr_validate(&user)?;
    let user_info = USER_INFO
        .may_load(deps.storage, &user_address.clone())?
        .unwrap_or_default();

    Ok(UserInfoResponse {
        total_astro_rewards: user_info.total_astro_rewards,
        delegated_astro_rewards: user_info.delegated_astro_rewards,
        lockup_positions: user_info.lockup_positions,
    })
}

/// @dev Returns summarized details regarding the user
pub fn query_lockup_info(
    deps: Deps,
    user_address: String,
    lp_token_address: String,
    duration: u64,
) -> StdResult<LockUpInfoResponse> {
    let lockup_id = user_address.to_string() + &lp_token_address + &duration.to_string();
    query_lockup_info_with_id(deps, lockup_id)
}

/// @dev Returns summarized details regarding the user
pub fn query_lockup_info_with_id(deps: Deps, lockup_id: String) -> StdResult<LockUpInfoResponse> {
    let lockup_info = LOCKUP_INFO
        .may_load(deps.storage, lockup_id.clone().as_bytes())?
        .unwrap_or_default();

    Ok(LockUpInfoResponse {
        pool_identifier: lockup_info.pool_identifier,
        duration: lockup_info.duration,
        lp_units_locked: lockup_info.lp_units_locked,
        astro_rewards: lockup_info.astro_rewards,
        is_migrated: lockup_info.is_migrated,
        withdrawal_counter: lockup_info.withdrawal_counter,
        unlock_timestamp: lockup_info.unlock_timestamp,
        astro_reward_index: lockup_info.astro_reward_index,
        dual_reward_index: lockup_info.dual_reward_index,
    })
}

//----------------------------------------------------------------------------------------
// HELPERS :: BOOLEANS & COMPUTATIONS (Rewards, Indexes etc)
//----------------------------------------------------------------------------------------

/// @dev Returns true is deposits are currently allowed else returns false
/// @params current_timestamp : Current block timestamp
/// @params config : Contract configuration
fn is_deposit_open(current_timestamp: u64, config: &Config) -> bool {
    let deposits_opened_till = config.init_timestamp + config.deposit_window;
    (current_timestamp >= config.init_timestamp) && (deposits_opened_till >= current_timestamp)
}

///  @dev Helper function to calculate maximum % of LP balances deposited that can be withdrawn
/// @params current_timestamp : Current block timestamp
/// @params config : Contract configuration
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

    // max withdrawal allowed decreasing linearly from 50% to 0% vs time elapsed
    let withdrawal_cutoff_final = withdrawal_cutoff_sec_point + (config.withdrawal_window / 2u64);
    //  Deposit window closed, 2nd half of withdrawal window :: max withdrawal allowed decreases linearly from 50% to 0% vs time elapsed
    if current_timestamp < withdrawal_cutoff_final {
        let slope = Decimal256::from_ratio(50u64, config.withdrawal_window / 2u64);
        let time_elapsed = current_timestamp - withdrawal_cutoff_sec_point;
        return Decimal256::from_ratio(time_elapsed, 1u64) * slope;
    }
    // Withdrawals not allowed
    else {
        return Decimal256::from_ratio(0u32, 100u32);
    }
}

/// @dev Helper function to calculate ASTRO rewards for a particular Lockup position
/// @params lockup_weighted_balance : Lockup position's weighted terraswap LP balance
/// @params total_weighted_amount : Total weighted terraswap LP balance of the Pool
/// @params incentives_percent : % of total ASTRO incentives allocated to this pool
/// @params total_lockdrop_incentives : Total ASTRO incentives to be distributed among Lockdrop participants
pub fn calculate_astro_incentives_for_lockup(
    lockup_weighted_balance: Uint256,
    total_weighted_amount: Uint256,
    incentives_percent: Decimal256,
    total_lockdrop_incentives: Uint256,
) -> Uint256 {
    let total_pool_incentives = incentives_percent * total_lockdrop_incentives;
    let percent_weight_of_total =
        Decimal256::from_ratio(lockup_weighted_balance, total_weighted_amount);
    percent_weight_of_total * total_pool_incentives
}

/// @dev Returns the timestamp when the lockup will get unlocked
/// @params config : Configuration
/// @params duration :Lockup duration (number of weeks)
fn calculate_unlock_timestamp(config: &Config, duration: u64) -> u64 {
    config.init_timestamp
        + config.deposit_window
        + config.withdrawal_window
        + (duration * config.seconds_per_week)
}

/// @dev Helper function. Returns effective weight for the amount to be used for calculating airdrop rewards
/// @params amount : Number of LP tokens
/// @params duration : Number of weeks
/// @weekly_multiplier : A constant weight multipler
fn calculate_weight(amount: Uint256, duration: u64, weekly_multiplier: Decimal256) -> Uint256 {
    let duration_weighted_amount = amount * Uint256::from(duration);
    duration_weighted_amount * weekly_multiplier
}

/// @dev Calculates equivalent Astroport LP Token balance against the Terraswap LP tokens deposited by the user post Liquidity migration
/// @params user_lp_units : Terraswap LP tokens deposited by the user
/// @params total_terraswap_lp_locked : Total Terraswap LP tokens deposited in the contract for a particular LP Pool
pub fn calculate_lockup_balance_post_migration(
    user_lp_units: Uint256,
    total_terraswap_lp_locked: Uint256,
    total_astroport_lp_locked: Uint256,
) -> Uint256 {
    let percent_of_total = Decimal256::from_ratio(user_lp_units, total_terraswap_lp_locked);
    percent_of_total * total_astroport_lp_locked
}

//-----------------------------------------------------------
// HELPER FUNCTIONS :: UPDATE STATE
//-----------------------------------------------------------

/// @dev Updates indexes for ASTRO & ASSET rewards as they are accrued by a particular LP tokens staked with the generator contract
/// @params pool_info : Pool Info for the LP Pool whose indexes are to be updated
/// @params astro_accured : ASTRO tokens accrued by the LP Pool to be added to the index
/// @params dual_rewards_accured : ASSET tokens accrued by the LP Pool to be added to the index
fn update_pool_reward_indexes(
    pool_info: &mut PoolInfo,
    astro_accured: Uint256,
    dual_rewards_accured: Uint256,
) {
    if !pool_info.is_staked {
        return;
    }
    let astro_rewards_index_increment =
        Decimal256::from_ratio(astro_accured, pool_info.astroport_pair.amount);
    pool_info.astro_global_reward_index =
        pool_info.astro_global_reward_index + astro_rewards_index_increment;

    let dual_rewards_index_increment =
        Decimal256::from_ratio(dual_rewards_accured, pool_info.astroport_pair.amount);
    pool_info.asset_global_reward_index =
        pool_info.asset_global_reward_index + dual_rewards_index_increment;
}

/// @dev Calculate unclaimed ASTRO rewards (accrued via LP Staking with generator contract) for a particular lockup position
/// @params pool_info : Pool Info for the LP Pool whose tokens are locked with the position
/// @params lockup_info :Lockup position info whose rewards are to be calculated
fn compute_lockup_position_accrued_astro_rewards(
    pool_info: &PoolInfo,
    lockup_info: &mut LockupInfo,
) -> Uint256 {
    let pending_astro_rewards = (lockup_info.astroport_lp_units
        * pool_info.astro_global_reward_index)
        - (lockup_info.astroport_lp_units * lockup_info.astro_reward_index);
    lockup_info.astro_reward_index = pool_info.astro_global_reward_index;
    pending_astro_rewards
}

/// @dev Calculate unclaimed ASSET rewards (accrued via LP Staking with generator contract) for a particular lockup position
/// @params pool_info : Pool Info for the LP Pool whose tokens are locked with the position
/// @params lockup_info :Lockup position info whose rewards are to be calculated
fn compute_lockup_position_accrued_dual_rewards(
    pool_info: &PoolInfo,
    lockup_info: &mut LockupInfo,
) -> Uint256 {
    let pending_dual_rewards = (lockup_info.astroport_lp_units
        * pool_info.asset_global_reward_index)
        - (lockup_info.astroport_lp_units * lockup_info.dual_reward_index);
    lockup_info.dual_reward_index = pool_info.asset_global_reward_index;
    pending_dual_rewards
}

/// @dev Deletes Lockup identifer string from the list of user's active lockup positions
/// @params user_info : User info struct (mutable)
/// @params lockup_id : Lockup Id which is to be deleted
fn remove_lockup_pos_from_user_info(user_info: &mut UserInfo, lockup_id: String) {
    let index = user_info
        .lockup_positions
        .iter()
        .position(|x| *x == lockup_id)
        .unwrap();
    user_info.lockup_positions.remove(index);
}

/// @dev Function to calculate ASTRO rewards for each of the user position and update positions via calculating equivalent Astroport LP units after migration
/// @params configuration struct
/// @params user Info struct
/// Returns user's total ASTRO rewards
fn update_user_lockup_positions_calc_rewards_and_migrate(
    deps: DepsMut,
    config: &Config,
    user_info: UserInfo,
) -> StdResult<Uint256> {
    let mut total_astro_rewards = Uint256::zero();

    for lockup_id in user_info.lockup_positions {
        // Retrieve mutable Lockup position , and pool info structs
        let mut lockup_info = LOCKUP_INFO
            .load(deps.storage, lockup_id.as_bytes())
            .unwrap();
        let pool_info = ASSET_POOLS
            .load(deps.storage, &lockup_info.pool_identifier.as_bytes())
            .unwrap();

        // After migration, we need to calculate user LP balances for Astroport LP tokens equally weighed according to their initial terraswap LP deposits
        if pool_info.is_migrated && !lockup_info.is_migrated {
            lockup_info.astroport_lp_units = calculate_lockup_balance_post_migration(
                lockup_info.lp_units_locked,
                pool_info.terraswap_pair.amount,
                pool_info.astroport_pair.amount,
            );
            lockup_info.is_migrated = true;
        }

        // Weighted lockup balance (using terraswap LP units to calculate as pool's total weighted balance is calculated on terraswap LP deposits summed over each deposit tx)
        let weighted_lockup_balance = calculate_weight(
            lockup_info.lp_units_locked,
            lockup_info.duration,
            config.weekly_multiplier,
        );

        // Calculate ASTRO Lockdrop rewards for the lockup position
        lockup_info.astro_rewards = calculate_astro_incentives_for_lockup(
            weighted_lockup_balance,
            pool_info.weighted_amount,
            pool_info.incentives_percent,
            config.lockdrop_incentives,
        );

        // Save updated Lockup state
        LOCKUP_INFO.save(deps.storage, lockup_id.as_bytes(), &lockup_info)?;
        total_astro_rewards += lockup_info.astro_rewards;
    }
    Ok(total_astro_rewards)
}

//-----------------------------------------------------------
// HELPER FUNCTIONS :: QUERY HELPERS
//-----------------------------------------------------------

/// @dev Queries terraswap pair to fetch the list of assets supported by the pool
/// @params pair_addr : Pair address to be quereied
fn query_terraswap_pair_assets(
    querier: &QuerierWrapper,
    pair_addr: String,
) -> StdResult<[terraswapAsset; 2]> {
    let pool_response: terraswap::pair::PoolResponse =
        querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: pair_addr,
            msg: to_binary(&terraswap::pair::QueryMsg::Pool {})?,
        }))?;
    Ok(pool_response.assets)
}

/// @dev Queries the generator contract to check if there are any unclaimed staking dual rewards for the associated LP Pool's Lp tokens
/// @params generator_address : Generator contract address
/// @params astroport_lp_token : Astroport LP Token address for which we need to query unclaimed rewards
/// @params contract_addr : Lockdrop contract address
fn query_unclaimed_staking_rewards_for_pool(
    querier: QuerierWrapper,
    generator_address: String,
    astroport_lp_token: Addr,
    contract_addr: Addr,
) -> PendingTokenResponse {
    let unclaimed_rewards_response: PendingTokenResponse = querier
        .query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: generator_address,
            msg: to_binary(&GenQueryMsg::PendingToken {
                lp_token: astroport_lp_token,
                user: contract_addr,
            })
            .unwrap(),
        }))
        .unwrap();
    unclaimed_rewards_response
}

//-----------------------------------------------------------
// HELPER FUNCTIONS :: COSMOS_MSGs
//-----------------------------------------------------------

/// @dev Helper function. Returns an Array containing CosmosMsgs to Claim rewards from the generator contract and update contract state once rewards are claimed
/// @params pool_identifer : LP Pool identifer
/// @params asset_token_addr : CW20 Token address which is the dual reward being accrued for the staked LP tokens
/// @params astroport_lp_token : Astroport LP Token address for which we need to query unclaimed rewards
/// @params contract_addr : Lockdrop contract address
fn prepare_cosmos_msgs_to_claim_dual_rewards(
    querier: QuerierWrapper,
    config: &Config,
    pool_identifer: String,
    asset_token_addr: Addr,
    astroport_lp_token: Addr,
    contract_addr: Addr,
) -> StdResult<[CosmosMsg; 2]> {
    // let mut cosmos_msgs = [];

    // Callback Cosmos Msg :: Add Cosmos Msg to claim rewards from the generator contract
    let dual_reward_claim_msg = build_claim_dual_rewards(
        contract_addr.clone(),
        astroport_lp_token,
        config.generator_address.clone(),
    )?;
    // QUERY :: Current ASTRO & ASSET Token Balance
    let astro_balance = cw20_get_balance(
        &querier,
        config.astro_token_address.clone(),
        contract_addr.clone(),
    )?;
    let dual_reward_balance = cw20_get_balance(&querier, asset_token_addr, contract_addr.clone())?;
    // Callback Cosmos Msg :: Add Cosmos Msg (UpdatePoolOnDualRewardsClaim) to update state once dual rewards are claimed
    let update_pool_state_msg = CallbackMsg::UpdatePoolOnDualRewardsClaim {
        pool_identifer: pool_identifer.clone(),
        prev_astro_balance: astro_balance.into(),
        prev_dual_reward_balance: dual_reward_balance.into(),
    }
    .to_cosmos_msg(&contract_addr)?;
    Ok([dual_reward_claim_msg, update_pool_state_msg])
}

/// @dev Returns CosmosMsg to unstake Astroport LP Tokens from the generator contract
/// @params generator_address : Generator contract address
/// @params lp_token_addr : Astroport LP token address to be unstaked
/// @params unstake_amount : Amount to be unstaked
fn build_unstake_from_generator_msg(
    generator_address: String,
    lp_token_addr: Addr,
    unstake_amount: Uint256,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: generator_address.to_string(),
        funds: vec![],
        msg: to_binary(&astroport::generator::ExecuteMsg::Withdraw {
            lp_token: lp_token_addr,
            amount: unstake_amount.into(),
        })?,
    }))
}

/// @dev Returns CosmosMsg to stake Astroport LP Tokens with the generator contract
/// @params generator_address : Generator contract address
/// @params lp_token_addr : Astroport LP token address to be staked
/// @params unstake_amount : Amount to be staked
fn build_stake_with_generator_msg(
    generator_address: String,
    lp_token_addr: Addr,
    stake_amount: Uint256,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: generator_address.to_string(),
        funds: vec![],
        msg: to_binary(&astroport::generator::ExecuteMsg::Deposit {
            lp_token: lp_token_addr,
            amount: stake_amount.into(),
        })?,
    }))
}

/// @dev Returns CosmosMsg to claim orphan rewards for a particular Astroport LP token from the generator contract
/// @params recepient_address : contract address
/// @params lp_token_contract : Astroport LP token address for which the rewards are to be claimed
/// @params generator_contract : Generator contract address
fn build_claim_dual_rewards(
    recepient_address: Addr,
    lp_token_contract: Addr,
    generator_contract: Addr,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: generator_contract.to_string(),
        funds: vec![],
        msg: to_binary(&astroport::generator::ExecuteMsg::SendOrphanReward {
            recipient: recepient_address.to_string(),
            lp_token: Some(lp_token_contract.to_string()),
        })?,
    }))
}

// @dev  Helper function which returns a cosmos wasm msg to provide Liquidity to the Astroport pool
// @param recipient : Astroport Asset definations defining the cw20 / native asset pair this pool will suppport
// @param native_denom : Native token type (uusd / uluna)
// @param native_amount : Native token amount to trasnfer to the astrport pool
fn build_provide_liquidity_to_lp_pool_msg(
    deps: Deps,
    assets_: [astroport::asset::Asset; 2],
    astroport_pool: Addr,
    native_denom: String,
    native_amount: Uint128,
) -> StdResult<CosmosMsg> {
    let native_coins_to_send = vec![deduct_tax(
        deps,
        Coin {
            denom: native_denom.to_string(),
            amount: native_amount.into(),
        },
    )?];

    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: astroport_pool.to_string(),
        funds: native_coins_to_send,
        msg: to_binary(&astroport::pair::ExecuteMsg::ProvideLiquidity {
            assets: assets_,
            slippage_tolerance: None,
        })?,
    }))
}
