use std::{cmp::Ordering, convert::TryInto};

use cosmwasm_std::{
    attr, entry_point, from_binary, to_binary, Addr, Binary, Coin, CosmosMsg, Decimal, Decimal256,
    Deps, DepsMut, Env, MessageInfo, Order, Response, StdError, StdResult, Uint128, Uint256,
    WasmMsg,
};

use astroport_periphery::lockdrop::{
    CallbackMsg, ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, LockUpInfoResponse,
    MigrationInfo, PoolResponse, QueryMsg, StateResponse, UpdateConfigMsg, UserInfoResponse,
};

use astroport::generator::{
    ExecuteMsg as GenExecuteMsg, PendingTokenResponse, QueryMsg as GenQueryMsg, RewardInfoResponse,
};
use astroport_periphery::auction::Cw20HookMsg::DepositAstroTokens;
use cw_storage_plus::U64Key;

use crate::state::{
    Config, LockupInfo, PoolInfo, State, ASSET_POOLS, CONFIG, LOCKUP_INFO, STATE, USER_INFO,
};
use cw20::{BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg, Cw20ReceiveMsg};

const SECONDS_PER_WEEK: u64 = 7 * 24 * 60 * 60;

//----------------------------------------------------------------------------------------
// Entry Points
//----------------------------------------------------------------------------------------

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    // CHECK :: init_timestamp needs to be valid
    if env.block.time.seconds() > msg.init_timestamp {
        return Err(StdError::generic_err(format!(
            "Invalid init_timestamp. Current timestamp : {}",
            env.block.time.seconds()
        )));
    }

    // CHECK :: min_lock_duration , max_lock_duration need to be valid (min_lock_duration < max_lock_duration)
    if msg.max_lock_duration < msg.min_lock_duration {
        return Err(StdError::generic_err("Invalid Lockup durations"));
    }

    let config = Config {
        owner: msg
            .owner
            .map(|v| deps.api.addr_validate(&v))
            .transpose()?
            .unwrap_or(info.sender),
        astro_token: None,
        auction_contract: None,
        generator: None,
        init_timestamp: msg.init_timestamp,
        deposit_window: msg.deposit_window,
        withdrawal_window: msg.withdrawal_window,
        min_lock_duration: msg.min_lock_duration,
        max_lock_duration: msg.max_lock_duration,
        weekly_multiplier: msg.weekly_multiplier,
        weekly_divider: msg.weekly_divider,
        lockdrop_incentives: None,
    };

    let state = State {
        total_incentives_share: 0,
        total_astro_delegated: Uint128::zero(),
        are_claims_allowed: false,
    };

    CONFIG.save(deps.storage, &config)?;
    STATE.save(deps.storage, &state)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),

        ExecuteMsg::UpdateConfig { new_config } => {
            handle_update_config(deps, env, info, new_config)
        }
        ExecuteMsg::InitializePool {
            terraswap_lp_token,
            incentives_share,
        } => handle_initialize_pool(deps, env, info, terraswap_lp_token, incentives_share),
        ExecuteMsg::UpdatePool {
            terraswap_lp_token,
            incentives_share,
        } => handle_update_pool(deps, env, info, terraswap_lp_token, incentives_share),

        ExecuteMsg::MigrateLiquidity {
            terraswap_lp_token,
            astroport_pool_addr,
        } => handle_migrate_liquidity(deps, env, info, terraswap_lp_token, astroport_pool_addr),

        ExecuteMsg::StakeLpTokens { terraswap_lp_token } => {
            handle_stake_lp_tokens(deps, env, info, terraswap_lp_token)
        }
        ExecuteMsg::EnableClaims {} => handle_enable_claims(deps, info),
        ExecuteMsg::DelegateAstroToAuction { amount } => {
            handle_delegate_astro_to_auction(deps, env, info, amount)
        }
        ExecuteMsg::WithdrawFromLockup {
            terraswap_lp_token,
            duration,
            amount,
        } => handle_withdraw_from_lockup(deps, env, info, terraswap_lp_token, duration, amount),
        ExecuteMsg::ClaimRewardsAndOptionallyUnlock {
            terraswap_lp_token,
            duration,
            withdraw_lp_stake,
        } => handle_claim_rewards_and_unlock_for_lockup(
            deps,
            env,
            info,
            terraswap_lp_token,
            duration,
            withdraw_lp_stake,
        ),

        ExecuteMsg::Callback(msg) => _handle_callback(deps, env, info, msg),
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, StdError> {
    let user_address = deps.api.addr_validate(&cw20_msg.sender)?;
    let amount = cw20_msg.amount;

    match from_binary(&cw20_msg.msg)? {
        Cw20HookMsg::IncreaseLockup { duration } => {
            handle_increase_lockup(deps, env, info, user_address, duration, amount)
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
            terraswap_lp_token,
            prev_astro_balance,
            prev_proxy_reward_balance,
        } => update_pool_on_dual_rewards_claim(
            deps,
            env,
            terraswap_lp_token,
            prev_astro_balance,
            prev_proxy_reward_balance,
        ),
        CallbackMsg::WithdrawUserLockupRewardsCallback {
            terraswap_lp_token,
            user_address,
            duration,
            withdraw_lp_stake,
        } => callback_withdraw_user_rewards_for_lockup_optional_withdraw(
            deps,
            env,
            terraswap_lp_token,
            user_address,
            duration,
            withdraw_lp_stake,
        ),
        CallbackMsg::WithdrawLiquidityFromTerraswapCallback {
            terraswap_lp_token,
            astroport_pool,
            prev_assets,
        } => callback_deposit_liquidity_in_astroport(
            deps,
            env,
            terraswap_lp_token,
            astroport_pool,
            prev_assets,
        ),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State {} => to_binary(&query_state(deps)?),
        QueryMsg::Pool { terraswap_lp_token } => to_binary(&query_pool(deps, terraswap_lp_token)?),
        QueryMsg::UserInfo { address } => to_binary(&query_user_info(deps, env, address)?),
        QueryMsg::LockUpInfo {
            user_address,
            terraswap_lp_token,
            duration,
        } => to_binary(&query_lockup_info(
            deps,
            &env,
            &user_address,
            terraswap_lp_token,
            duration,
        )?),
    }
}

//----------------------------------------------------------------------------------------
// Handle Functions
//----------------------------------------------------------------------------------------

/// @dev Admin function to update Configuration parameters
/// @param new_config : Same as UpdateConfigMsg struct
pub fn handle_update_config(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    new_config: UpdateConfigMsg,
) -> StdResult<Response> {
    let mut config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;
    let mut messages: Vec<WasmMsg> = vec![];
    let mut attributes = vec![attr("action", "update_config")];

    // CHECK :: Only owner can call this function
    if info.sender != config.owner {
        return Err(StdError::generic_err("Unauthorized"));
    }

    // CHECK ::: Configuration can only be updated before claims are enabled
    if state.are_claims_allowed {
        return Err(StdError::generic_err(
            "ASTRO tokens are live. Configuration cannot be updated now",
        ));
    }

    if let Some(owner) = new_config.owner {
        config.owner = deps.api.addr_validate(&owner)?;
    };

    if let Some(astro_addr) = new_config.astro_token_address {
        config.astro_token = Some(deps.api.addr_validate(&astro_addr)?);
    };

    if let Some(auction) = new_config.auction_contract_address {
        config.auction_contract = Some(deps.api.addr_validate(&auction)?);
    };

    if let Some(generator) = new_config.generator_address {
        config.generator = Some(deps.api.addr_validate(&generator)?);
    }

    if let Some(new_incentives) = new_config.lockdrop_incentives {
        if let Some(astro_addr) = &config.astro_token {
            if env.block.time.seconds()
                >= config.init_timestamp + config.deposit_window + config.withdrawal_window
            {
                return Err(StdError::generic_err("ASTRO is already being distributed"));
            };
            let prev_incentives = config.lockdrop_incentives.unwrap_or_default();
            match prev_incentives.cmp(&new_incentives) {
                Ordering::Equal => {}
                Ordering::Greater => {
                    let amount = prev_incentives - new_incentives;
                    messages.push(WasmMsg::Execute {
                        contract_addr: astro_addr.to_string(),
                        funds: vec![],
                        msg: to_binary(&Cw20ExecuteMsg::Transfer {
                            recipient: info.sender.to_string(),
                            amount,
                        })?,
                    });
                    attributes.push(attr("incentives_returned", amount));
                }
                Ordering::Less => {
                    let amount = new_incentives - prev_incentives;
                    messages.push(WasmMsg::Execute {
                        contract_addr: astro_addr.to_string(),
                        funds: vec![],
                        msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                            owner: info.sender.to_string(),
                            recipient: env.contract.address.to_string(),
                            amount,
                        })?,
                    });
                    attributes.push(attr("incentives_received", amount));
                }
            };
            config.lockdrop_incentives = Some(new_incentives);
        } else {
            return Err(StdError::generic_err("Astro contract wasn't specified!"));
        }
    }

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new()
        .add_attributes(attributes)
        .add_messages(messages))
}

/// @dev Admin function to initialize new LP Pool
/// @param terraswap_lp_token : terraswap LP token address
/// @param incentives_share : parameter defining share of total ASTRO incentives are allocated for this pool
pub fn handle_initialize_pool(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    terraswap_lp_token: String,
    incentives_share: u64,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // CHECK ::: Only owner can call this function
    if info.sender != config.owner {
        return Err(StdError::generic_err("Unauthorized"));
    }

    // CHECK :: Is lockdrop deposit window closed
    if env.block.time.seconds() >= config.init_timestamp + config.deposit_window {
        return Err(StdError::generic_err(
            "Pools cannot be added post deposit window closure",
        ));
    }

    let terraswap_lp_token = deps.api.addr_validate(&terraswap_lp_token)?;

    // CHECK ::: Is LP Token Pool already initialized
    if ASSET_POOLS
        .may_load(deps.storage, &terraswap_lp_token)?
        .is_some()
    {
        return Err(StdError::generic_err("Already supported"));
    }

    let terraswap_pool = {
        let res: Option<cw20::MinterResponse> = deps
            .querier
            .query_wasm_smart(&terraswap_lp_token, &Cw20QueryMsg::Minter {})?;
        deps.api
            .addr_validate(&res.expect("No minter for the LP token!").minter)?
    };

    // POOL INFO :: Initialize new pool
    let pool_info = PoolInfo {
        terraswap_pool,
        terraswap_amount_in_lockups: Default::default(),
        migration_info: None,
        incentives_share,
        weighted_amount: Default::default(),
        generator_astro_per_share: Default::default(),
        generator_proxy_per_share: Default::default(),
        is_staked: false,
    };
    // STATE UPDATE :: Save state and PoolInfo
    ASSET_POOLS.save(deps.storage, &terraswap_lp_token, &pool_info)?;

    state.total_incentives_share += incentives_share;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "initialize_pool"),
        attr("terraswap_lp_token", terraswap_lp_token),
        attr("incentives_share", incentives_share.to_string()),
    ]))
}

/// @dev Admin function to update LP Pool Configuration
/// @param terraswap_lp_token : Parameter to identify the pool. Equals pool's terraswap Lp token address
/// @param incentives_share : parameter defining share of total ASTRO incentives are allocated for this pool
pub fn handle_update_pool(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    terraswap_lp_token: String,
    incentives_share: u64,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // CHECK ::: Only owner can call this function
    if info.sender != config.owner {
        return Err(StdError::generic_err("Unauthorized"));
    }

    // CHECK :: Is lockdrop deposit window closed
    if env.block.time.seconds() >= config.init_timestamp + config.deposit_window {
        return Err(StdError::generic_err(
            "Pools cannot be updated post deposit window closure",
        ));
    }

    let terraswap_lp_token = deps.api.addr_validate(&terraswap_lp_token)?;

    // CHECK ::: Is LP Token Pool initialized
    let mut pool_info = ASSET_POOLS.load(deps.storage, &terraswap_lp_token)?;

    // update total incentives
    state.total_incentives_share =
        state.total_incentives_share - pool_info.incentives_share + incentives_share;

    // Update Pool Incentives
    pool_info.incentives_share = incentives_share;

    ASSET_POOLS.save(deps.storage, &terraswap_lp_token, &pool_info)?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "update_pool"),
        attr("terraswap_lp_token", terraswap_lp_token),
        attr("set_incentives_share", incentives_share.to_string()),
    ]))
}

/// @dev Admin function to enable ASTRO Claims by users. Called along-with Bootstrap Auction contract's LP Pool provide liquidity tx
pub fn handle_enable_claims(deps: DepsMut, info: MessageInfo) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // CHECK :: ONLY AUCTION CONTRACT CAN CALL THIS FUNCTION
    if let Some(auction) = config.auction_contract {
        if info.sender != auction {
            return Err(StdError::generic_err("Unauthorized"));
        }
    } else {
        return Err(StdError::generic_err("Auction contract hasn't been set!"));
    }

    // CHECK ::: Claims are only enabled once
    if state.are_claims_allowed {
        return Err(StdError::generic_err("Already allowed"));
    }
    state.are_claims_allowed = true;

    STATE.save(deps.storage, &state)?;
    Ok(Response::new().add_attribute("action", "allow_claims"))
}

/// @dev Admin function to migrate Liquidity from Terraswap to Astroport
/// @param terraswap_lp_token : Parameter to identify the pool
/// @param astroport_pool_address : Astroport Pool address to which the liquidity is to be migrated
pub fn handle_migrate_liquidity(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    terraswap_lp_token: String,
    astroport_pool_addr: String,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    // Only owner can call this function
    if info.sender != config.owner {
        return Err(StdError::generic_err("Unauthorized"));
    }

    // CHECK :: may the liquidity be migrated or not ?
    if env.block.time.seconds()
        < config.init_timestamp + config.deposit_window + config.withdrawal_window
    {
        return Err(StdError::generic_err(
            "Deposit / Withdrawal windows not closed",
        ));
    }

    let terraswap_lp_token = deps.api.addr_validate(&terraswap_lp_token)?;
    let astroport_pool = deps.api.addr_validate(&astroport_pool_addr)?;

    let mut pool_info = ASSET_POOLS.load(deps.storage, &terraswap_lp_token)?;

    // CHECK :: has the liquidity already been migrated or not ?
    if pool_info.migration_info.is_some() {
        return Err(StdError::generic_err("Liquidity already migrated"));
    }

    let mut cosmos_msgs: Vec<CosmosMsg> = vec![];

    let lp_balance: BalanceResponse = deps.querier.query_wasm_smart(
        &terraswap_lp_token,
        &Cw20QueryMsg::Balance {
            address: env.contract.address.to_string(),
        },
    )?;

    // COSMOS MSG :: WITHDRAW LIQUIDITY FROM TERRASWAP
    let msg = WasmMsg::Execute {
        contract_addr: terraswap_lp_token.to_string(),
        funds: vec![],
        msg: to_binary(&Cw20ExecuteMsg::Send {
            contract: pool_info.terraswap_pool.to_string(),
            msg: to_binary(&terraswap::pair::Cw20HookMsg::WithdrawLiquidity {})?,
            amount: lp_balance.balance,
        })?,
    };
    cosmos_msgs.push(msg.into());

    let terraswap_lp_info: terraswap::asset::PairInfo = deps.querier.query_wasm_smart(
        &pool_info.terraswap_pool,
        &terraswap::pair::QueryMsg::Pair {},
    )?;

    let mut assets = vec![];

    for asset_info in terraswap_lp_info.asset_infos {
        assets.push(terraswap::asset::Asset {
            amount: match &asset_info {
                terraswap::asset::AssetInfo::NativeToken { denom } => {
                    terraswap::querier::query_balance(
                        &deps.querier,
                        env.contract.address.clone(),
                        denom.clone(),
                    )?
                }
                terraswap::asset::AssetInfo::Token { contract_addr } => {
                    terraswap::querier::query_token_balance(
                        &deps.querier,
                        deps.api.addr_validate(contract_addr)?,
                        env.contract.address.clone(),
                    )?
                }
            },
            info: asset_info,
        })
    }

    // COSMOS MSG :: CALLBACK AFTER LIQUIDITY WITHDRAWAL
    let update_state_msg = CallbackMsg::WithdrawLiquidityFromTerraswapCallback {
        terraswap_lp_token: terraswap_lp_token.clone(),
        astroport_pool: astroport_pool.clone(),
        prev_assets: assets.try_into().unwrap(),
    }
    .to_cosmos_msg(&env)?;
    cosmos_msgs.push(update_state_msg);

    let astroport_lp_token = {
        let msg = astroport::pair::QueryMsg::Pair {};
        let res: astroport::asset::PairInfo =
            deps.querier.query_wasm_smart(&astroport_pool, &msg)?;
        res.liquidity_token
    };

    pool_info.migration_info = Some(MigrationInfo {
        astroport_lp_token,
        terraswap_migrated_amount: lp_balance.balance,
    });
    ASSET_POOLS.save(deps.storage, &terraswap_lp_token, &pool_info)?;

    Ok(Response::new().add_messages(cosmos_msgs))
}

/// @dev Function to stake one of the supported LP Tokens with the Generator contract
/// @params terraswap_lp_token : Pool's terraswap LP token address whose Astroport LP tokens are to be staked
pub fn handle_stake_lp_tokens(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    terraswap_lp_token: String,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    let mut cosmos_msgs = vec![];

    // CHECK ::: Only owner can call this function
    if info.sender != config.owner {
        return Err(StdError::generic_err("Unauthorized"));
    }

    let terraswap_lp_token = deps.api.addr_validate(&terraswap_lp_token)?;

    // CHECK ::: Is LP Token Pool supported or not ?
    let mut pool_info = ASSET_POOLS.load(deps.storage, &terraswap_lp_token)?;

    let MigrationInfo {
        astroport_lp_token, ..
    } = pool_info
        .migration_info
        .as_ref()
        .expect("Terraswap liquidity hasn't migrated yet!");

    let amount = {
        let res: BalanceResponse = deps.querier.query_wasm_smart(
            astroport_lp_token,
            &Cw20QueryMsg::Balance {
                address: env.contract.address.to_string(),
            },
        )?;
        res.balance
    };

    let generator = config.generator.expect("Generator address hasn't set yet!");

    cosmos_msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: astroport_lp_token.to_string(),
        funds: vec![],
        msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
            spender: generator.to_string(),
            amount,
            expires: None,
        })?,
    }));

    cosmos_msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: generator.to_string(),
        funds: vec![],
        msg: to_binary(&astroport::generator::ExecuteMsg::Deposit {
            lp_token: astroport_lp_token.clone(),
            amount,
        })?,
    }));

    // UPDATE STATE & SAVE
    pool_info.is_staked = true;
    ASSET_POOLS.save(deps.storage, &terraswap_lp_token, &pool_info)?;

    Ok(Response::new()
        .add_messages(cosmos_msgs)
        .add_attributes(vec![
            attr("action", "stake_to_generator"),
            attr("terraswap_lp_token", terraswap_lp_token),
            attr("astroport_lp_amount", amount),
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
    amount: Uint128,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let terraswap_lp_token = info.sender;

    // CHECK ::: LP Token supported or not ?
    let mut pool_info = ASSET_POOLS.load(deps.storage, &terraswap_lp_token)?;

    // CHECK :: Lockdrop deposit window open
    let current_time = env.block.time.seconds();
    if current_time < config.init_timestamp
        || current_time >= config.init_timestamp + config.deposit_window
    {
        return Err(StdError::generic_err("Deposit window closed"));
    }

    // CHECK :: Valid Lockup Duration
    if duration > config.max_lock_duration || duration < config.min_lock_duration {
        return Err(StdError::generic_err(format!(
            "Lockup duration needs to be between {} and {}",
            config.min_lock_duration, config.max_lock_duration
        )));
    }

    pool_info.weighted_amount += calculate_weight(amount, duration, &config);
    pool_info.terraswap_amount_in_lockups += amount;

    let lockup_key = (&terraswap_lp_token, &user_address, U64Key::new(duration));

    LOCKUP_INFO.update::<_, StdError>(deps.storage, lockup_key, |li| {
        if let Some(mut li) = li {
            li.lp_units_locked = li.lp_units_locked.checked_add(amount)?;
            Ok(li)
        } else {
            Ok(LockupInfo {
                lp_units_locked: amount,
                astro_rewards: None,
                unlock_timestamp: config.init_timestamp
                    + config.deposit_window
                    + config.withdrawal_window
                    + (duration * SECONDS_PER_WEEK),
                generator_astro_debt: Uint128::zero(),
                generator_proxy_debt: Uint128::zero(),
                withdrawal_flag: false,
            })
        }
    })?;

    // SAVE UPDATED STATE
    ASSET_POOLS.save(deps.storage, &terraswap_lp_token, &pool_info)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "increase_lockup_position"),
        attr("terraswap_lp_token", terraswap_lp_token),
        attr("user", user_address),
        attr("duration", duration.to_string()),
        attr("amount", amount),
    ]))
}

/// @dev Function to withdraw LP Tokens from an existing Lockup position
/// @param terraswap_lp_token : Terraswap Lp token address to identify the LP pool against which withdrawal has to be made
/// @param duration : Duration of the lockup position from which withdrawal is to be made
/// @param amount : Number of LP tokens to be withdrawn
pub fn handle_withdraw_from_lockup(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    terraswap_lp_token: String,
    duration: u64,
    amount: Uint128,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    // CHECK :: Valid Withdraw Amount
    if amount.is_zero() {
        return Err(StdError::generic_err("Invalid withdrawal request"));
    }

    let terraswap_lp_token = deps.api.addr_validate(&terraswap_lp_token)?;

    // CHECK ::: LP Token supported or not ?
    let mut pool_info = ASSET_POOLS.load(deps.storage, &terraswap_lp_token)?;

    // Retrieve Lockup position
    let user_address = info.sender;
    let lockup_key = (&terraswap_lp_token, &user_address, U64Key::new(duration));
    let mut lockup_info = LOCKUP_INFO.load(deps.storage, lockup_key.clone())?;

    // CHECK :: Has user already withdrawn LP tokens once post the deposit window closure state
    if lockup_info.withdrawal_flag {
        return Err(StdError::generic_err(
            "Withdrawal already happened. No more withdrawals accepted",
        ));
    }

    // Check :: Amount should be within the allowed withdrawal limit bounds
    let max_withdrawal_percent =
        calculate_max_withdrawal_percent_allowed(env.block.time.seconds(), &config);
    let max_withdrawal_allowed = lockup_info.lp_units_locked * max_withdrawal_percent;
    if amount > max_withdrawal_allowed {
        return Err(StdError::generic_err(format!(
            "Amount exceeds maximum allowed withdrawal limit of {}",
            max_withdrawal_allowed
        )));
    }

    // Update withdrawal flag after the deposit window
    if env.block.time.seconds() >= config.init_timestamp + config.deposit_window {
        lockup_info.withdrawal_flag = true;
    }

    // STATE :: RETRIEVE --> UPDATE
    lockup_info.lp_units_locked -= amount;
    pool_info.weighted_amount -= calculate_weight(amount, duration, &config);
    pool_info.terraswap_amount_in_lockups -= amount;

    // Remove Lockup position from the list of user positions if Lp_Locked balance == 0
    if lockup_info.lp_units_locked.is_zero() {
        LOCKUP_INFO.remove(deps.storage, lockup_key);
    } else {
        LOCKUP_INFO.save(deps.storage, lockup_key, &lockup_info)?;
    }

    // SAVE Updated States
    ASSET_POOLS.save(deps.storage, &terraswap_lp_token, &pool_info)?;

    // COSMOS_MSG ::TRANSFER WITHDRAWN LP Tokens
    let msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: terraswap_lp_token.to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: user_address.to_string(),
            amount,
        })?,
        funds: vec![],
    });

    Ok(Response::new().add_message(msg).add_attributes(vec![
        attr("action", "withdraw_from_lockup"),
        attr("terraswap_lp_token", terraswap_lp_token),
        attr("user_address", user_address),
        attr("duration", duration.to_string()),
        attr("amount", amount),
    ]))
}

// @dev Function to delegate part of the ASTRO rewards to be used for LP Bootstrapping via auction
/// @param amount : Number of ASTRO to delegate
pub fn handle_delegate_astro_to_auction(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let user_address = info.sender;

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
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    // If user's total ASTRO rewards == 0 :: We update all of the user's lockup positions to calculate ASTRO rewards and for each alongwith their equivalent Astroport LP Shares
    if user_info.total_astro_rewards == Uint128::zero() {
        user_info.total_astro_rewards = update_user_lockup_positions_and_calc_rewards(
            deps.branch(),
            &config,
            &state,
            &user_address,
        )?;
    }

    // CHECK :: ASTRO to delegate cannot exceed user's unclaimed ASTRO balance
    let max_deletatable_astro = user_info
        .total_astro_rewards
        .checked_sub(user_info.delegated_astro_rewards)?;

    if amount > max_deletatable_astro {
        return Err(StdError::generic_err(format!("ASTRO to delegate cannot exceed user's unclaimed ASTRO balance. ASTRO to delegate = {}, Max delegatable ASTRO = {}. ",amount, max_deletatable_astro)));
    }

    // UPDATE STATE
    user_info.delegated_astro_rewards += amount;
    state.total_astro_delegated += amount;

    // SAVE UPDATED STATE
    STATE.save(deps.storage, &state)?;
    USER_INFO.save(deps.storage, &user_address, &user_info)?;

    // COSMOS_MSG ::Delegate ASTRO to the LP Bootstrapping via Auction contract
    let msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config
            .astro_token
            .expect("Astro token contract hasn't been set yet!")
            .to_string(),
        funds: vec![],
        msg: to_binary(&Cw20ExecuteMsg::Send {
            contract: config
                .auction_contract
                .expect("Auction contract hasn't been set yet!")
                .to_string(),
            msg: to_binary(&DepositAstroTokens {
                user_address: user_address.clone(),
            })?,
            amount,
        })?,
    });

    Ok(Response::new().add_message(msg).add_attributes(vec![
        attr("action", "delegate_astro_to_auction"),
        attr("user_address", user_address),
        attr("amount", amount),
    ]))
}

/// @dev Function to claim user Rewards for a particular Lockup position
/// @param terraswap_lp_token : Terraswap LP token to identify the LP pool whose Token is locked in the lockup position
/// @param duration : Lockup duration (number of weeks)
/// @param @withdraw_lp_stake : Boolean value indicating if the LP tokens are to be withdrawn or not
pub fn handle_claim_rewards_and_unlock_for_lockup(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    terraswap_lp_token: String,
    duration: u64,
    withdraw_lp_stake: bool,
) -> StdResult<Response> {
    let state = STATE.load(deps.storage)?;

    if !state.are_claims_allowed {
        return Err(StdError::generic_err("Reward claim not allowed"));
    }

    let config = CONFIG.load(deps.storage)?;
    let user_address = info.sender;

    let terraswap_lp_token = deps.api.addr_validate(&terraswap_lp_token)?;

    // CHECK ::: Is LP Token Pool supported or not ?
    let pool_info = ASSET_POOLS.load(deps.storage, &terraswap_lp_token)?;

    // Check is there lockup or not ?
    let lockup_key = (&terraswap_lp_token, &user_address, U64Key::new(duration));
    let mut lockup_info = LOCKUP_INFO.load(deps.storage, lockup_key.clone())?;
    if lockup_info.astro_rewards.is_none() {
        let weighted_lockup_balance =
            calculate_weight(lockup_info.lp_units_locked, duration, &config);
        lockup_info.astro_rewards = Some(calculate_astro_incentives_for_lockup(
            weighted_lockup_balance,
            pool_info.weighted_amount,
            pool_info.incentives_share,
            state.total_incentives_share,
            config
                .lockdrop_incentives
                .expect("Lockdrop incentives should be set!"),
        ));
        LOCKUP_INFO.save(deps.storage, lockup_key, &lockup_info)?;
    }

    // CHECK :: Can the Lockup position be unlocked or not ?
    if withdraw_lp_stake {
        if env.block.time.seconds() < lockup_info.unlock_timestamp {
            return Err(StdError::generic_err(format!(
                "{} seconds to unlock",
                lockup_info.unlock_timestamp - env.block.time.seconds()
            )));
        }
    }

    let mut cosmos_msgs = vec![];

    if let Some(MigrationInfo {
        astroport_lp_token, ..
    }) = &pool_info.migration_info
    {
        if pool_info.is_staked {
            let generator = config
                .generator
                .expect("Generator should be set at this moment!");

            // QUERY :: Check if there are any pending staking rewards
            let pending_rewards: PendingTokenResponse = deps.querier.query_wasm_smart(
                &generator,
                &GenQueryMsg::PendingToken {
                    lp_token: astroport_lp_token.clone(),
                    user: env.contract.address.clone(),
                },
            )?;

            if !pending_rewards.pending.is_zero()
                || (pending_rewards.pending_on_proxy.is_some()
                    && !pending_rewards.pending_on_proxy.unwrap().is_zero())
            {
                let rwi: RewardInfoResponse = deps.querier.query_wasm_smart(
                    &generator,
                    &GenQueryMsg::RewardInfo {
                        lp_token: astroport_lp_token.clone(),
                    },
                )?;

                let astro_balance = {
                    let res: BalanceResponse = deps.querier.query_wasm_smart(
                        rwi.base_reward_token,
                        &Cw20QueryMsg::Balance {
                            address: env.contract.address.to_string(),
                        },
                    )?;
                    res.balance
                };

                let proxy_reward_balance = match rwi.proxy_reward_token {
                    Some(proxy_reward_token) => {
                        let res: BalanceResponse = deps.querier.query_wasm_smart(
                            proxy_reward_token,
                            &Cw20QueryMsg::Balance {
                                address: env.contract.address.to_string(),
                            },
                        )?;
                        Some(res.balance)
                    }
                    None => None,
                };

                cosmos_msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: generator.to_string(),
                    funds: vec![],
                    msg: to_binary(&GenExecuteMsg::Withdraw {
                        lp_token: astroport_lp_token.clone(),
                        amount: Uint128::zero(),
                    })?,
                }));

                cosmos_msgs.push(
                    CallbackMsg::UpdatePoolOnDualRewardsClaim {
                        terraswap_lp_token: terraswap_lp_token.clone(),
                        prev_astro_balance: astro_balance,
                        prev_proxy_reward_balance: proxy_reward_balance,
                    }
                    .to_cosmos_msg(&env)?,
                );
            }
        }
    }

    cosmos_msgs.push(
        CallbackMsg::WithdrawUserLockupRewardsCallback {
            terraswap_lp_token,
            user_address,
            duration,
            withdraw_lp_stake,
        }
        .to_cosmos_msg(&env)?,
    );

    Ok(Response::new().add_messages(cosmos_msgs))
}

//----------------------------------------------------------------------------------------
// Callback Functions
//----------------------------------------------------------------------------------------

/// @dev CALLBACK Function to update contract state after dual staking rewards are claimed from the generator contract
/// @param terraswap_lp_token : Pool identifier to identify the LP pool whose rewards have been claimed
/// @param prev_astro_balance : Contract's ASTRO token balance before claim
/// @param prev_dual_reward_balance : Contract's Generator Proxy reward token balance before claim
pub fn update_pool_on_dual_rewards_claim(
    deps: DepsMut,
    env: Env,
    terraswap_lp_token: Addr,
    prev_astro_balance: Uint128,
    prev_proxy_reward_balance: Option<Uint128>,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut pool_info = ASSET_POOLS.load(deps.storage, &terraswap_lp_token)?;

    let generator = config.generator.expect("Generator hasn't been set yet!");
    let MigrationInfo {
        astroport_lp_token, ..
    } = pool_info
        .migration_info
        .as_ref()
        .expect("Pool should be migrated!");

    let rwi: RewardInfoResponse = deps.querier.query_wasm_smart(
        &generator,
        &GenQueryMsg::RewardInfo {
            lp_token: astroport_lp_token.clone(),
        },
    )?;

    let lp_balance: Uint128 = deps.querier.query_wasm_smart(
        &generator,
        &GenQueryMsg::Deposit {
            lp_token: astroport_lp_token.clone(),
            user: env.contract.address.clone(),
        },
    )?;

    let base_reward_received;
    // Increment claimed Astro rewards per LP share
    pool_info.generator_astro_per_share = pool_info.generator_astro_per_share + {
        let res: BalanceResponse = deps.querier.query_wasm_smart(
            rwi.base_reward_token,
            &Cw20QueryMsg::Balance {
                address: env.contract.address.to_string(),
            },
        )?;
        base_reward_received = res.balance - prev_astro_balance;
        Decimal::from_ratio(base_reward_received, lp_balance)
    };

    // Increment claimed Proxy rewards per LP share
    let mut proxy_reward_received = Uint128::zero();
    pool_info.generator_proxy_per_share = pool_info.generator_proxy_per_share + {
        match rwi.proxy_reward_token {
            Some(proxy_reward_token) => {
                let res: BalanceResponse = deps.querier.query_wasm_smart(
                    proxy_reward_token,
                    &Cw20QueryMsg::Balance {
                        address: env.contract.address.to_string(),
                    },
                )?;
                proxy_reward_received = res.balance
                    - prev_proxy_reward_balance.expect("Should be passed into this function!");
                Decimal::from_ratio(proxy_reward_received, lp_balance)
            }
            None => Decimal::zero(),
        }
    };

    // SAVE UPDATED STATE OF THE POOL
    ASSET_POOLS.save(deps.storage, &terraswap_lp_token, &pool_info)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "update_generator_dual_rewards"),
        attr("terraswap_lp_token", terraswap_lp_token),
        attr("astro_reward_received", base_reward_received),
        attr("proxy_reward_received", proxy_reward_received),
        attr(
            "generator_astro_per_share",
            pool_info.generator_astro_per_share.to_string(),
        ),
        attr(
            "generator_proxy_per_share",
            pool_info.generator_proxy_per_share.to_string(),
        ),
    ]))
}

/// @dev CALLBACK Function to withdraw user rewards and LP Tokens after claims / unlocks
/// @param terraswap_lp_token : Pool identifier to identify the LP pool
/// @param user_address : User address who is claiming the rewards / unlocking his lockup position
/// @param duration : Duration of the lockup for which rewards have been claimed / position unlocked
/// @param withdraw_lp_stake : Boolean value indicating if the ASTRO LP Tokens are to be sent to the user or not
pub fn callback_withdraw_user_rewards_for_lockup_optional_withdraw(
    deps: DepsMut,
    env: Env,
    terraswap_lp_token: Addr,
    user_address: Addr,
    duration: u64,
    withdraw_lp_stake: bool,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut pool_info = ASSET_POOLS.load(deps.storage, &terraswap_lp_token)?;
    let lockup_key = (&terraswap_lp_token, &user_address, U64Key::new(duration));
    let mut lockup_info = LOCKUP_INFO.load(deps.storage, lockup_key.clone())?;

    let mut user_info = USER_INFO
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    let mut cosmos_msgs = vec![];
    let mut attributes = vec![
        attr("action", "withdraw_rewards_and_or_unlock"),
        attr("terraswap_lp_token", &terraswap_lp_token),
        attr("user_address", &user_address),
        attr("duration", duration.to_string()),
    ];

    let astro_rewards = lockup_info
        .astro_rewards
        .expect("Astro reward should be already set!");

    // Transfers claimable one time ASTRO rewards to the user that the user gets for all his lock
    if let Some(astro_token) = &config.astro_token {
        if !user_info.astro_transferred {
            // Calculating how much Astro user can claim (from total one time reward)
            let total_claimable_astro_rewards = user_info
                .total_astro_rewards
                .checked_sub(user_info.delegated_astro_rewards)?;
            if total_claimable_astro_rewards > Uint128::zero() {
                cosmos_msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: astro_token.to_string(),
                    funds: vec![],
                    msg: to_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: user_address.to_string(),
                        amount: total_claimable_astro_rewards,
                    })?,
                }));
            }
            user_info.astro_transferred = true;
            attributes.push(attr(
                "total_claimable_astro_reward",
                total_claimable_astro_rewards,
            ));
            USER_INFO.save(deps.storage, &user_address, &user_info)?;
        }
    }

    if let Some(MigrationInfo {
        astroport_lp_token, ..
    }) = &pool_info.migration_info
    {
        // Calculate Astro LP share for the lockup position
        let astroport_lp_amount: Uint128 = {
            let balance: Uint128 = if pool_info.is_staked {
                deps.querier.query_wasm_smart(
                    &config
                        .generator
                        .as_ref()
                        .expect("Should be set!")
                        .to_string(),
                    &GenQueryMsg::Deposit {
                        lp_token: astroport_lp_token.clone(),
                        user: env.contract.address.clone(),
                    },
                )?
            } else {
                let res: BalanceResponse = deps.querier.query_wasm_smart(
                    astroport_lp_token,
                    &Cw20QueryMsg::Balance {
                        address: env.contract.address.to_string(),
                    },
                )?;
                res.balance
            };
            (lockup_info.lp_units_locked.full_mul(balance)
                / Uint256::from(pool_info.terraswap_amount_in_lockups))
            .try_into()?
        };

        // If Astro LP tokens are staked with Astro generator
        if pool_info.is_staked {
            let generator = config.generator.expect("Generator should be set");

            let rwi: RewardInfoResponse = deps.querier.query_wasm_smart(
                &generator,
                &GenQueryMsg::RewardInfo {
                    lp_token: astroport_lp_token.clone(),
                },
            )?;

            // Calculate claimable Astro staking rewards for this lockup
            let total_lockup_astro_rewards =
                pool_info.generator_astro_per_share * astroport_lp_amount;
            let pending_astro_rewards =
                total_lockup_astro_rewards - lockup_info.generator_astro_debt;
            lockup_info.generator_astro_debt = total_lockup_astro_rewards;

            // If claimable Astro staking rewards > 0, claim them
            if pending_astro_rewards > Uint128::zero() {
                cosmos_msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: rwi.base_reward_token.to_string(),
                    funds: vec![],
                    msg: to_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: user_address.to_string(),
                        amount: pending_astro_rewards,
                    })?,
                }));
            }
            attributes.push(attr("generator_astro_reward", pending_astro_rewards));

            // If this LP token is getting dual incentives
            if let Some(proxy_reward_token) = rwi.proxy_reward_token {
                // Calculate claimable proxy staking rewards for this lockup
                let total_lockup_proxy_rewards =
                    pool_info.generator_proxy_per_share * astroport_lp_amount;
                let pending_proxy_rewards =
                    total_lockup_proxy_rewards - lockup_info.generator_proxy_debt;
                lockup_info.generator_proxy_debt = total_lockup_proxy_rewards;

                // If claimable proxy staking rewards > 0, claim them
                if pending_proxy_rewards > Uint128::zero() {
                    cosmos_msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
                        contract_addr: proxy_reward_token.to_string(),
                        funds: vec![],
                        msg: to_binary(&Cw20ExecuteMsg::Transfer {
                            recipient: user_address.to_string(),
                            amount: pending_proxy_rewards,
                        })?,
                    }));
                }
                attributes.push(attr("generator_proxy_reward", pending_proxy_rewards));
            }

            //  COSMOSMSG :: If LP Tokens are staked, we unstake the amount which needs to be returned to the user
            if withdraw_lp_stake {
                cosmos_msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: generator.to_string(),
                    funds: vec![],
                    msg: to_binary(&GenExecuteMsg::Withdraw {
                        lp_token: astroport_lp_token.clone(),
                        amount: astroport_lp_amount,
                    })?,
                }));
            }
        }

        if withdraw_lp_stake {
            // COSMOSMSG :: Returns LP units locked by the user in the current lockup position
            cosmos_msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: astroport_lp_token.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: user_address.to_string(),
                    amount: astroport_lp_amount,
                })?,
                funds: vec![],
            }));
            pool_info.terraswap_amount_in_lockups -= lockup_info.lp_units_locked;
            ASSET_POOLS.save(deps.storage, &terraswap_lp_token, &pool_info)?;

            attributes.push(attr("astroport_lp_unlocked", astroport_lp_amount));
            LOCKUP_INFO.remove(deps.storage, lockup_key);
        } else {
            LOCKUP_INFO.save(deps.storage, lockup_key, &lockup_info)?;
        }
    } else if withdraw_lp_stake {
        return Err(StdError::generic_err("Pool should be migrated!"));
    }

    Ok(Response::new()
        .add_messages(cosmos_msgs)
        .add_attributes(attributes))
}

/// @dev CALLBACK Function to deposit Liquidity in Astroport after its withdrawn from terraswap
/// @param terraswap_lp_token : Pool identifier to identify the LP pool
/// @param astroport_pool : Astroport Pool details to which the liquidity is to be migrated
/// @param prev_assets : balances of terraswap pool assets before liquidity was withdrawn
pub fn callback_deposit_liquidity_in_astroport(
    deps: DepsMut,
    env: Env,
    terraswap_lp_token: Addr,
    astroport_pool: Addr,
    prev_assets: [terraswap::asset::Asset; 2],
) -> StdResult<Response> {
    let mut cosmos_msgs = vec![];

    let mut assets = vec![];
    let mut coins = vec![];

    for prev_asset in prev_assets {
        match prev_asset.info {
            terraswap::asset::AssetInfo::NativeToken { denom } => {
                let mut new_asset = astroport::asset::Asset {
                    info: astroport::asset::AssetInfo::NativeToken {
                        denom: denom.clone(),
                    },
                    amount: terraswap::querier::query_balance(
                        &deps.querier,
                        env.contract.address.clone(),
                        denom.clone(),
                    )?
                    .checked_sub(prev_asset.amount)?,
                };

                new_asset.amount -= new_asset.compute_tax(&deps.querier)?;

                coins.push(Coin {
                    denom,
                    amount: new_asset.amount,
                });
                assets.push(new_asset);
            }
            terraswap::asset::AssetInfo::Token { contract_addr } => {
                let amount = terraswap::querier::query_token_balance(
                    &deps.querier,
                    deps.api.addr_validate(&contract_addr)?,
                    env.contract.address.clone(),
                )?
                .checked_sub(prev_asset.amount)?;

                cosmos_msgs.push(
                    WasmMsg::Execute {
                        contract_addr: contract_addr.to_string(),
                        funds: vec![],
                        msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                            spender: astroport_pool.to_string(),
                            expires: None,
                            amount,
                        })?,
                    }
                    .into(),
                );

                assets.push(astroport::asset::Asset {
                    info: astroport::asset::AssetInfo::Token {
                        contract_addr: deps.api.addr_validate(&contract_addr)?,
                    },
                    amount,
                });
            }
        }
    }

    cosmos_msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: astroport_pool.to_string(),
        funds: coins,
        msg: to_binary(&astroport::pair::ExecuteMsg::ProvideLiquidity {
            assets: assets.clone().try_into().unwrap(),
            slippage_tolerance: None,
            auto_stack: None,
        })?,
    }));

    Ok(Response::new()
        .add_messages(cosmos_msgs)
        .add_attributes(vec![
            attr("action", "migrate_liquidity_to_astroport"),
            attr("terraswap_lp_token", terraswap_lp_token),
            attr("astroport_pool", astroport_pool),
            attr("liquidity", format!("{}-{}", assets[0], assets[1])),
        ]))
}

// //----------------------------------------------------------------------------------------
// // Query Functions
// //----------------------------------------------------------------------------------------

/// @dev Returns the contract's configuration
pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;

    Ok(ConfigResponse {
        owner: config.owner,
        auction_contract: config.auction_contract,
        generator: config.generator,
        astro_token: config.astro_token,
        init_timestamp: config.init_timestamp,
        deposit_window: config.deposit_window,
        withdrawal_window: config.withdrawal_window,
        min_lock_duration: config.min_lock_duration,
        max_lock_duration: config.max_lock_duration,
        weekly_multiplier: config.weekly_multiplier,
        weekly_divider: config.weekly_divider,
        lockdrop_incentives: config.lockdrop_incentives,
    })
}

/// @dev Returns the contract's State
pub fn query_state(deps: Deps) -> StdResult<StateResponse> {
    let state: State = STATE.load(deps.storage)?;
    Ok(StateResponse {
        total_incentives_share: state.total_incentives_share,
        total_astro_delegated: state.total_astro_delegated,
        are_claims_allowed: state.are_claims_allowed,
        supported_pairs_list: ASSET_POOLS
            .keys(deps.storage, None, None, Order::Ascending)
            .map(|v| Addr::unchecked(String::from_utf8(v).expect("Addr deserialization error!")))
            .collect(),
    })
}

/// @dev Returns the pool's State
pub fn query_pool(deps: Deps, terraswap_lp_token: String) -> StdResult<PoolResponse> {
    let terraswap_lp_token = deps.api.addr_validate(&terraswap_lp_token)?;
    let pool_info: PoolInfo = ASSET_POOLS.load(deps.storage, &terraswap_lp_token)?;
    Ok(PoolResponse {
        terraswap_pool: pool_info.terraswap_pool,
        terraswap_amount_in_lockups: pool_info.terraswap_amount_in_lockups,
        migration_info: pool_info.migration_info,
        incentives_share: pool_info.incentives_share,
        weighted_amount: pool_info.weighted_amount,
        generator_astro_per_share: pool_info.generator_astro_per_share,
        generator_proxy_per_share: pool_info.generator_proxy_per_share,
        is_staked: pool_info.is_staked,
    })
}

/// @dev Returns summarized details regarding the user
pub fn query_user_info(deps: Deps, env: Env, user: String) -> StdResult<UserInfoResponse> {
    let user_address = deps.api.addr_validate(&user)?;
    let mut user_info = USER_INFO
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    let mut total_astro_rewards = Uint128::zero();
    let mut lockup_infos = vec![];

    for pool in ASSET_POOLS
        .keys(deps.storage, None, None, Order::Ascending)
        .map(|v| Addr::unchecked(String::from_utf8(v).expect("Addr deserialization error!")))
    {
        for duration in LOCKUP_INFO
            .prefix((&pool, &user_address))
            .keys(deps.storage, None, None, Order::Ascending)
            .map(|v| u64::from_be_bytes(v.try_into().expect("Duration deserialization error!")))
        {
            let lockup_info = query_lockup_info(deps, &env, &user, pool.to_string(), duration)?;
            if let Some(astro_rewards) = lockup_info.astro_rewards {
                total_astro_rewards += astro_rewards;
            }
            lockup_infos.push(lockup_info);
        }
    }

    if user_info.total_astro_rewards == Uint128::zero() {
        user_info.total_astro_rewards = total_astro_rewards;
    }

    Ok(UserInfoResponse {
        total_astro_rewards: user_info.total_astro_rewards,
        delegated_astro_rewards: user_info.delegated_astro_rewards,
        astro_transferred: user_info.astro_transferred,
        lockup_infos,
    })
}

/// @dev Returns summarized details regarding the user
pub fn query_lockup_info(
    deps: Deps,
    env: &Env,
    user_address: &str,
    terraswap_lp_token: String,
    duration: u64,
) -> StdResult<LockUpInfoResponse> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;
    let terraswap_lp_token = deps.api.addr_validate(&terraswap_lp_token)?;
    let user_address = deps.api.addr_validate(user_address)?;
    let lockup_key = (&terraswap_lp_token, &user_address, U64Key::new(duration));
    let mut pool_info = ASSET_POOLS.load(deps.storage, &terraswap_lp_token)?;
    let mut lockup_info = LOCKUP_INFO.load(deps.storage, lockup_key)?;

    let mut pool_astroport_lp_units = Uint128::zero();
    let mut lockup_astroport_lp_units: Option<Uint128> = None;
    let mut astroport_lp_token_opt: Option<Addr> = None;
    let mut claimable_generator_astro_debt = Uint128::zero();
    let mut claimable_generator_proxy_debt = Uint128::zero();
    if let Some(MigrationInfo {
        astroport_lp_token, ..
    }) = pool_info.migration_info.clone()
    {
        lockup_astroport_lp_units = Some({
            // Query Astro LP Tokens balance for the pool
            pool_astroport_lp_units = if pool_info.is_staked {
                deps.querier.query_wasm_smart(
                    &config
                        .generator
                        .as_ref()
                        .expect("Should be set!")
                        .to_string(),
                    &GenQueryMsg::Deposit {
                        lp_token: astroport_lp_token.clone(),
                        user: env.contract.address.clone(),
                    },
                )?
            } else {
                let res: BalanceResponse = deps.querier.query_wasm_smart(
                    &astroport_lp_token,
                    &Cw20QueryMsg::Balance {
                        address: env.contract.address.to_string(),
                    },
                )?;
                res.balance
            };
            // Calculate Lockup Astro LP shares
            (lockup_info
                .lp_units_locked
                .full_mul(pool_astroport_lp_units)
                / Uint256::from(pool_info.terraswap_amount_in_lockups))
            .try_into()?
        });
        astroport_lp_token_opt = Some(astroport_lp_token);
    }

    // Calculate currently expected ASTRO Rewards if not finalized
    if lockup_info.astro_rewards.is_none() {
        let pool_info = ASSET_POOLS.load(deps.storage, &terraswap_lp_token)?;
        let weighted_lockup_balance =
            calculate_weight(lockup_info.lp_units_locked, duration, &config);
        lockup_info.astro_rewards = Some(calculate_astro_incentives_for_lockup(
            weighted_lockup_balance,
            pool_info.weighted_amount,
            pool_info.incentives_share,
            state.total_incentives_share,
            config
                .lockdrop_incentives
                .expect("Lockdrop incentives should be set!"),
        ));
    }

    // If LP tokens are staked, calculate the rewards claimable by the user for this lockup position
    if let Some(MigrationInfo {
        astroport_lp_token, ..
    }) = &pool_info.migration_info
    {
        if pool_info.is_staked
            && lockup_astroport_lp_units.is_some()
            && !lockup_astroport_lp_units.unwrap().is_zero()
        {
            let generator = config
                .generator
                .expect("Generator should be set at this moment!");

            // QUERY :: Check if there are any pending staking rewards
            let pending_rewards: PendingTokenResponse = deps.querier.query_wasm_smart(
                &generator,
                &GenQueryMsg::PendingToken {
                    lp_token: astroport_lp_token.clone(),
                    user: env.contract.address.clone(),
                },
            )?;

            // Calculate claimable Astro staking rewards for this lockup
            pool_info.generator_astro_per_share = pool_info.generator_astro_per_share
                + Decimal::from_ratio(pending_rewards.pending, pool_astroport_lp_units);

            let total_lockup_astro_rewards =
                pool_info.generator_astro_per_share * lockup_astroport_lp_units.unwrap();
            claimable_generator_astro_debt =
                total_lockup_astro_rewards - lockup_info.generator_astro_debt;

            // Calculate claimable Proxy staking rewards for this lockup
            if !pending_rewards.pending_on_proxy.is_none() {
                pool_info.generator_proxy_per_share = pool_info.generator_proxy_per_share
                    + Decimal::from_ratio(
                        pending_rewards.pending_on_proxy.unwrap(),
                        pool_astroport_lp_units,
                    );
                let total_lockup_proxy_rewards =
                    pool_info.generator_proxy_per_share * lockup_astroport_lp_units.unwrap();
                claimable_generator_proxy_debt =
                    total_lockup_proxy_rewards - lockup_info.generator_proxy_debt;
            }
        }
    }

    Ok(LockUpInfoResponse {
        terraswap_lp_token,
        lp_units_locked: lockup_info.lp_units_locked,
        withdrawal_flag: lockup_info.withdrawal_flag,
        astro_rewards: lockup_info.astro_rewards,
        generator_astro_debt: lockup_info.generator_astro_debt,
        claimable_generator_astro_debt: claimable_generator_astro_debt,
        generator_proxy_debt: lockup_info.generator_proxy_debt,
        claimable_generator_proxy_debt: claimable_generator_proxy_debt,
        unlock_timestamp: lockup_info.unlock_timestamp,
        astroport_lp_units: lockup_astroport_lp_units,
        astroport_lp_token: astroport_lp_token_opt,
        duration,
    })
}

//----------------------------------------------------------------------------------------
// HELPERS :: BOOLEANS & COMPUTATIONS (Rewards, Indexes etc)
//----------------------------------------------------------------------------------------

///  @dev Helper function to calculate maximum % of LP balances deposited that can be withdrawn
/// @params current_timestamp : Current block timestamp
/// @params config : Contract configuration
fn calculate_max_withdrawal_percent_allowed(current_timestamp: u64, config: &Config) -> Decimal {
    let withdrawal_cutoff_init_point = config.init_timestamp + config.deposit_window;

    // Deposit window :: 100% withdrawals allowed
    if current_timestamp < withdrawal_cutoff_init_point {
        return Decimal::from_ratio(100u32, 100u32);
    }

    let withdrawal_cutoff_second_point =
        withdrawal_cutoff_init_point + (config.withdrawal_window / 2u64);
    // Deposit window closed, 1st half of withdrawal window :: 50% withdrawals allowed
    if current_timestamp <= withdrawal_cutoff_second_point {
        return Decimal::from_ratio(50u32, 100u32);
    }

    // max withdrawal allowed decreasing linearly from 50% to 0% vs time elapsed
    let withdrawal_cutoff_final = withdrawal_cutoff_init_point + config.withdrawal_window;
    //  Deposit window closed, 2nd half of withdrawal window :: max withdrawal allowed decreases linearly from 50% to 0% vs time elapsed
    if current_timestamp < withdrawal_cutoff_final {
        let time_left = withdrawal_cutoff_final - current_timestamp;
        Decimal::from_ratio(
            50u64 * time_left,
            100u64 * (withdrawal_cutoff_final - withdrawal_cutoff_second_point),
        )
    }
    // Withdrawals not allowed
    else {
        Decimal::from_ratio(0u32, 100u32)
    }
}

/// @dev Helper function to calculate ASTRO rewards for a particular Lockup position
/// @params lockup_weighted_balance : Lockup position's weighted terraswap LP balance
/// @params total_weighted_amount : Total weighted terraswap LP balance of the Pool
/// @params pool_incentives_share : Share of total ASTRO incentives allocated to this pool
/// @params total_incentives_share: Calculated total incentives share for allocating among pools
/// @params total_lockdrop_incentives : Total ASTRO incentives to be distributed among Lockdrop participants
pub fn calculate_astro_incentives_for_lockup(
    lockup_weighted_balance: Uint256,
    total_weighted_amount: Uint256,
    pool_incentives_share: u64,
    total_incentives_share: u64,
    total_lockdrop_incentives: Uint128,
) -> Uint128 {
    (Decimal256::from_ratio(
        Uint256::from(pool_incentives_share) * lockup_weighted_balance,
        Uint256::from(total_incentives_share) * total_weighted_amount,
    ) * total_lockdrop_incentives.into())
    .try_into()
    .unwrap()
}

/// @dev Helper function. Returns effective weight for the amount to be used for calculating lockdrop rewards
/// @params amount : Number of LP tokens
/// @params duration : Number of weeks
/// @config : Config with weekly multiplier and divider
fn calculate_weight(amount: Uint128, duration: u64, config: &Config) -> Uint256 {
    let lock_weight = Decimal256::one()
        + Decimal256::from_ratio(
            (duration - 1) * config.weekly_multiplier,
            config.weekly_divider,
        );
    lock_weight * amount.into()
}

//-----------------------------------------------------------
// HELPER FUNCTIONS :: UPDATE STATE
//-----------------------------------------------------------

/// @dev Function to calculate ASTRO rewards for each of the user position
/// @params configuration struct
/// @params user Info struct
/// Returns user's total ASTRO rewards
fn update_user_lockup_positions_and_calc_rewards(
    deps: DepsMut,
    config: &Config,
    state: &State,
    user_address: &Addr,
) -> StdResult<Uint128> {
    let mut total_astro_rewards = Uint128::zero();

    let mut keys: Vec<(Addr, u64)> = vec![];

    for pool_key in ASSET_POOLS
        .keys(deps.storage, None, None, Order::Ascending)
        .map(|v| Addr::unchecked(String::from_utf8(v).expect("Addr deserialization error!")))
    {
        for duration in LOCKUP_INFO
            .prefix((&pool_key, user_address))
            .keys(deps.storage, None, None, Order::Ascending)
            .map(|v| u64::from_be_bytes(v.try_into().expect("Duration deserialization error!")))
        {
            keys.push((pool_key.clone(), duration));
        }
    }
    for (pool, duration) in keys {
        let pool_info = ASSET_POOLS.load(deps.storage, &pool)?;
        let lockup_key = (&pool, user_address, U64Key::new(duration));
        let mut lockup_info = LOCKUP_INFO.load(deps.storage, lockup_key.clone())?;

        let lockup_astro_rewards = if let Some(astro_reward) = lockup_info.astro_rewards {
            astro_reward
        } else {
            // Weighted lockup balance (using terraswap LP units to calculate as pool's total weighted balance is calculated on terraswap LP deposits summed over each deposit tx)
            let weighted_lockup_balance =
                calculate_weight(lockup_info.lp_units_locked, duration, config);

            // Calculate ASTRO Lockdrop rewards for the lockup position
            let lockup_astro_rewards = calculate_astro_incentives_for_lockup(
                weighted_lockup_balance,
                pool_info.weighted_amount,
                pool_info.incentives_share,
                state.total_incentives_share,
                config
                    .lockdrop_incentives
                    .expect("Lockdrop incentives should be set!"),
            );

            lockup_info.astro_rewards = Some(lockup_astro_rewards);
            LOCKUP_INFO.save(deps.storage, lockup_key, &lockup_info)?;
            lockup_astro_rewards
        };

        // Save updated Lockup state
        total_astro_rewards += lockup_astro_rewards;
    }

    Ok(total_astro_rewards)
}
