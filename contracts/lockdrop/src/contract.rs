use std::vec;

use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::{
    entry_point, from_binary, to_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut,
    Env, MessageInfo, QueryRequest, Response, StdError, StdResult, Uint128, WasmMsg, WasmQuery,
};

use astroport_periphery::helpers::{
    build_approve_cw20_msg, build_send_cw20_token_msg, build_transfer_cw20_from_user_msg,
    build_transfer_cw20_token_msg, cw20_get_balance, is_str_present_in_vec, option_string_to_addr,
    zero_address,
};
use astroport_periphery::lockdrop::{
    CallbackMsg, ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, LockUpInfoResponse,
    PoolResponse, QueryMsg, StateResponse, UpdateConfigMsg, UserInfoResponse, WithdrawalStatus,
};

use astroport::generator::{PendingTokenResponse, QueryMsg as GenQueryMsg};
use astroport_periphery::asset::{Asset, AssetInfo, Cw20Asset, LiquidityPool, NativeAsset};
use astroport_periphery::lp_bootstrap_auction::Cw20HookMsg::DelegateAstroTokens;
use astroport_periphery::tax::deduct_tax;

use crate::state::{
    self, Config, LockupInfo, PoolInfo, State, UserInfo, ASSET_POOLS, CONFIG, LOCKUP_INFO, STATE,
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
        total_astro_returned: Uint256::zero(),
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

        ExecuteMsg::UpdateConfig { new_config } => update_config(deps, _env, info, new_config),
        ExecuteMsg::InitializePool {
            terraswap_pool,
            incentives_percent,
        } => handle_initialize_pool(deps, _env, info, terraswap_pool, incentives_percent),

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

        ExecuteMsg::WithdrawFromLockup {
            pool_identifer,
            duration,
            amount,
        } => handle_withdraw_from_lockup(deps, _env, info, pool_identifer, duration, amount),

        ExecuteMsg::DelegateAstroToAuction { amount } => {
            handle_delegate_astro_to_auction(deps, _env, info, amount)
        }
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

pub fn update_config(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    new_config: UpdateConfigMsg,
) -> StdResult<Response> {
    let mut config = CONFIG.load(deps.storage)?;

    // CHECK :: Only owner can call this function
    if info.sender != config.owner {
        return Err(StdError::generic_err("Unauthorized"));
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

/// Admin function to initialize new new LP Pool
pub fn handle_initialize_pool(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    terraswap_pool: LiquidityPool,
    // cw20_asset_addr: String,
    // native_denom: String,
    incentives_percent: Option<Decimal256>,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // CHECK ::: Only owner can call this function
    if info.sender != config.owner {
        return Err(StdError::generic_err("Unauthorized"));
    }

    // CHECK ::: Is LP Token Pool already initialized
    if is_str_present_in_vec(
        state.supported_pairs_list.clone(),
        terraswap_pool.lp_token_addr.clone().to_string(),
    ) {
        return Err(StdError::generic_err("Already supported"));
    }

    // POOL INFO :: Initialize new pool
    let mut pool_info = PoolInfo {
        terraswap_pair: terraswap_pool.clone(),
        astroport_pair: LiquidityPool {
            lp_token_addr: zero_address(),
            pair_addr: zero_address(),
            amount: Uint256::zero(),
        },
        cw20_asset: Cw20Asset {
            contract_addr: "".to_string(),
        },
        native_asset: NativeAsset {
            denom: "uuusd".to_string(),
        },
        incentives_percent: incentives_percent.unwrap_or(Decimal256::zero()),
        weighted_amount: Uint256::zero(),
        astro_global_reward_index: Decimal256::zero(),
        asset_global_reward_index: Decimal256::zero(),
        is_staked: false,
        is_migrated: false,
    };

    // QUERY :: Query terraswap pair to to fetch pool's trading Asset Pairs
    let pool_response: terraswap::pair::PoolResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: terraswap_pool.pair_addr.clone().to_string(),
            msg: to_binary(&terraswap::pair::QueryMsg::Pool {})?,
        }))?;

    // Update PoolInfo with the pool assets
    for asset_info in pool_response.assets {
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

/// @dev Admin function to enable ASTRO Claims by users. Called along-with Bootstrap Auction contract's LP Pool provide liquidity tx
pub fn handle_enable_claims(deps: DepsMut, info: MessageInfo) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // CHECK :: ONLY AUCTION CONTRACT CAN CALL THIS FUNCTION
    if info.sender != config.auction_contract_address {
        return Err(StdError::generic_err("Unauthorized"));
    }

    if state.are_claims_allowed {
        return Err(StdError::generic_err("Already allowed"));
    }
    state.are_claims_allowed = true;

    STATE.save(deps.storage, &state)?;
    Ok(Response::new().add_attribute("action", "Lockdrop::ExecuteMsg::EnableClaims"))
}

pub fn handle_migrate_liquidity(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    pool_identifer: String,
    astroport_pool_address: String,
    astroport_lp_address: String,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    // Only owner can call this function
    if info.sender != config.owner {
        return Err(StdError::generic_err("Unauthorized"));
    }

    let pool_info = ASSET_POOLS.load(deps.storage, &pool_identifer.clone().as_bytes())?;

    // Check if the liquidity has migrated or not ?
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

    // QUERY :: Get current Asset and uusd balances to calculate how much liquidity was withdrawn from the terraswap pool
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

// @dev ReceiveCW20 Hook function to increase Lockup position size when any of the supported LP Tokens are sent to the
// contract by the user
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

    let mut pool_info = ASSET_POOLS.load(deps.storage, &pool_identifer.clone().as_bytes())?;

    // ASSET POOL :: UPDATE --> SAVE
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

// @dev Function to withdraw LP Tokens from an existing Lockup position
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

    // Check :: Amount should be withing the allowed withdrawal limit bounds
    let withdrawals_status =
        calculate_max_withdrawal_percent_allowed(env.block.time.seconds(), &config);
    let max_withdrawal_allowed =
        lockup_info.lp_units_locked * withdrawals_status.max_withdrawal_percent;
    if amount > max_withdrawal_allowed {
        return Err(StdError::generic_err(format!(
            "Amount exceeds maximum allowed withdrawal limit of {} ",
            max_withdrawal_allowed
        )));
    }
    // Update withdrawal counter if the max_withdrawal_percent <= 50% ::: as it is being
    // processed post the deposit window closure
    if withdrawals_status.max_withdrawal_percent <= Decimal256::from_ratio(50u64, 100u64) {
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
pub fn handle_delegate_astro_to_auction(
    deps: DepsMut,
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

    // Check is user's total ASTRO rewards have been calculated or not, and calculate and store them in case they are not calculated yet
    if user_info.total_astro_rewards == Uint256::zero() {
        let mut total_astro_rewards = Uint256::zero();
        for lockup_id in &mut user_info.lockup_positions {
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
            // Calculate ASTRO Lockdrop rewards for the lockup position
            let weighted_lockup_balance = calculate_weight(
                lockup_info.lp_units_locked,
                lockup_info.duration,
                config.weekly_multiplier,
            );
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
        user_info.total_astro_rewards = total_astro_rewards;
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

//
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

// @dev Function to stake one of the supported LP Tokens with the Generator contract
pub fn handle_stake_lp_tokens(
    deps: DepsMut,
    env: Env,
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

    // CHECK :: Staking LP allowed only after deposit / withdraw windows have concluded
    if env.block.time.seconds()
        <= (config.init_timestamp + config.deposit_window + config.withdrawal_window)
    {
        return Err(StdError::generic_err(
            "Staking allowed after the completion of deposit / withdrawal windows",
        ));
    }

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

// @dev Function to withdraw user Rewards for a particular LP Pool
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

    // QUERY :: ARE THERE ANY REWARDS TO BE CLAIMED FOR LP STAKING > 0 ?
    // --> If unclaimed rewards > 0, add claimReward {} msg to the cosmos msg array
    let unclaimed_rewards_response: PendingTokenResponse = deps
        .querier
        .query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: config.generator_address.to_string(),
            msg: to_binary(&GenQueryMsg::PendingToken {
                lp_token: pool_info.astroport_pair.lp_token_addr.clone(),
                user: _env.contract.address.clone(),
            })
            .unwrap(),
        }))
        .unwrap();

    if unclaimed_rewards_response.pending > Uint128::zero() {
        // QUERY :: Current ASTRO & DUAL Reward Token Balance
        // -->add CallbackMsg::UpdatePoolOnDualRewardsClaim{} msg to the cosmos msg array
        cosmos_msgs.push(build_claim_dual_rewards(
            _env.contract.address.clone(),
            pool_info.astroport_pair.pair_addr.clone(),
            config.generator_address.clone(),
        )?);
        let astro_balance = cw20_get_balance(
            &deps.querier,
            config.astro_token_address,
            _env.contract.address.clone(),
        )?;
        let dual_reward_balance = cw20_get_balance(
            &deps.querier,
            deps.api
                .addr_validate(&pool_info.cw20_asset.contract_addr)?,
            _env.contract.address.clone(),
        )?;
        let update_pool_state_msg = CallbackMsg::UpdatePoolOnDualRewardsClaim {
            pool_identifer: pool_identifer.clone(),
            prev_astro_balance: astro_balance.into(),
            prev_dual_reward_balance: dual_reward_balance.into(),
        }
        .to_cosmos_msg(&_env.contract.address)?;
        cosmos_msgs.push(update_pool_state_msg);
    }

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

// @dev Function to unlock a Lockup position whose lockup duration has expired
pub fn handle_unlock_position(
    deps: DepsMut,
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
    if env.block.time.seconds() > lockup_info.unlock_timestamp {
        return Err(StdError::generic_err("Invalid LP Token Pool"));
    }

    // CHECK :: Is the lockup position valid / already unlocked or not ?
    if lockup_info.astroport_lp_units == Uint256::zero() {
        return Err(StdError::generic_err("Invalid request"));
    }

    // Check is user's total ASTRO rewards have been calculated or not, and calculate and store them in case they are not calculated yet
    let mut user_info = USER_INFO
        .may_load(deps.storage, &user_address.clone())?
        .unwrap_or_default();
    if user_info.total_astro_rewards == Uint256::zero() {
        let mut total_astro_rewards = Uint256::zero();
        for lockup_id in &mut user_info.lockup_positions {
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

            // Calculate ASTRO Lockdrop rewards for the lockup position
            let weighted_lockup_balance = calculate_weight(
                lockup_info.lp_units_locked,
                lockup_info.duration,
                config.weekly_multiplier,
            );
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
        user_info.total_astro_rewards = total_astro_rewards;
        USER_INFO.save(deps.storage, &user_address, &user_info)?;
    }

    let mut cosmos_msgs = vec![];

    // QUERY :: ARE THERE ANY REWARDS TO BE CLAIMED FOR LP STAKING > 0 ?
    // --> If unclaimed rewards > 0, add claimReward {} msg to the cosmos msg array
    let unclaimed_rewards_response: PendingTokenResponse = deps
        .querier
        .query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: config.generator_address.to_string(),
            msg: to_binary(&GenQueryMsg::PendingToken {
                lp_token: pool_info.astroport_pair.lp_token_addr.clone(),
                user: env.contract.address.clone(),
            })
            .unwrap(),
        }))
        .unwrap();

    if unclaimed_rewards_response.pending > Uint128::zero() {
        // QUERY :: Current ASTRO & DUAL Reward Token Balance
        // -->add CallbackMsg::UpdatePoolOnDualRewardsClaim{} msg to the cosmos msg array
        cosmos_msgs.push(build_claim_dual_rewards(
            env.contract.address.clone(),
            pool_info.astroport_pair.pair_addr.clone(),
            config.generator_address.clone(),
        )?);
        let astro_balance = cw20_get_balance(
            &deps.querier,
            config.astro_token_address,
            env.contract.address.clone(),
        )?;
        let dual_reward_balance = cw20_get_balance(
            &deps.querier,
            deps.api
                .addr_validate(&pool_info.cw20_asset.contract_addr)?,
            env.contract.address.clone(),
        )?;
        let update_state_msg = CallbackMsg::UpdatePoolOnDualRewardsClaim {
            pool_identifer: pool_identifer.clone(),
            prev_astro_balance: astro_balance.into(),
            prev_dual_reward_balance: dual_reward_balance.into(),
        }
        .to_cosmos_msg(&env.contract.address)?;
        cosmos_msgs.push(update_state_msg);
    }

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
            ("action", "lockdrop::ExecuteMsg::WithdrawUserRewardsForPool"),
            ("pool_identifer", &pool_identifer),
            ("user_address", &user_address.to_string()),
        ]))
}

// @dev Function to unlock a Lockup position whose lockup duration has expired
pub fn handle_force_unlock_position(
    deps: DepsMut,
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

    // CHECK :: Is the lockup position valid / already unlocked or not ?
    if lockup_info.astroport_lp_units == Uint256::zero() {
        return Err(StdError::generic_err("Invalid request"));
    }

    // Check is user's total ASTRO rewards have been calculated or not, and calculate and store them in case they are not calculated yet
    let mut user_info = USER_INFO
        .may_load(deps.storage, &user_address.clone())?
        .unwrap_or_default();
    if user_info.total_astro_rewards == Uint256::zero() {
        let mut total_astro_rewards = Uint256::zero();
        for lockup_id in &mut user_info.lockup_positions {
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

            // Calculate ASTRO Lockdrop rewards for the lockup position
            let weighted_lockup_balance = calculate_weight(
                lockup_info.lp_units_locked,
                lockup_info.duration,
                config.weekly_multiplier,
            );
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
        user_info.total_astro_rewards = total_astro_rewards;
        USER_INFO.save(deps.storage, &user_address, &user_info)?;
    }

    let mut cosmos_msgs = vec![];

    // QUERY :: ARE THERE ANY REWARDS TO BE CLAIMED FOR LP STAKING > 0 ?
    // --> If unclaimed rewards > 0, add claimReward {} msg to the cosmos msg array
    let unclaimed_rewards_response: PendingTokenResponse = deps
        .querier
        .query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: config.generator_address.to_string(),
            msg: to_binary(&GenQueryMsg::PendingToken {
                lp_token: pool_info.astroport_pair.lp_token_addr.clone(),
                user: env.contract.address.clone(),
            })
            .unwrap(),
        }))
        .unwrap();

    if unclaimed_rewards_response.pending > Uint128::zero() {
        // QUERY :: Current ASTRO & DUAL Reward Token Balance
        // -->add CallbackMsg::UpdatePoolOnDualRewardsClaim{} msg to the cosmos msg array
        cosmos_msgs.push(build_claim_dual_rewards(
            env.contract.address.clone(),
            pool_info.astroport_pair.pair_addr.clone(),
            config.generator_address.clone(),
        )?);
        let astro_balance = cw20_get_balance(
            &deps.querier,
            config.astro_token_address,
            env.contract.address.clone(),
        )?;
        let dual_reward_balance = cw20_get_balance(
            &deps.querier,
            deps.api
                .addr_validate(&pool_info.cw20_asset.contract_addr)?,
            env.contract.address.clone(),
        )?;
        let update_state_msg = CallbackMsg::UpdatePoolOnDualRewardsClaim {
            pool_identifer: pool_identifer.clone(),
            prev_astro_balance: astro_balance.into(),
            prev_dual_reward_balance: dual_reward_balance.into(),
        }
        .to_cosmos_msg(&env.contract.address)?;
        cosmos_msgs.push(update_state_msg);
    }

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
            ("action", "lockdrop::ExecuteMsg::WithdrawUserRewardsForPool"),
            ("pool_identifer", &pool_identifer),
            ("user_address", &user_address.to_string()),
        ]))
}

// //----------------------------------------------------------------------------------------
// // Callback Functions
// //----------------------------------------------------------------------------------------

// CALLBACK :: CALLED AFTER ASTRO / DUAL REWARDS ARE CLAIMED FROM THE GENERATOR CONTRACT :: UPDATES THE REWARD_INDEXES OF THE POOL
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

// CALLBACK ::
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
        compute_lockup_position_accrued_astro_rewards(&pool_info, &mut lockup_info);

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
        // COSMOSMSG :: Transfers ASTRO (that user received as rewards for this lockup position) from user to itself
        if force_unlock {
            let mut state = STATE.load(deps.storage)?;
            let transfer_astro_msg = build_transfer_cw20_from_user_msg(
                config.astro_token_address.clone().to_string(),
                user_address.clone().to_string(),
                env.contract.address.to_string(),
                lockup_info.astro_rewards,
            )?;
            state.total_astro_returned += lockup_info.astro_rewards;
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
        // remove_lockup_pos_from_user_info(&mut user_info, lockup_id.clone());
        pool_info.astroport_pair.amount =
            pool_info.astroport_pair.amount - lockup_info.lp_units_locked;

        // Save updated pool state
        ASSET_POOLS.save(deps.storage, &pool_identifer.as_bytes(), &pool_info)?;
    }

    // Save updated state
    LOCKUP_INFO.save(deps.storage, lockup_id.clone().as_bytes(), &lockup_info)?;

    Ok(Response::new()
        .add_messages(cosmos_msgs)
        .add_attributes(vec![
            (
                "action",
                "lockdrop::CallbackMsg::WithdrawPendingRewardsForLockup",
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
        ]))
}

pub fn callback_deposit_liquidity_in_astroport(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
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

    // ASSET DEFINATION
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

pub fn callback_update_pool_state_after_migration(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    pool_identifer: String,
    astroport_pool: LiquidityPool,
    prev_astroport_lp_balance: Uint128,
) -> StdResult<Response> {
    let mut pool_info = ASSET_POOLS.load(deps.storage, &pool_identifer.as_bytes())?;

    // QUERY :: Get current Asset and uusd balances to calculate how much liquidity was withdrawn from the terraswap pool
    let astroport_lp_balance = cw20_get_balance(
        &deps.querier,
        astroport_pool.lp_token_addr.clone(),
        env.contract.address.clone(),
    )?;
    let astroport_lp_minted = astroport_lp_balance - prev_astroport_lp_balance;

    // POOL INFO :: Update state
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

// Calculate pending ASTRO rewards for a particular LOCKUP Position
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

// Calculate pending DUAL rewards for a particular LOCKUP Position
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
        total_astro_returned: state.total_astro_returned,
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

// //----------------------------------------------------------------------------------------
// // HELPERS
// //----------------------------------------------------------------------------------------

/// true if deposits are allowed
fn is_deposit_open(current_timestamp: u64, config: &Config) -> bool {
    let deposits_opened_till = config.init_timestamp + config.deposit_window;
    (current_timestamp >= config.init_timestamp) && (deposits_opened_till >= current_timestamp)
}

fn calculate_max_withdrawal_percent_allowed(
    current_timestamp: u64,
    config: &Config,
) -> WithdrawalStatus {
    let withdrawal_cutoff_init_point = config.init_timestamp + config.deposit_window;

    // 100% withdrawals allowed
    if current_timestamp <= withdrawal_cutoff_init_point {
        return WithdrawalStatus {
            max_withdrawal_percent: Decimal256::from_ratio(100u32, 100u32),
            more_withdrawals_allowed: false,
        };
    }

    // 50% withdrawals allowed
    let withdrawal_cutoff_sec_point =
        withdrawal_cutoff_init_point + (config.withdrawal_window / 2u64);
    if current_timestamp <= withdrawal_cutoff_sec_point {
        return WithdrawalStatus {
            max_withdrawal_percent: Decimal256::from_ratio(50u32, 100u32),
            more_withdrawals_allowed: true,
        };
    }

    // max withdrawal allowed decreasing linearly from 50% to 0% vs time elapsed
    let withdrawal_cutoff_final = withdrawal_cutoff_sec_point + (config.withdrawal_window / 2u64);
    if current_timestamp < withdrawal_cutoff_final {
        let slope = Decimal256::from_ratio(50u64, config.withdrawal_window / 2u64);
        let time_elapsed = current_timestamp - withdrawal_cutoff_sec_point;
        return WithdrawalStatus {
            max_withdrawal_percent: Decimal256::from_ratio(time_elapsed, 1u64) * slope,
            more_withdrawals_allowed: true,
        };
    }
    // Withdrawals not allowed
    else {
        return WithdrawalStatus {
            max_withdrawal_percent: Decimal256::from_ratio(0u32, 100u32),
            more_withdrawals_allowed: true,
        };
    }
}

// /// Helper function. Updates ASTRO Lockdrop rewards that a user will get based on the weighted LP deposits across all of this lockup positions
// fn update_user_astro_incentives(deps: &mut DepsMut, config: &Config, user_info: &mut UserInfo) {
//     if user_info.total_astro_rewards == Uint256::zero() {
//         return;
//     }

//     let mut total_astro_rewards = Uint256::zero();
//     for lockup_id in &mut user_info.lockup_positions {
//         let mut lockup_info = LOCKUP_INFO.load(deps.storage, lockup_id.as_bytes()).unwrap();
//         let pool_info = ASSET_POOLS.load(deps.storage, &lockup_info.pool_lp_token_addr ).unwrap();
//         let weighted_lockup_balance = calculate_weight(lockup_info.lp_units_locked, lockup_info.duration, config.weekly_multiplier);
//         lockup_info.astro_rewards = config.lockdrop_incentives * Decimal256::from_ratio(weighted_lockup_balance, pool_info.weighted_amount);
//         LOCKUP_INFO.save(deps.storage, lockup_id.as_bytes(), &lockup_info);
//         total_astro_rewards += lockup_info.astro_rewards;
//     }

//     user_info.total_astro_rewards = total_astro_rewards;
//     user_info.unclaimed_astro_rewards = total_astro_rewards;
// }

// /// true if withdrawals are allowed
// fn is_withdraw_open(current_timestamp: u64, config: &Config) -> bool {
//     let withdrawals_opened_till = config.init_timestamp + config.withdrawal_window;
//     (current_timestamp >= config.init_timestamp) && (withdrawals_opened_till >= current_timestamp)
// }

/// Returns the timestamp when the lockup will get unlocked
fn calculate_unlock_timestamp(config: &Config, duration: u64) -> u64 {
    config.init_timestamp
        + config.deposit_window
        + config.withdrawal_window
        + (duration * config.seconds_per_week)
}

// // Calculate Lockdrop Reward
// fn calculate_lockdrop_reward(
//     deposited_ust: Uint256,
//     duration: u64,
//     config: &Config,
//     total_deposits_weight: Uint256,
// ) -> Uint256 {
//     if total_deposits_weight == Uint256::zero() {
//         return Uint256::zero();
//     }
//     let amount_weight = calculate_weight(deposited_ust, duration, config.weekly_multiplier);
//     config.lockdrop_incentives * Decimal256::from_ratio(amount_weight, total_deposits_weight)
// }

// Returns effective weight for the amount to be used for calculating airdrop rewards
fn calculate_weight(amount: Uint256, duration: u64, weekly_multiplier: Decimal256) -> Uint256 {
    let duration_weighted_amount = amount * Uint256::from(duration);
    duration_weighted_amount * weekly_multiplier
}

// // native coins
// fn get_denom_amount_from_coins(coins: &[Coin], denom: &str) -> Uint256 {
//     coins
//         .iter()
//         .find(|c| c.denom == denom)
//         .map(|c| Uint256::from(c.amount))
//         .unwrap_or_else(Uint256::zero)
// }

// //-----------------------------
// // MARS REWARDS COMPUTATION
// //-----------------------------

// Accrue ASTRO & DUAL rewards by updating the pool's reward index
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

//
pub fn calculate_lockup_balance_post_migration(
    user_lp_units: Uint256,
    total_terraswap_lp_locked: Uint256,
    total_astroport_lp_locked: Uint256,
) -> Uint256 {
    let percent_of_total = Decimal256::from_ratio(user_lp_units, total_terraswap_lp_locked);
    percent_of_total * total_astroport_lp_locked
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

// //-----------------------------
// // COSMOS_MSGs
// //-----------------------------

/// Helper Function. Returns CosmosMsg which unstakes LP Tokens from the generator contract
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

/// Helper Function. Returns CosmosMsg to facilitate LP Tokens staking with the generator contract
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

//
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

//
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
