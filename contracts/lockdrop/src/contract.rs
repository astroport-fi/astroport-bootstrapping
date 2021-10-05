use std::vec;

use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::{
    entry_point, to_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Env,
    MessageInfo, QueryRequest, Response, StdError, StdResult, Uint128, WasmMsg, WasmQuery, from_binary
};

use astroport_periphery::helpers::{cw20_get_balance, zero_address, build_transfer_cw20_token_msg,build_send_cw20_token_msg,build_transfer_cw20_from_user_msg, option_string_to_addr } ;
use astroport_periphery::lockdrop::{ 
    PoolType, WithdrawalStatus, Cw20HookMsg, CallbackMsg, ConfigResponse, ExecuteMsg, StateResponse,PoolResponse, InstantiateMsg,
    LockUpInfoResponse, QueryMsg, UserInfoResponse, UpdateConfigMsg
};
use astroport::generator::{ PendingTokenResponse, QueryMsg as GenQueryMsg};
use astroport_periphery::lp_bootstrap_auction::Cw20HookMsg::{DelegateAstroTokens } ;


use crate::state::{CONFIG, Config, LOCKUP_INFO, LockupInfo, ASSET_POOLS, PoolInfo, STATE, State, USER_INFO, UserInfo};
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
        auction_contract_address: option_string_to_addr(deps.api, msg.auction_contract_address, zero_address())?,
        generator_address: option_string_to_addr(deps.api, msg.generator_address, zero_address())?,
        astro_token_address: option_string_to_addr(deps.api, msg.astro_token_address, zero_address())?,
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
        supported_lp_tokens: vec![]
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
        ExecuteMsg::MigrateLiquidity {
            lp_token_address,
            astroport_pool_address,
            astroport_lp_address
        } => handle_migrate_liquidity(deps, _env, info, lp_token_address, astroport_pool_address, astroport_lp_address),

        ExecuteMsg::UpdateConfig { new_config } => update_config(deps, _env, info, new_config),
        ExecuteMsg::InitializePool { 
            lp_token_addr,
            pool_addr,
            incentives_percent,
            pool_type
         } => handle_initialize_pool(deps, _env, info, lp_token_addr, pool_addr, incentives_percent, pool_type),
         ExecuteMsg::StakeLpTokens { lp_token_address } => handle_stake_lp_tokens(deps, _env,  info, lp_token_address),
         ExecuteMsg::UnstakeLpTokens { lp_token_address } => handle_unstake_lp_tokens(deps, _env,  info, lp_token_address),
         ExecuteMsg::EnableClaims {} => handle_enable_claims(deps,  info),
  
         ExecuteMsg::WithdrawFromLockup { lp_token_address, duration, amount } => {
            handle_withdraw_from_lockup(deps, _env, info, lp_token_address, duration, amount)
        },

        ExecuteMsg::DelegateAstroToAuction { amount } => { handle_delegate_astro_to_auction(deps, _env, info, amount)},
        ExecuteMsg::WithdrawUserRewardsForLockup { lp_token_address, duration } => handle_withdraw_user_rewards_for_lockup(deps, _env, info, lp_token_address, duration),

        ExecuteMsg::UnlockPosition { lp_token_address, duration } => handle_unlock_position(deps, _env, info, lp_token_address, duration),
        ExecuteMsg::ForceUnlockPosition { lp_token_address, duration } => handle_force_unlock_position(deps, _env, info, lp_token_address, duration),

        // ExecuteMsg::Unlock { duration } => try_unlock_position(deps, _env, info, duration),
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
        Cw20HookMsg::IncreaseLockup { 
                        duration
                    } => {
                        handle_increase_lockup(
                            deps,
                            env,
                            info,
                            user_address_,
                            duration,
                            amount.into()
                        )
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
        return Err(StdError::generic_err("callbacks cannot be invoked externally"));
    }
    match msg {
        CallbackMsg::UpdatePoolOnDualRewardsClaim {
            lp_token_addr,
            prev_astro_balance,
            prev_dual_reward_balance,
        } => update_pool_on_dual_rewards_claim(deps, env, lp_token_addr, prev_astro_balance, prev_dual_reward_balance),
        CallbackMsg::WithdrawUserLockupRewardsCallback { 
            user_address, 
            lp_token_addr, 
            duration ,
            withdraw_lp_stake
        } => callback_withdraw_user_rewards_for_lockup_optional_withdraw(deps, env, user_address, lp_token_addr, duration,withdraw_lp_stake),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State {} => to_binary(&query_state(deps)?),
        QueryMsg::Pool { lp_token_addr } => to_binary(&query_pool(deps, lp_token_addr)?),
        QueryMsg::UserInfo { address } => to_binary(&query_user_info(deps, _env, address)?),
        QueryMsg::LockUpInfo { user_address, lp_token_address, duration } => {
            to_binary(&query_lockup_info(deps, user_address, lp_token_address, duration)?)
        }
        QueryMsg::LockUpInfoWithId { lockup_id } => {
            to_binary(&query_lockup_info_with_id(deps, lockup_id)?)
        }
    }
}

//----------------------------------------------------------------------------------------
// Handle Functions
//----------------------------------------------------------------------------------------


/// Admin function to initialize new new LP Pool
pub fn handle_initialize_pool(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    lp_token_addr: String,
    pool_addr: String,
    incentives_percent: Decimal256,
    pool_type: PoolType
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // CHECK ::: Only owner can call this function
    if info.sender != config.owner {
        return Err(StdError::generic_err("Only owner can update configuration"));
    }

    // CHECK ::: Is LP Token Pool already initialized
    if is_str_present_in_vec(state.supported_lp_tokens.clone(), lp_token_addr.clone()) {
        return Err(StdError::generic_err("Already supported"));
    }    

    let lp_token_addr_ = deps.api.addr_validate(&lp_token_addr)?;

    // POOL INFO :: RETRIEVE --> CHECK IF DOESNT ALREADY EXIST --> UPDATE
    let mut pool_info = ASSET_POOLS.may_load(deps.storage, &lp_token_addr_.clone())?.unwrap_or_default();    

    pool_info.lp_token_addr = lp_token_addr_;
    pool_info.pool_addr = deps.api.addr_validate(&pool_addr)?;
    pool_info.incentives_percent = incentives_percent;
    match pool_type { 
        PoolType::Terraswap {} => { pool_info.pool_type = "terraswap".to_string() },
        PoolType::Astroport {} => { pool_info.pool_type = "astroport".to_string() },
        _  => {
            return Err(StdError::generic_err("Invalid pool type"));
        }
    }

    state.supported_lp_tokens.push(lp_token_addr.clone());
    ASSET_POOLS.save(deps.storage, &pool_info.lp_token_addr, &pool_info)?;
    STATE.save(deps.storage,  &state)?;

    Ok(Response::new().add_attributes(vec![
        ("action", "lockdrop::ExecuteMsg::InitializePool"),
        ("lp_token_addr", &lp_token_addr.to_string()),
        ("pool_addr", &pool_addr.to_string()),
        ("incentives_percent", incentives_percent.to_string().as_str()),
        ("pool_type", pool_info.pool_type.as_str()),
    ]))
}


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
    config.owner = option_string_to_addr(deps.api,new_config.owner,config.owner)?;
    config.auction_contract_address = option_string_to_addr(deps.api,new_config.auction_contract_address,config.auction_contract_address)?;
    config.generator_address = option_string_to_addr(deps.api,new_config.generator_address,config.generator_address)?;
    config.astro_token_address = option_string_to_addr(deps.api,new_config.astro_token_address,config.astro_token_address)?;

    // UPDATE :: LOCKDROP INCENTIVES IF PROVIDED
    config.lockdrop_incentives = new_config.lockdrop_incentives.unwrap_or(config.lockdrop_incentives);

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "lockdrop::ExecuteMsg::UpdateConfig"))
}



/// @dev Admin function to enable ASTRO Claims by users. Called along-with Bootstrap Auction contract's LP Pool provide liquidity tx 
pub fn handle_enable_claims( deps: DepsMut, info: MessageInfo) -> StdResult<Response> { 
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
    lp_token_address: String, 
    astroport_pool_address: String, 
    astroport_lp_address: String
)  -> StdResult<Response> {

    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.owner {
        return Err(StdError::generic_err("Only owner can update configuration"));
    }


    Ok()


}









// @dev ReceiveCW20 Hook function to increase Lockup position size when any of the supported LP Tokens are sent to the ]
// contract by the user
pub fn handle_increase_lockup(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    user_address: Addr,
    duration: u64,
    amount: Uint256
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;
    let lp_token_addr = info.sender.clone();
    let mut pool_info = ASSET_POOLS.load(deps.storage, &lp_token_addr )?;

    // CHECK ::: LP Token supported or not ?
    if !is_str_present_in_vec(state.supported_lp_tokens, lp_token_addr.to_string().clone()) {
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

    // ASSET POOL :: UPDATE --> SAVE
    pool_info.total_lp_units_before_migration += amount;
    pool_info.weighted_amount +=  calculate_weight(amount, duration, config.weekly_multiplier);

    // LOCKUP INFO :: RETRIEVE --> UPDATE
    let lockup_id = user_address.to_string().clone() + &lp_token_addr.to_string().clone() + &duration.to_string();
    let mut lockup_info = LOCKUP_INFO.may_load(deps.storage, lockup_id.clone().as_bytes())?.unwrap_or_default();
    if lockup_info.lp_units_locked > Uint256::zero() {
        lockup_info.duration = duration;
        lockup_info.unlock_timestamp = calculate_unlock_timestamp(&config, duration);    
    }
    lockup_info.lp_units_locked += amount;

    // USER INFO :: RETRIEVE --> UPDATE
    let mut user_info = USER_INFO.may_load(deps.storage, &user_address )?.unwrap_or_default();
    if !is_str_present_in_vec(user_info.lockup_positions.clone(), lockup_id.clone()) {
        user_info.lockup_positions.push(lockup_id.clone());
    }

    // SAVE UPDATED STATE
    ASSET_POOLS.save(deps.storage, &lp_token_addr, &pool_info)?;
    LOCKUP_INFO.save(deps.storage, lockup_id.clone().as_bytes(), &lockup_info)?;
    USER_INFO.save(deps.storage, &user_address, &user_info)?;

    Ok(Response::new().add_attributes(vec![
        ("action", "lockdrop::ExecuteMsg::IncreaseLockupPosition"),
        ("user", &user_address.to_string()),
        ("lp_token", &lp_token_addr.to_string()),
        ("duration", duration.to_string().as_str()),
        ("amount", amount.to_string().as_str()),
    ]))
}



// @dev Function to withdraw LP Tokens from an existing Lockup position
pub fn handle_withdraw_from_lockup(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    lp_token_addr: String,
    duration: u64,
    amount: Uint256
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut pool_info = ASSET_POOLS.load(deps.storage, &deps.api.addr_validate(&lp_token_addr)? )?;
    
    // CHECK :: Valid Withdraw Amount
    if amount == Uint256::zero() {
        return Err(StdError::generic_err("Invalid withdrawal request"));
    }

    let user_address = info.sender.clone();
    let lockup_id = user_address.to_string().clone() + &lp_token_addr.clone() + &duration.to_string();
    let mut lockup_info = LOCKUP_INFO.may_load(deps.storage, lockup_id.clone().as_bytes())?.unwrap_or_default();

    // CHECK :: Has user already withdrawn LP tokens once post the deposit window closure state
    if lockup_info.withdrawal_counter {
        return Err(StdError::generic_err("Maximum Withdrawal limit reached. No more withdrawals accepted"));
    }

    // Check :: Amount should be withing the allowed withdrawal limit bounds
    let withdrawals_status = calculate_max_withdrawals_allowed(env.block.time.seconds(), &config);
    let max_withdrawal_allowed = lockup_info.lp_units_locked * withdrawals_status.max_withdrawal_percent;
    if amount > max_withdrawal_allowed  {
        return Err(StdError::generic_err(format!("Amount exceeds maximum allowed withdrawal limit of {} ",max_withdrawal_allowed )));
    }
    // Update withdrawal counter if the max_withdrawal_percent <= 50% ::: as it is being 
    // processed post the deposit window closure 
    if  withdrawals_status.max_withdrawal_percent <= Decimal256::from_ratio(50u64,100u64) {
        lockup_info.withdrawal_counter = true;
    }

    // STATE :: RETRIEVE --> UPDATE
    lockup_info.lp_units_locked = lockup_info.lp_units_locked - amount;
    pool_info.total_lp_units_before_migration = pool_info.total_lp_units_before_migration - amount;
    pool_info.weighted_amount = pool_info.weighted_amount - calculate_weight(amount, duration, config.weekly_multiplier);

    // Remove Lockup position from the list of user positions if Lp_Locked balance == 0
    if lockup_info.lp_units_locked == Uint256::zero() {
        let mut user_info = USER_INFO.load(deps.storage, &user_address.clone())?;
        remove_lockup_pos_from_user_info(&mut user_info, lockup_id.clone());
        USER_INFO.save(deps.storage, &user_address, &user_info)?;
    }

    LOCKUP_INFO.save(deps.storage, lockup_id.clone().as_bytes(), &lockup_info)?;
    ASSET_POOLS.save(deps.storage, &deps.api.addr_validate(&lp_token_addr)?, &pool_info)?;

    // COSMOS_MSG ::TRANSFER WITHDRAWN LP Tokens
    let send_cw20_msg = build_transfer_cw20_token_msg(user_address.clone(), pool_info.lp_token_addr.to_string(), amount.into() )?;

    Ok(Response::new()
        .add_messages(vec![send_cw20_msg])
        .add_attributes(vec![
            ("action", "lockdrop::ExecuteMsg::WithdrawFromLockup"),
            ("user", &user_address.to_string()),
            ("lp_token_addr", &pool_info.lp_token_addr.to_string()),
            ("duration", duration.to_string().as_str()),
            ("amount", amount.to_string().as_str()),
        ]))
}





// @dev Function to delegate part of the ASTRO rewards to be used for LP Bootstrapping via auction 
pub fn handle_delegate_astro_to_auction(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint256
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let user_address = info.sender.clone();
    
    // CHECK :: Have the deposit / withdraw windows concluded
    if env.block.time.seconds() < (config.init_timestamp + config.deposit_window + config.withdrawal_window ) {
        return Err(StdError::generic_err("Deposit / withdraw windows not closed yet"));
    }

    // CHECK :: Can users withdraw their ASTRO tokens ? -> if so, then delegation is no longer allowed
    if state.are_claims_allowed {
        return Err(StdError::generic_err("Delegation window over"));
    }

    let mut user_info = USER_INFO.may_load(deps.storage, &user_address.clone())?.unwrap_or_default();
    
    // CHECK :: User needs to have atleast 1 lockup position
    if user_info.lockup_positions.len() == 0 {
        return Err(StdError::generic_err("No valid lockup positions"));
    }

    // Check is user's total ASTRO rewards have been calculated or not, and calculate and store them in case they are not calculated yet
    if user_info.total_astro_rewards == Uint256::zero() {
        let mut total_astro_rewards = Uint256::zero();
        for lockup_id in &mut user_info.lockup_positions {
            let mut lockup_info = LOCKUP_INFO.load(deps.storage, lockup_id.as_bytes()).unwrap();
            let pool_info = ASSET_POOLS.load(deps.storage, &lockup_info.pool_lp_token_addr ).unwrap();
            let weighted_lockup_balance = calculate_weight(lockup_info.lp_units_locked, lockup_info.duration, config.weekly_multiplier);
            lockup_info.astro_rewards = config.lockdrop_incentives * Decimal256::from_ratio(weighted_lockup_balance, pool_info.weighted_amount);
            LOCKUP_INFO.save(deps.storage, lockup_id.as_bytes(), &lockup_info);
            total_astro_rewards += lockup_info.astro_rewards;
        }
    
        user_info.total_astro_rewards = total_astro_rewards;
        user_info.unclaimed_astro_rewards = total_astro_rewards;    
        // update_user_astro_incentives(&mut deps, &config, &mut user_info);
    }

    // CHECK :: ASTRO to delegate cannot exceed user's unclaimed ASTRO balance
    if amount >  user_info.unclaimed_astro_rewards {
        return Err(StdError::generic_err(format!("ASTRO to delegate cannot exceed user's unclaimed ASTRO balance. ASTRO to delegate = {}, Max delegatable ASTRO = {} ",amount, user_info.unclaimed_astro_rewards)));
    }    

    // UPDATE STATE 
    user_info.delegated_astro_rewards += amount;
    user_info.unclaimed_astro_rewards = user_info.unclaimed_astro_rewards - amount;
    state.total_astro_delegated += amount;

    // SAVE UPDATED STATE
    STATE.save(deps.storage,  &state)?;
    USER_INFO.save(deps.storage, &user_address, &user_info)?;

    // COSMOS_MSG ::Delegate ASTRO to the LP Bootstrapping via Auction contract
    let msg_ = to_binary(&DelegateAstroTokens {
            user_address: info.sender.to_string().clone()
        })?;
    let delegate_msg = build_send_cw20_token_msg(config.auction_contract_address.to_string() , config.astro_token_address.to_string(), amount.into(), msg_)?;

    Ok(Response::new()
        .add_messages(vec![delegate_msg])
        .add_attributes(vec![
            ("action", "lockdrop::ExecuteMsg::DelegateAstroToAuction"),
            ("user", &user_address.to_string()),
            ("amount", amount.to_string().as_str()),
        ]))
}








// @dev Function to stake one of the supported LP Tokens with the Generator contract 
pub fn handle_stake_lp_tokens(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    lp_token_addr: String
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut pool_info = ASSET_POOLS.may_load(deps.storage, &deps.api.addr_validate(&lp_token_addr)? )?.unwrap_or_default();

    // CHECK ::: Only owner can call this function
    if info.sender != config.owner {
        return Err(StdError::generic_err("Only owner can stake LP Tokens with generator"));
    }

    // CHECK :: LP Pool is supported or not 
    if pool_info.lp_token_addr.clone() == zero_address() {
        return Err(StdError::generic_err("Invalid LP Token Pool"));
    }
    
    // CHECK :: Staking LP allowed only after deposit / withdraw windows have concluded
    if env.block.time.seconds() <= (config.init_timestamp + config.deposit_window + config.withdrawal_window) {
        return Err(StdError::generic_err("Staking allowed after the completion of deposit / withdrawal windows"));
    }

    //  COSMOSMSG :: If LP Tokens are migrated, used LP after migration balance else use LP before migration balance
    let mut lp_balance_to_stake = pool_info.total_lp_units_before_migration.clone();
    if pool_info.is_migrated.clone() {
        lp_balance_to_stake = pool_info.total_lp_units_after_migration.clone();
    }
    let stake_lp_msg = build_stake_with_generator_msg( config.generator_address.to_string().clone(),  pool_info.lp_token_addr.clone(), lp_balance_to_stake)?;

    // UPDATE STATE & SAVE
    pool_info.is_staked = true;
    ASSET_POOLS.save(deps.storage, &deps.api.addr_validate(&lp_token_addr)?, &pool_info)?;

    Ok(Response::new()
        .add_message(stake_lp_msg)
        .add_attributes(vec![
            ("action", "lockdrop::ExecuteMsg::StakeLPTokens"),
            ("lp_token_addr", &lp_token_addr.clone()),
            ("staked_amount", lp_balance_to_stake.to_string().as_str()),
    ]))
}



// @dev Function to unstake LP Tokens from the generator contract 
pub fn handle_unstake_lp_tokens(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    lp_token_addr: String
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut pool_info = ASSET_POOLS.may_load(deps.storage, &deps.api.addr_validate(&lp_token_addr)? )?.unwrap_or_default();

    // CHECK ::: Only owner can call this function
    if info.sender != config.owner {
        return Err(StdError::generic_err("Only owner can stake LP Tokens with generator"));
    }

    // CHECK :: LP Pool is supported or not 
    if pool_info.lp_token_addr == zero_address() {
        return Err(StdError::generic_err("Invalid LP Token Pool"));
    }

    // CHECK :: LP Pool is supported or not 
    if !pool_info.is_staked  {
        return Err(StdError::generic_err("Already not staked"));
    }

    let mut cosmos_msgs = vec![];

    // QUERY :: ARE THERE ANY REWARDS TO BE CLAIMED FOR LP STAKING > 0 ?
    // --> If unclaimed rewards > 0, add claimReward {} msg to the cosmos msg array
    let unclaimed_rewards_response: PendingTokenResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                                                                    contract_addr: config.generator_address.to_string(),
                                                                    msg: to_binary(&GenQueryMsg::PendingToken {
                                                                                        lp_token: pool_info.lp_token_addr.clone(),
                                                                                        user: env.contract.address.clone(),
                                                                                    }).unwrap(),
                                                                })).unwrap();

    if unclaimed_rewards_response.pending > Uint128::zero() {
        // QUERY :: Current ASTRO & DUAL Reward Token Balance
        // -->add CallbackMsg::UpdatePoolOnDualRewardsClaim{} msg to the cosmos msg array    
        let astro_balance = cw20_get_balance(&deps.querier, config.astro_token_address,  env.contract.address.clone()  )?;
        let mut dual_reward_balance = Uint128::zero();
        if pool_info.dual_reward_addr != zero_address() {
            dual_reward_balance =   cw20_get_balance(&deps.querier, pool_info.dual_reward_addr.clone(),  env.contract.address.clone()  )? ;        
        }
        let update_state_msg = CallbackMsg::UpdatePoolOnDualRewardsClaim { lp_token_addr: pool_info.lp_token_addr.clone(), 
                                                                                    prev_astro_balance: astro_balance.into(),
                                                                                    prev_dual_reward_balance: dual_reward_balance.into()
                                                                                }.to_cosmos_msg(&env.contract.address)?;
        cosmos_msgs.push(build_claim_dual_rewards(env.contract.address.clone(), pool_info.lp_token_addr.clone(), config.generator_address.clone())?);
        cosmos_msgs.push(update_state_msg);
    }        
        
    //  COSMOSMSG :: If LP Tokens are migrated, used LP after migration balance else use LP before migration balance
    let mut lp_balance_to_unstake = pool_info.total_lp_units_before_migration;
    if pool_info.is_migrated {
        lp_balance_to_unstake = pool_info.total_lp_units_after_migration;
    }
    let unstake_lp_msg = build_unstake_from_generator_msg( config.generator_address.to_string().clone(),  pool_info.lp_token_addr.clone(), lp_balance_to_unstake)?;
    cosmos_msgs.push(unstake_lp_msg);

    // UPDATE STATE & SAVE
    pool_info.is_staked = false;
    ASSET_POOLS.save(deps.storage, &deps.api.addr_validate(&lp_token_addr)?, &pool_info)?;

    Ok(Response::new()
        .add_messages(cosmos_msgs)
        .add_attributes(vec![
            ("action", "lockdrop::ExecuteMsg::UnstakeLPTokens"),
            ("lp_token_addr", &pool_info.lp_token_addr.to_string()),
            ("pool_type", &pool_info.pool_type),
            ("unstaked_amount", lp_balance_to_unstake.to_string().as_str()),
    ]))
}





// @dev Function to withdraw user Rewards for a particular LP Pool
pub fn handle_withdraw_user_rewards_for_lockup(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    lp_token_addr:String,
    duration: u64
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let user_address = info.sender.clone();
    let pool_info = ASSET_POOLS.may_load(deps.storage, &deps.api.addr_validate(&lp_token_addr)? )?.unwrap_or_default();

    // CHECK :: LP Pool is supported or not 
    if pool_info.lp_token_addr == zero_address() {
        return Err(StdError::generic_err("Invalid LP Token Pool"));
    }

    let mut cosmos_msgs = vec![];

    // QUERY :: ARE THERE ANY REWARDS TO BE CLAIMED FOR LP STAKING > 0 ?
    // --> If unclaimed rewards > 0, add claimReward {} msg to the cosmos msg array
    let unclaimed_rewards_response: PendingTokenResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                                                                    contract_addr: config.generator_address.to_string(),
                                                                    msg: to_binary(&GenQueryMsg::PendingToken {
                                                                                        lp_token: pool_info.lp_token_addr.clone(),
                                                                                        user: _env.contract.address.clone(),
                                                                                    }).unwrap(),
                                                                })).unwrap();

    if unclaimed_rewards_response.pending > Uint128::zero()  {
        // QUERY :: Current ASTRO & DUAL Reward Token Balance
        // -->add CallbackMsg::UpdatePoolOnDualRewardsClaim{} msg to the cosmos msg array    
        let astro_balance = cw20_get_balance(&deps.querier, config.astro_token_address,  _env.contract.address.clone()  )?;
        let mut dual_reward_balance = Uint128::zero();
        if pool_info.dual_reward_addr != zero_address() {
            dual_reward_balance =  cw20_get_balance(&deps.querier, pool_info.dual_reward_addr,  _env.contract.address.clone()  )?;        
        }
        let update_state_msg = CallbackMsg::UpdatePoolOnDualRewardsClaim { lp_token_addr: pool_info.lp_token_addr.clone(), 
                                                                                    prev_astro_balance: astro_balance.into(),
                                                                                    prev_dual_reward_balance: dual_reward_balance.into()
                                                                                }.to_cosmos_msg(&_env.contract.address)?;
        cosmos_msgs.push(build_claim_dual_rewards(_env.contract.address.clone(), pool_info.lp_token_addr.clone(), config.generator_address.clone())?);
        cosmos_msgs.push(update_state_msg);
    }
    
    let withdraw_user_rewards_for_lockup_msg = CallbackMsg::WithdrawUserLockupRewardsCallback { 
                                                            user_address: user_address.clone(),
                                                            lp_token_addr: deps.api.addr_validate(&lp_token_addr)?,
                                                            duration: duration,
                                                            withdraw_lp_stake: false
                                                        }.to_cosmos_msg(&_env.contract.address)?;    
    cosmos_msgs.push(withdraw_user_rewards_for_lockup_msg);

    Ok(Response::new()
        .add_messages(cosmos_msgs)
        .add_attributes(vec![
            ("action", "lockdrop::ExecuteMsg::WithdrawUserRewardsForPool"),
            ("lp_token_addr", &pool_info.lp_token_addr.to_string()),
            ("user_address", &user_address.to_string()),
    ]))
}


// @dev Function to unlock a Lockup position whose lockup duration has expired
pub fn handle_unlock_position(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    lp_token_addr:String,
    duration: u64
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let user_address = info.sender.clone();
    let mut user_info = USER_INFO.may_load(deps.storage, &user_address.clone())?.unwrap_or_default();
    let pool_info = ASSET_POOLS.may_load(deps.storage, &deps.api.addr_validate(&lp_token_addr)? )?.unwrap_or_default();
    let lockup_id = user_address.to_string().clone() + &lp_token_addr.to_string().clone() + &duration.to_string();
    let lockup_info = LOCKUP_INFO.may_load(deps.storage, lockup_id.clone().as_bytes())?.unwrap_or_default();

    // CHECK :: LP Pool is supported or not 
    if pool_info.lp_token_addr == zero_address() {
        return Err(StdError::generic_err(format!("{} seconds left to unlock",lockup_info.unlock_timestamp)));
    }
    
    // CHECK :: Can the Lockup position be unlocked or not ? 
    if env.block.time.seconds() > lockup_info.unlock_timestamp  {
        return Err(StdError::generic_err("Invalid LP Token Pool"));
    }

    // CHECK :: Is the lockup position valid or not ?
    if lockup_info.lp_units_locked == Uint256::zero() {
        return Err(StdError::generic_err("Invalid request"));
    }    

    // Check is user's total ASTRO rewards have been calculated or not, and calculate and store them in case they are not calculated yet    
    if user_info.total_astro_rewards == Uint256::zero() {
        let mut total_astro_rewards = Uint256::zero();
        for lockup_id in &mut user_info.lockup_positions {
            let mut lockup_info = LOCKUP_INFO.load(deps.storage, lockup_id.as_bytes()).unwrap();
            let pool_info = ASSET_POOLS.load(deps.storage, &lockup_info.pool_lp_token_addr ).unwrap();
            let weighted_lockup_balance = calculate_weight(lockup_info.lp_units_locked, lockup_info.duration, config.weekly_multiplier);
            lockup_info.astro_rewards = config.lockdrop_incentives * Decimal256::from_ratio(weighted_lockup_balance, pool_info.weighted_amount);
            LOCKUP_INFO.save(deps.storage, lockup_id.as_bytes(), &lockup_info);
            total_astro_rewards += lockup_info.astro_rewards;
        }
    
        user_info.total_astro_rewards = total_astro_rewards;
        user_info.unclaimed_astro_rewards = total_astro_rewards;
        // update_user_astro_incentives(&mut deps, &config, &mut user_info);
    }    

    let mut cosmos_msgs = vec![];

    // QUERY :: ARE THERE ANY REWARDS TO BE CLAIMED FOR LP STAKING > 0 ?
    // --> If unclaimed rewards > 0, add claimReward {} msg to the cosmos msg array
    let unclaimed_rewards_response: PendingTokenResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                                                                    contract_addr: config.generator_address.to_string(),
                                                                    msg: to_binary(&GenQueryMsg::PendingToken {
                                                                                        lp_token: pool_info.lp_token_addr.clone(),
                                                                                        user: env.contract.address.clone(),
                                                                                    }).unwrap(),
                                                                })).unwrap();

    // QUERY :: Current ASTRO & DUAL Reward Token Balance
    // -->add CallbackMsg::UpdatePoolOnDualRewardsClaim{} msg to the cosmos msg array   
    if unclaimed_rewards_response.pending > Uint128::zero() { 
        let astro_balance = cw20_get_balance(&deps.querier, config.astro_token_address,  env.contract.address.clone()  )?;
        let mut dual_reward_balance = Uint128::zero();
        if pool_info.dual_reward_addr != zero_address() {
            dual_reward_balance =  cw20_get_balance(&deps.querier, pool_info.dual_reward_addr,  env.contract.address.clone()   )?;        
        }
        let update_state_msg = CallbackMsg::UpdatePoolOnDualRewardsClaim { lp_token_addr: pool_info.lp_token_addr.clone() , 
                                                                                    prev_astro_balance: astro_balance.into(),
                                                                                    prev_dual_reward_balance: dual_reward_balance.into(),
                                                                                }.to_cosmos_msg(&env.contract.address)?;
        cosmos_msgs.push(build_claim_dual_rewards(env.contract.address.clone(), pool_info.lp_token_addr.clone(), config.generator_address.clone())?);
        cosmos_msgs.push(update_state_msg);
    }

    let withdraw_user_rewards_for_lockup_msg = CallbackMsg::WithdrawUserLockupRewardsCallback { 
                                                            user_address: user_address.clone(),
                                                            lp_token_addr: deps.api.addr_validate(&lp_token_addr)?,
                                                            duration: duration,
                                                            withdraw_lp_stake: true    
                                                        }.to_cosmos_msg(&env.contract.address)?;    
    cosmos_msgs.push(withdraw_user_rewards_for_lockup_msg);

    Ok(Response::new()
        .add_messages(cosmos_msgs)
        .add_attributes(vec![
            ("action", "lockdrop::ExecuteMsg::WithdrawUserRewardsForPool"),
            ("lp_token_addr", &pool_info.lp_token_addr.to_string()),
            ("user_address", &user_address.to_string()),
    ]))
}



// @dev Function to unlock a Lockup position whose lockup duration has expired
pub fn handle_force_unlock_position(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    lp_token_addr:String,
    duration: u64
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let user_address = info.sender.clone();
    let pool_info = ASSET_POOLS.may_load(deps.storage, &deps.api.addr_validate(&lp_token_addr)? )?.unwrap_or_default();
    let lockup_id = user_address.to_string().clone() + &lp_token_addr.to_string().clone() + &duration.to_string();
    let lockup_info = LOCKUP_INFO.may_load(deps.storage, lockup_id.clone().as_bytes())?.unwrap_or_default();

    // CHECK :: LP Pool is supported or not 
    if pool_info.lp_token_addr == zero_address() {
        return Err(StdError::generic_err(format!("{} seconds left to unlock",lockup_info.unlock_timestamp)));
    }
    
    // CHECK :: Is the lockup position valid or not ?
    if lockup_info.lp_units_locked == Uint256::zero() {
        return Err(StdError::generic_err("Invalid request"));
    }
    

    let mut cosmos_msgs = vec![];

    // QUERY :: ARE THERE ANY REWARDS TO BE CLAIMED FOR LP STAKING > 0 ?
    // --> If unclaimed rewards > 0, add claimReward {} msg to the cosmos msg array
    let unclaimed_rewards_response: PendingTokenResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                                                                    contract_addr: config.generator_address.to_string(),
                                                                    msg: to_binary(&GenQueryMsg::PendingToken {
                                                                                        lp_token: pool_info.lp_token_addr.clone(),
                                                                                        user: env.contract.address.clone(),
                                                                                    }).unwrap(),
                                                                })).unwrap();

    // QUERY :: Current ASTRO & DUAL Reward Token Balance
    // -->add CallbackMsg::UpdatePoolOnDualRewardsClaim{} msg to the cosmos msg array  
    if unclaimed_rewards_response.pending > Uint128::zero() { 
        let astro_balance = cw20_get_balance(&deps.querier, config.astro_token_address,  env.contract.address.clone()  )?;
        let mut dual_reward_balance = Uint128::zero();
        if pool_info.dual_reward_addr != zero_address() {
            dual_reward_balance =  cw20_get_balance(&deps.querier, pool_info.dual_reward_addr,  env.contract.address.clone()  )?;        
        }
        let update_state_msg = CallbackMsg::UpdatePoolOnDualRewardsClaim { lp_token_addr: pool_info.lp_token_addr.clone(), 
                                                                                    prev_astro_balance: astro_balance.into(),
                                                                                    prev_dual_reward_balance: dual_reward_balance.into(),
                                                                                }.to_cosmos_msg(&env.contract.address)?;
        cosmos_msgs.push(build_claim_dual_rewards(env.contract.address.clone(), pool_info.lp_token_addr.clone(), config.generator_address.clone())?);
        cosmos_msgs.push(update_state_msg);
    }
    let withdraw_user_rewards_for_lockup_msg = CallbackMsg::WithdrawUserLockupRewardsCallback { 
                                                            user_address: user_address.clone(),
                                                            lp_token_addr: deps.api.addr_validate(&lp_token_addr)?,
                                                            duration: duration,
                                                            withdraw_lp_stake: true    
                                                        }.to_cosmos_msg(&env.contract.address)?;    
    cosmos_msgs.push(withdraw_user_rewards_for_lockup_msg);

    Ok(Response::new()
        .add_messages(cosmos_msgs)
        .add_attributes(vec![
            ("action", "lockdrop::ExecuteMsg::WithdrawUserRewardsForPool"),
            ("lp_token_addr", &pool_info.lp_token_addr.to_string()),
            ("user_address", &user_address.to_string()),
    ]))
}



//----------------------------------------------------------------------------------------
// Callback Functions
//----------------------------------------------------------------------------------------

// CALLBACK :: CALLED AFTER ASTRO / DUAL REWARDS ARE CLAIMED FROM THE GENERATOR CONTRACT :: UPDATES THE REWARD_INDEXES OF THE POOL
pub fn update_pool_on_dual_rewards_claim(
    deps: DepsMut,
    env: Env,
    lp_token_addr: Addr,
    prev_astro_balance: Uint256,
    prev_dual_reward_balance: Uint256,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut pool_info = ASSET_POOLS.load(deps.storage, &lp_token_addr )?;

    // QUERY CURRENT ASTRO / DUAL REWARD TOKEN BALANCE :: Used to calculate claimed rewards
    let cur_astro_balance = cw20_get_balance(&deps.querier, config.astro_token_address.clone(), env.contract.address.clone() )?;
    let cur_dual_reward_balance = cw20_get_balance(&deps.querier, pool_info.dual_reward_addr.clone(), env.contract.address.clone() )?;
    let astro_claimed = Uint256::from(cur_astro_balance) - prev_astro_balance;
    let dual_reward_claimed = Uint256::from(cur_dual_reward_balance) - prev_dual_reward_balance;

    // UPDATE ASTRO & DUAL REWARD INDEXED FOR THE CURRENT POOL
    if astro_claimed > Uint256::zero() {
        update_astro_rewards_index(&mut pool_info, astro_claimed);
    }
    if dual_reward_claimed > Uint256::zero() {
        update_dual_rewards_index(&mut pool_info, dual_reward_claimed);
    }
    
    // SAVE UPDATED STATE OF THE POOL
    ASSET_POOLS.save(deps.storage, &lp_token_addr.clone(), &pool_info)?;

    Ok(Response::new()
    .add_attributes(vec![
        ("action", "lockdrop::CallbackMsg::UpdateRewardIndexes"),
        ("lp_token_addr", lp_token_addr.to_string().as_str()),
        ("astro_claimed", astro_claimed.to_string().as_str()),
        ("dual_reward_claimed", dual_reward_claimed.to_string().as_str()),
        ("astro_global_reward_index", pool_info.astro_global_reward_index.to_string().as_str()),
        ("asset_global_reward_index", pool_info.asset_global_reward_index.to_string().as_str()),
    ]))

}


// CALLBACK :: 
pub fn callback_withdraw_user_rewards_for_lockup_optional_withdraw(
    deps: DepsMut,
    env: Env,
    user_address: Addr,
    lp_token_addr: Addr,
    duration: u64,
    withdraw_lp_stake: bool
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state: State = STATE.load(deps.storage)?;
    let mut pool_info = ASSET_POOLS.load(deps.storage, &lp_token_addr )?;
    let lockup_id = user_address.to_string().clone() + &lp_token_addr.to_string().clone() + &duration.to_string();
    let mut lockup_info = LOCKUP_INFO.may_load(deps.storage, lockup_id.clone().as_bytes())?.unwrap_or_default();

    // UPDATE ASTRO & DUAL REWARD INDEXED FOR THE CURRENT POOL
    let pending_astro_rewards = compute_lockup_position_accrued_astro_rewards(&pool_info, &mut lockup_info);
    let pending_dual_rewards = compute_lockup_position_accrued_astro_rewards(&pool_info, &mut lockup_info);

    // SAVE UPDATED STATE OF THE POOL
    ASSET_POOLS.save(deps.storage, &lp_token_addr.clone(), &pool_info)?;

    // COSMOS MSG :: Transfer pending ASTRO / DUAL Rewards
    let mut cosmos_msgs = vec![];
    if pending_astro_rewards > Uint256::zero() {
        cosmos_msgs.push( build_transfer_cw20_token_msg(user_address.clone(), config.astro_token_address.clone().to_string(), pending_astro_rewards.into() )? );
    }
    if pending_dual_rewards > Uint256::zero() {
        cosmos_msgs.push( build_transfer_cw20_token_msg(user_address.clone(), pool_info.dual_reward_addr.clone().to_string(), pending_dual_rewards.into()  )? );
    }

    if withdraw_lp_stake {

        // COSMOSMSG :: Transfers ASTRO (that user received as rewards for this lockup position) from user to itself
        let transfer_astro_msg = build_transfer_cw20_from_user_msg(config.astro_token_address.clone().to_string(), user_address.clone().to_string(), env.contract.address.to_string(), lockup_info.astro_rewards )?;
        cosmos_msgs.push(transfer_astro_msg);

        //  COSMOSMSG :: If LP Tokens are staked, we unstake the amount which needs to be returned to the user
        if pool_info.is_staked {
            let unstake_lp_msg = build_unstake_from_generator_msg( config.generator_address.clone().to_string(),  pool_info.lp_token_addr.clone(), lockup_info.lp_units_locked)?;
            cosmos_msgs.push(unstake_lp_msg);
        }
        // COSMOSMSG :: Returns LP units locked by the user in the current lockup position
        let transfer_lp_msg = build_transfer_cw20_token_msg( user_address.clone(),  pool_info.lp_token_addr.clone().to_string(), lockup_info.lp_units_locked.into() )?;
        cosmos_msgs.push(transfer_lp_msg);

        // UPDATE STATE :: Lockup, state, pool, user
        // Remove lockup position from user's lockup position array
        lockup_info.lp_units_locked = Uint256::zero();
        // remove_lockup_pos_from_user_info(&mut user_info, lockup_id.clone());

        state.total_astro_returned += lockup_info.astro_rewards;
        if pool_info.is_migrated {
            pool_info.total_lp_units_after_migration = pool_info.total_lp_units_after_migration - lockup_info.lp_units_locked;        
        }
        else {
            pool_info.total_lp_units_before_migration = pool_info.total_lp_units_before_migration - lockup_info.lp_units_locked;        
        }
        // Save updated pool state
        ASSET_POOLS.save(deps.storage, &lp_token_addr.clone(), &pool_info)?;        
    }

    // Save updated state
    LOCKUP_INFO.save(deps.storage, lockup_id.clone().as_bytes(), &lockup_info)?;

    Ok(Response::new()
    .add_messages(cosmos_msgs)
    .add_attributes(vec![
        ("action", "lockdrop::CallbackMsg::WithdrawPendingRewardsForLockup"),
        ("lp_token_addr", lp_token_addr.to_string().as_str()),
        ("user_address", user_address.to_string().as_str()),
        ("duration", duration.to_string().as_str()),
        ("pending_astro_rewards", pending_astro_rewards.to_string().as_str()),
        ("pending_dual_rewards", pending_dual_rewards.to_string().as_str()),
    ]))

}


// Calculate pending ASTRO rewards for a particular LOCKUP Position
fn compute_lockup_position_accrued_astro_rewards(pool_info: &PoolInfo, lockup_info: &mut LockupInfo) -> Uint256 {
    if !pool_info.is_staked {
        return Uint256::zero();
    }
    let pending_astro_rewards = (lockup_info.lp_units_locked * pool_info.astro_global_reward_index)  - (lockup_info.lp_units_locked * lockup_info.astro_reward_index);
    lockup_info.astro_reward_index = pool_info.astro_global_reward_index;
    pending_astro_rewards
}


// Calculate pending DUAL rewards for a particular LOCKUP Position
fn compute_lockup_position_accrued_dual_rewards(pool_info: &PoolInfo, lockup_info: &mut LockupInfo) -> Uint256 {
    if !pool_info.is_staked {
        return Uint256::zero();
    }
    let pending_dual_rewards = (lockup_info.lp_units_locked * pool_info.asset_global_reward_index)  - (lockup_info.lp_units_locked * lockup_info.dual_reward_index);
    lockup_info.dual_reward_index = pool_info.asset_global_reward_index;
    pending_dual_rewards
}





//----------------------------------------------------------------------------------------
// Query Functions
//----------------------------------------------------------------------------------------

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
        supported_lp_tokens: state.supported_lp_tokens,
    })
}


/// @dev Returns the pool's State
pub fn query_pool(deps: Deps, lp_token_addr:String) -> StdResult<PoolResponse> {
    let pool_info: PoolInfo = ASSET_POOLS.load(deps.storage, &deps.api.addr_validate(&lp_token_addr)? )?;
    Ok(PoolResponse {
        lp_token_addr: pool_info.lp_token_addr,
        pool_addr: pool_info.pool_addr,
        dual_reward_addr: pool_info.dual_reward_addr,
        incentives_percent: pool_info.incentives_percent,
        total_lp_units_before_migration: pool_info.total_lp_units_before_migration,
        total_lp_units_after_migration: pool_info.total_lp_units_after_migration,
        is_staked: pool_info.is_staked,
        pool_type: pool_info.pool_type,
        is_migrated: pool_info.is_migrated,
        weighted_amount: pool_info.weighted_amount,
        astro_global_reward_index: pool_info.astro_global_reward_index,
        asset_global_reward_index: pool_info.asset_global_reward_index

    })
}





/// @dev Returns summarized details regarding the user
pub fn query_user_info(deps: Deps, env: Env, user: String) -> StdResult<UserInfoResponse> {
    let config = CONFIG.load(deps.storage)?;
    let user_address = deps.api.addr_validate(&user)?;
    let mut user_info = USER_INFO.may_load(deps.storage, &user_address.clone())?.unwrap_or_default();

    Ok(UserInfoResponse {
        total_astro_rewards: user_info.total_astro_rewards,
        unclaimed_astro_rewards: user_info.unclaimed_astro_rewards,
        delegated_astro_rewards: user_info.delegated_astro_rewards,
        lockup_positions: user_info.lockup_positions
    })
}


// /// @dev Returns summarized details regarding the user
pub fn query_lockup_info(deps: Deps, user_address: String, lp_token_address:String, duration: u64) -> StdResult<LockUpInfoResponse> {
    let lockup_id = user_address.to_string() + &lp_token_address + &duration.to_string();
    query_lockup_info_with_id(deps, lockup_id)
}

/// @dev Returns summarized details regarding the user
pub fn query_lockup_info_with_id(deps: Deps, lockup_id: String) -> StdResult<LockUpInfoResponse> {
    let lockup_info = LOCKUP_INFO.may_load(deps.storage, lockup_id.clone().as_bytes())?.unwrap_or_default();
    let state: State = STATE.load(deps.storage)?;

    Ok(LockUpInfoResponse {
        pool_lp_token_addr: lockup_info.pool_lp_token_addr,
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
// HELPERS
//----------------------------------------------------------------------------------------

/// true if deposits are allowed
fn is_deposit_open(current_timestamp: u64, config: &Config) -> bool {
    let deposits_opened_till = config.init_timestamp + config.deposit_window;
    (current_timestamp >= config.init_timestamp) && (deposits_opened_till >= current_timestamp)
}



fn calculate_max_withdrawals_allowed(current_timestamp: u64, config: &Config) -> WithdrawalStatus {
    let withdrawal_cutoff_init_point = config.init_timestamp + config.deposit_window;
    // 100% withdrawals allowed
    if current_timestamp <= withdrawal_cutoff_init_point {
        return  WithdrawalStatus {
            max_withdrawal_percent: Decimal256::from_ratio(100u32, 100u32),
            update_withdrawal_counter: false
        }
    }

    let withdrawal_cutoff_sec_point = withdrawal_cutoff_init_point + (config.withdrawal_window/2u64);
    // 50% withdrawals allowed
    if current_timestamp <= withdrawal_cutoff_sec_point {
        return  WithdrawalStatus {
            max_withdrawal_percent: Decimal256::from_ratio(50u32, 100u32),
            update_withdrawal_counter: true
        }
    }

    let withdrawal_cutoff_final = withdrawal_cutoff_sec_point + (config.withdrawal_window/2u64);
    // max withdrawal allowed decreasing linearly from 50% to 0% vs time elapsed
    if current_timestamp < withdrawal_cutoff_final {
        let slope = Decimal256::from_ratio( 50u64, config.withdrawal_window/2u64 );
        let time_elapsed = current_timestamp - withdrawal_cutoff_sec_point;
        return  WithdrawalStatus {
            max_withdrawal_percent: Decimal256::from_ratio(time_elapsed, 1u64) * slope,
            update_withdrawal_counter: true
        }
    }
    // Withdrawals not allowed
    else {
        return  WithdrawalStatus {
            max_withdrawal_percent: Decimal256::from_ratio(0u32, 100u32),
            update_withdrawal_counter: true
        }
    }
}



/// Helper function. Updates ASTRO Lockdrop rewards that a user will get based on the weighted LP deposits across all of this lockup positions
fn update_user_astro_incentives(deps: &mut DepsMut, config: &Config, user_info: &mut UserInfo) {
    if user_info.total_astro_rewards == Uint256::zero() {
        return;
    }

    let mut total_astro_rewards = Uint256::zero();
    for lockup_id in &mut user_info.lockup_positions {
        let mut lockup_info = LOCKUP_INFO.load(deps.storage, lockup_id.as_bytes()).unwrap();
        let pool_info = ASSET_POOLS.load(deps.storage, &lockup_info.pool_lp_token_addr ).unwrap();
        let weighted_lockup_balance = calculate_weight(lockup_info.lp_units_locked, lockup_info.duration, config.weekly_multiplier);
        lockup_info.astro_rewards = config.lockdrop_incentives * Decimal256::from_ratio(weighted_lockup_balance, pool_info.weighted_amount);
        LOCKUP_INFO.save(deps.storage, lockup_id.as_bytes(), &lockup_info);
        total_astro_rewards += lockup_info.astro_rewards;
    }

    user_info.total_astro_rewards = total_astro_rewards;
    user_info.unclaimed_astro_rewards = total_astro_rewards;
}










/// true if withdrawals are allowed
fn is_withdraw_open(current_timestamp: u64, config: &Config) -> bool {
    let withdrawals_opened_till = config.init_timestamp + config.withdrawal_window;
    (current_timestamp >= config.init_timestamp) && (withdrawals_opened_till >= current_timestamp)
}

/// Returns the timestamp when the lockup will get unlocked
fn calculate_unlock_timestamp(config: &Config, duration: u64) -> u64 {
    config.init_timestamp + config.deposit_window + config.withdrawal_window + (duration * config.seconds_per_week)
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

// Accrue ASTRO rewards by updating the pool's reward index
fn update_astro_rewards_index(pool_info: &mut PoolInfo, astro_accured: Uint256)  {
    if !pool_info.is_staked {
        return;
    }
    let mut total_lp_tokens = pool_info.total_lp_units_before_migration;
    if pool_info.is_migrated {
        total_lp_tokens = pool_info.total_lp_units_after_migration;
    }
    let astro_rewards_index_increment = Decimal256::from_ratio( astro_accured, total_lp_tokens);
    pool_info.astro_global_reward_index = pool_info.astro_global_reward_index + astro_rewards_index_increment;
}


// Accrue ASSET (Dual Reward) rewards by updating the pool's reward index
fn update_dual_rewards_index(pool_info: &mut PoolInfo, dual_rewards_accured: Uint256)  {
    if !pool_info.is_staked {
        return;
    }
    let mut total_lp_tokens = pool_info.total_lp_units_before_migration;
    if pool_info.is_migrated {
        total_lp_tokens = pool_info.total_lp_units_after_migration;
    }
    let dual_rewards_index_increment = Decimal256::from_ratio( dual_rewards_accured, total_lp_tokens);
    pool_info.asset_global_reward_index = pool_info.asset_global_reward_index + dual_rewards_index_increment;
}


// Returns true if the user_info stuct's lockup_positions vector contains the lockup_id
fn is_str_present_in_vec(vector_struct: Vec<String>, lockup_id: String) -> bool {
    if vector_struct.iter().any(|id| id == &lockup_id) {
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


/// Helper Function. Returns CosmosMsg which unstakes LP Tokens from the generator contract
fn build_unstake_from_generator_msg(generator_address: String, lp_token_addr:Addr, unstake_amount: Uint256) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: generator_address.to_string(),
        funds: vec![],
        msg: to_binary(&astroport::generator::ExecuteMsg::Withdraw {
            lp_token: lp_token_addr,
            amount: unstake_amount.into()
        })?,
    }))
}

/// Helper Function. Returns CosmosMsg which stakes LP Tokens with the generator contract
fn build_stake_with_generator_msg(generator_address: String, lp_token_addr:Addr, stake_amount: Uint256) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: generator_address.to_string(),
        funds: vec![],
        msg: to_binary(&astroport::generator::ExecuteMsg::Deposit {
            lp_token: lp_token_addr,
            amount: stake_amount.into()
        })?,
    }))
}


fn build_claim_dual_rewards(recepient_address:Addr, lp_token_contract: Addr , generator_contract: Addr) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: generator_contract.to_string(),
        funds: vec![],
        msg: to_binary(&astroport::generator::ExecuteMsg::SendOrphanReward { 
            recipient: recepient_address.to_string(),
            lp_token: Some(lp_token_contract.to_string())
         })?,
    }))
}



//----------------------------------------------------------------------------------------
// TESTS
//----------------------------------------------------------------------------------------

// #[cfg(test)]
// mod tests {
//     use super::*;

//     use cosmwasm_std::testing::{MockApi, MockStorage, MOCK_CONTRACT_ADDR};
//     use cosmwasm_std::{attr, coin, Coin, Decimal, OwnedDeps, SubMsg, Timestamp, Uint128};

//     use mars::testing::{
//         assert_generic_error_message, mock_dependencies, mock_env, mock_info, MarsMockQuerier,
//         MockEnvParams,
//     };

//     use mars_periphery::lockdrop::{CallbackMsg, ExecuteMsg, InstantiateMsg, UpdateConfigMsg};

//     #[test]
//     fn test_proper_initialization() {
//         let mut deps = mock_dependencies(&[]);
//         let info = mock_info("owner");
//         let env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(10_000_001),
//             ..Default::default()
//         });

//         let mut base_config = InstantiateMsg {
//             owner: "owner".to_string(),
//             address_provider: None,
//             ma_ust_token: None,
//             init_timestamp: 10_000_000,
//             deposit_window: 100000,
//             withdrawal_window: 72000,
//             min_duration: 1,
//             max_duration: 5,
//             seconds_per_week: 7 * 86400 as u64, 
//             denom: Some("uusd".to_string()),
//             weekly_multiplier: Some(Decimal256::from_ratio(9u64, 100u64)),
//             lockdrop_incentives: None,
//         };

//         // ***
//         // *** Test :: "Invalid timestamp" ***
//         // ***
//         base_config.init_timestamp = 10_000_000;
//         let mut res_f = instantiate(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             base_config.clone(),
//         );
//         assert_generic_error_message(res_f, "Invalid timestamp");

//         // ***
//         // *** Test :: "Invalid deposit / withdraw window" ***
//         // ***
//         base_config.init_timestamp = 10_000_007;
//         base_config.deposit_window = 15u64;
//         base_config.withdrawal_window = 15u64;
//         res_f = instantiate(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             base_config.clone(),
//         );
//         assert_generic_error_message(res_f, "Invalid deposit / withdraw window");

//         // ***
//         // *** Test :: "Invalid Lockup durations" ***
//         // ***
//         base_config.init_timestamp = 10_000_007;
//         base_config.deposit_window = 15u64;
//         base_config.withdrawal_window = 9u64;
//         base_config.max_duration = 9u64;
//         base_config.min_duration = 9u64;
//         res_f = instantiate(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             base_config.clone(),
//         );
//         assert_generic_error_message(res_f, "Invalid Lockup durations");

//         // ***
//         // *** Test :: Should instantiate successfully ***
//         // ***
//         base_config.min_duration = 1u64;
//         let res_s = instantiate(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             base_config.clone(),
//         )
//         .unwrap();
//         assert_eq!(0, res_s.messages.len());
//         // let's verify the config
//         let config_ = query_config(deps.as_ref()).unwrap();
//         assert_eq!("owner".to_string(), config_.owner);
//         assert_eq!("".to_string(), config_.address_provider);
//         assert_eq!("".to_string(), config_.ma_ust_token);
//         assert_eq!(10_000_007, config_.init_timestamp);
//         assert_eq!(15u64, config_.deposit_window);
//         assert_eq!(9u64, config_.withdrawal_window);
//         assert_eq!(1u64, config_.min_duration);
//         assert_eq!(9u64, config_.max_duration);
//         assert_eq!(Decimal256::from_ratio(9u64, 100u64), config_.multiplier);
//         assert_eq!(Uint256::zero(), config_.lockdrop_incentives);

//         // let's verify the state
//         let state_ = query_state(deps.as_ref()).unwrap();
//         assert_eq!(Uint256::zero(), state_.final_ust_locked);
//         assert_eq!(Uint256::zero(), state_.final_maust_locked);
//         assert_eq!(Uint256::zero(), state_.total_ust_locked);
//         assert_eq!(Uint256::zero(), state_.total_maust_locked);
//         assert_eq!(Decimal256::zero(), state_.global_reward_index);
//         assert_eq!(Uint256::zero(), state_.total_deposits_weight);
//     }

//     #[test]
//     fn test_update_config() {
//         let mut deps = mock_dependencies(&[]);
//         let mut info = mock_info("owner");
//         let mut env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_000_000_00),
//             ..Default::default()
//         });

//         // *** Instantiate successfully ***
//         let base_config = InstantiateMsg {
//             owner: "owner".to_string(),
//             address_provider: None,
//             ma_ust_token: None,
//             init_timestamp: 1_000_000_05,
//             deposit_window: 100000u64,
//             withdrawal_window: 72000u64,
//             min_duration: 1u64,
//             max_duration: 5u64,
//             seconds_per_week: 7 * 86400 as u64, 
//             denom: Some("uusd".to_string()),
//             weekly_multiplier: Some(Decimal256::from_ratio(9u64, 100u64)),
//             lockdrop_incentives: None,
//         };
//         let res_s = instantiate(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             base_config.clone(),
//         )
//         .unwrap();
//         assert_eq!(0, res_s.messages.len());

//         // ***
//         // *** Test :: Error "Only owner can update configuration" ***
//         // ***
//         info = mock_info("not_owner");
//         let mut update_config = UpdateConfigMsg {
//             owner: Some("new_owner".to_string()),
//             address_provider: Some("new_address_provider".to_string()),
//             ma_ust_token: Some("new_ma_ust_token".to_string()),
//             init_timestamp: None,
//             deposit_window: None,
//             withdrawal_window: None,
//             min_duration: None,
//             max_duration: None,
//             weekly_multiplier: None,
//             lockdrop_incentives: None,
//         };
//         let mut update_config_msg = ExecuteMsg::UpdateConfig {
//             new_config: update_config.clone(),
//         };

//         let res_f = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             update_config_msg.clone(),
//         );
//         assert_generic_error_message(res_f, "Only owner can update configuration");

//         // ***
//         // *** Test :: Update addresses successfully ***
//         // ***
//         info = mock_info("owner");
//         let update_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             update_config_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             update_s.attributes,
//             vec![attr("action", "lockdrop::ExecuteMsg::UpdateConfig")]
//         );
//         // let's verify the config
//         let mut config_ = query_config(deps.as_ref()).unwrap();
//         assert_eq!("new_owner".to_string(), config_.owner);
//         assert_eq!("new_address_provider".to_string(), config_.address_provider);
//         assert_eq!("new_ma_ust_token".to_string(), config_.ma_ust_token);
//         assert_eq!(1_000_000_05, config_.init_timestamp);
//         assert_eq!(100000u64, config_.deposit_window);
//         assert_eq!(72000u64, config_.withdrawal_window);
//         assert_eq!(1u64, config_.min_duration);
//         assert_eq!(5u64, config_.max_duration);
//         assert_eq!(Decimal256::from_ratio(9u64, 100u64), config_.multiplier);
//         assert_eq!(Uint256::zero(), config_.lockdrop_incentives);

//         // ***
//         // *** Test :: Don't Update init_timestamp,min_lock_duration, max_lock_duration, weekly_multiplier (Reason :: env.block.time.seconds() >= config.init_timestamp)  ***
//         // ***
//         info = mock_info("new_owner");
//         env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_000_000_05),
//             ..Default::default()
//         });
//         update_config.init_timestamp = Some(1_000_000_39);
//         update_config.min_duration = Some(3u64);
//         update_config.max_duration = Some(9u64);
//         update_config.weekly_multiplier = Some(Decimal256::from_ratio(17u64, 100u64));
//         update_config.lockdrop_incentives = Some(Uint256::from(100000u64));
//         update_config_msg = ExecuteMsg::UpdateConfig {
//             new_config: update_config.clone(),
//         };

//         let mut update_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             update_config_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             update_s.attributes,
//             vec![attr("action", "lockdrop::ExecuteMsg::UpdateConfig")]
//         );

//         config_ = query_config(deps.as_ref()).unwrap();
//         assert_eq!(1_000_000_05, config_.init_timestamp);
//         assert_eq!(1u64, config_.min_duration);
//         assert_eq!(5u64, config_.max_duration);
//         assert_eq!(Decimal256::from_ratio(9u64, 100u64), config_.multiplier);
//         assert_eq!(Uint256::from(100000u64), config_.lockdrop_incentives);

//         // ***
//         // *** Test :: Update init_timestamp successfully ***
//         // ***
//         env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_000_000_01),
//             ..Default::default()
//         });
//         update_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             update_config_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             update_s.attributes,
//             vec![attr("action", "lockdrop::ExecuteMsg::UpdateConfig")]
//         );

//         config_ = query_config(deps.as_ref()).unwrap();
//         assert_eq!(1_000_000_39, config_.init_timestamp);
//         assert_eq!(3u64, config_.min_duration);
//         assert_eq!(9u64, config_.max_duration);
//         assert_eq!(Decimal256::from_ratio(17u64, 100u64), config_.multiplier);
//         assert_eq!(Uint256::from(100000u64), config_.lockdrop_incentives);
//     }

//     #[test]
//     fn test_deposit_ust() {
//         let mut deps = th_setup(&[]);
//         let deposit_amount = 110000u128;
//         let mut info =
//             cosmwasm_std::testing::mock_info("depositor", &[coin(deposit_amount, "uusd")]);
//         deps.querier
//             .set_incentives_address(Addr::unchecked("incentives".to_string()));
//         deps.querier
//             .set_unclaimed_rewards("cosmos2contract".to_string(), Uint128::from(0u64));
//         // ***
//         // *** Test :: Error "Deposit window closed" Reason :: Deposit attempt before deposit window is open ***
//         // ***
//         let mut env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_000_000_05),
//             ..Default::default()
//         });
//         let mut deposit_msg = ExecuteMsg::DepositUst { duration: 3u64 };
//         let mut deposit_f = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         );
//         assert_generic_error_message(deposit_f, "Deposit window closed");

//         // ***
//         // *** Test :: Error "Deposit window closed" Reason :: Deposit attempt after deposit window is closed ***
//         // ***
//         env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_010_000_01),
//             ..Default::default()
//         });
//         deposit_f = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         );
//         assert_generic_error_message(deposit_f, "Deposit window closed");

//         // ***
//         // *** Test :: Error "Amount cannot be zero" ***
//         // ***
//         env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_000_000_15),
//             ..Default::default()
//         });
//         info = cosmwasm_std::testing::mock_info("depositor", &[coin(0u128, "uusd")]);
//         deposit_f = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         );
//         assert_generic_error_message(deposit_f, "Amount cannot be zero");

//         // ***
//         // *** Test :: Error "Lockup duration needs to be between {} and {}" Reason :: Selected lockup duration < min_duration ***
//         // ***
//         info = cosmwasm_std::testing::mock_info("depositor", &[coin(10000u128, "uusd")]);
//         deposit_msg = ExecuteMsg::DepositUst { duration: 1u64 };
//         deposit_f = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         );
//         assert_generic_error_message(deposit_f, "Lockup duration needs to be between 3 and 9");

//         // ***
//         // *** Test :: Error "Lockup duration needs to be between {} and {}" Reason :: Selected lockup duration > max_duration ***
//         // ***
//         deposit_msg = ExecuteMsg::DepositUst { duration: 21u64 };
//         deposit_f = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         );
//         assert_generic_error_message(deposit_f, "Lockup duration needs to be between 3 and 9");

//         // ***
//         // *** Test #1 :: Successfully deposit UST  ***
//         // ***
//         deposit_msg = ExecuteMsg::DepositUst { duration: 3u64 };
//         let mut deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "3"),
//                 attr("ust_deposited", "10000")
//             ]
//         );
//         // let's verify the Lockdrop
//         let mut lockdrop_ =
//             query_lockup_info_with_id(deps.as_ref(), "depositor3".to_string()).unwrap();
//         assert_eq!(3u64, lockdrop_.duration);
//         assert_eq!(Uint256::from(10000u64), lockdrop_.ust_locked);
//         assert_eq!(Uint256::zero(), lockdrop_.maust_balance);
//         assert_eq!(Uint256::from(21432423343u64), lockdrop_.lockdrop_reward);
//         assert_eq!(101914410u64, lockdrop_.unlock_timestamp);
//         // let's verify the User
//         let mut user_ =
//             query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
//         assert_eq!(Uint256::from(10000u64), user_.total_ust_locked);
//         assert_eq!(Uint256::zero(), user_.total_maust_locked);
//         assert_eq!(vec!["depositor3".to_string()], user_.lockup_position_ids);
//         assert_eq!(false, user_.is_lockdrop_claimed);
//         assert_eq!(Decimal256::zero(), user_.reward_index);
//         assert_eq!(Uint256::zero(), user_.pending_xmars);
//         // let's verify the state
//         let mut state_ = query_state(deps.as_ref()).unwrap();
//         assert_eq!(Uint256::zero(), state_.final_ust_locked);
//         assert_eq!(Uint256::zero(), state_.final_maust_locked);
//         assert_eq!(Uint256::from(10000u64), state_.total_ust_locked);
//         assert_eq!(Uint256::zero(), state_.total_maust_locked);
//         assert_eq!(Uint256::from(2700u64), state_.total_deposits_weight);

//         // ***
//         // *** Test #2 :: Successfully deposit UST  ***
//         // ***
//         info = cosmwasm_std::testing::mock_info("depositor", &[coin(100u128, "uusd")]);
//         deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "3"),
//                 attr("ust_deposited", "100")
//             ]
//         );
//         // let's verify the Lockdrop
//         lockdrop_ = query_lockup_info_with_id(deps.as_ref(), "depositor3".to_string()).unwrap();
//         assert_eq!(3u64, lockdrop_.duration);
//         assert_eq!(Uint256::from(10100u64), lockdrop_.ust_locked);
//         assert_eq!(101914410u64, lockdrop_.unlock_timestamp);
//         // let's verify the User
//         user_ = query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
//         assert_eq!(Uint256::from(10100u64), user_.total_ust_locked);
//         assert_eq!(vec!["depositor3".to_string()], user_.lockup_position_ids);
//         // let's verify the state
//         state_ = query_state(deps.as_ref()).unwrap();
//         assert_eq!(Uint256::from(10100u64), state_.total_ust_locked);
//         assert_eq!(Uint256::from(2727u64), state_.total_deposits_weight);

//         // ***
//         // *** Test #3 :: Successfully deposit UST (new lockup)  ***
//         // ***
//         deposit_msg = ExecuteMsg::DepositUst { duration: 5u64 };
//         info = cosmwasm_std::testing::mock_info("depositor", &[coin(5432u128, "uusd")]);
//         deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "5"),
//                 attr("ust_deposited", "5432")
//             ]
//         );
//         // let's verify the Lockdrop
//         lockdrop_ = query_lockup_info_with_id(deps.as_ref(), "depositor5".to_string()).unwrap();
//         assert_eq!(5u64, lockdrop_.duration);
//         assert_eq!(Uint256::from(5432u64), lockdrop_.ust_locked);
//         assert_eq!(103124010u64, lockdrop_.unlock_timestamp);
//         // let's verify the User
//         user_ = query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
//         assert_eq!(Uint256::from(15532u64), user_.total_ust_locked);
//         assert_eq!(
//             vec!["depositor3".to_string(), "depositor5".to_string()],
//             user_.lockup_position_ids
//         );
//         // let's verify the state
//         state_ = query_state(deps.as_ref()).unwrap();
//         assert_eq!(Uint256::from(15532u64), state_.total_ust_locked);
//         assert_eq!(Uint256::from(5171u64), state_.total_deposits_weight);
//     }

//     #[test]
//     fn test_withdraw_ust() {
//         let mut deps = th_setup(&[]);
//         let deposit_amount = 1000000u128;
//         let info = cosmwasm_std::testing::mock_info("depositor", &[coin(deposit_amount, "uusd")]);
//         deps.querier
//             .set_incentives_address(Addr::unchecked("incentives".to_string()));
//         deps.querier
//             .set_unclaimed_rewards("cosmos2contract".to_string(), Uint128::from(0u64));
//         // Set tax data
//         deps.querier.set_native_tax(
//             Decimal::from_ratio(1u128, 100u128),
//             &[(String::from("uusd"), Uint128::new(100u128))],
//         );

//         // ***** Setup *****

//         let mut env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_000_000_15),
//             ..Default::default()
//         });
//         // Create a lockdrop position for testing
//         let mut deposit_msg = ExecuteMsg::DepositUst { duration: 3u64 };
//         let mut deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "3"),
//                 attr("ust_deposited", "1000000")
//             ]
//         );
//         deposit_msg = ExecuteMsg::DepositUst { duration: 5u64 };
//         deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "5"),
//                 attr("ust_deposited", "1000000")
//             ]
//         );

//         // ***
//         // *** Test :: Error "Withdrawals not allowed" Reason :: Withdrawal attempt after the window is closed ***
//         // ***
//         env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(10_00_720_11),
//             ..Default::default()
//         });
//         let mut withdrawal_msg = ExecuteMsg::WithdrawUst {
//             amount: Uint256::from(100u64),
//             duration: 5u64,
//         };
//         let mut withdrawal_f = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             withdrawal_msg.clone(),
//         );
//         assert_generic_error_message(withdrawal_f, "Withdrawals not allowed");

//         // ***
//         // *** Test :: Error "Lockup doesn't exist" Reason :: Invalid lockup ***
//         // ***
//         env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(10_00_120_10),
//             ..Default::default()
//         });
//         withdrawal_msg = ExecuteMsg::WithdrawUst {
//             amount: Uint256::from(100u64),
//             duration: 4u64,
//         };
//         withdrawal_f = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             withdrawal_msg.clone(),
//         );
//         assert_generic_error_message(withdrawal_f, "Lockup doesn't exist");

//         // ***
//         // *** Test :: Error "Invalid withdrawal request" Reason :: Invalid amount ***
//         // ***
//         withdrawal_msg = ExecuteMsg::WithdrawUst {
//             amount: Uint256::from(100000000u64),
//             duration: 5u64,
//         };
//         withdrawal_f = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             withdrawal_msg.clone(),
//         );
//         assert_generic_error_message(withdrawal_f, "Invalid withdrawal request");

//         withdrawal_msg = ExecuteMsg::WithdrawUst {
//             amount: Uint256::from(0u64),
//             duration: 5u64,
//         };
//         withdrawal_f = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             withdrawal_msg.clone(),
//         );
//         assert_generic_error_message(withdrawal_f, "Invalid withdrawal request");

//         // ***
//         // *** Test #1 :: Successfully withdraw UST  ***
//         // ***
//         withdrawal_msg = ExecuteMsg::WithdrawUst {
//             amount: Uint256::from(42u64),
//             duration: 5u64,
//         };
//         let mut withdrawal_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             withdrawal_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             withdrawal_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::WithdrawUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "5"),
//                 attr("ust_withdrawn", "42")
//             ]
//         );
//         // let's verify the Lockdrop
//         let mut lockdrop_ =
//             query_lockup_info_with_id(deps.as_ref(), "depositor5".to_string()).unwrap();
//         assert_eq!(5u64, lockdrop_.duration);
//         assert_eq!(Uint256::from(999958u64), lockdrop_.ust_locked);
//         assert_eq!(103124010u64, lockdrop_.unlock_timestamp);
//         // let's verify the User
//         let mut user_ =
//             query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
//         assert_eq!(Uint256::from(1999958u64), user_.total_ust_locked);
//         assert_eq!(
//             vec!["depositor3".to_string(), "depositor5".to_string()],
//             user_.lockup_position_ids
//         );
//         // let's verify the state
//         let mut state_ = query_state(deps.as_ref()).unwrap();
//         assert_eq!(Uint256::from(1999958u64), state_.total_ust_locked);
//         assert_eq!(Uint256::from(719982u64), state_.total_deposits_weight);

//         // ***
//         // *** Test #2 :: Successfully withdraw UST  ***
//         // ***
//         withdrawal_msg = ExecuteMsg::WithdrawUst {
//             amount: Uint256::from(999958u64),
//             duration: 5u64,
//         };
//         withdrawal_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             withdrawal_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             withdrawal_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::WithdrawUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "5"),
//                 attr("ust_withdrawn", "999958")
//             ]
//         );
//         // let's verify the Lockdrop
//         lockdrop_ = query_lockup_info_with_id(deps.as_ref(), "depositor5".to_string()).unwrap();
//         assert_eq!(5u64, lockdrop_.duration);
//         assert_eq!(Uint256::from(0u64), lockdrop_.ust_locked);
//         assert_eq!(103124010u64, lockdrop_.unlock_timestamp);
//         // let's verify the User
//         user_ = query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
//         assert_eq!(Uint256::from(1000000u64), user_.total_ust_locked);
//         assert_eq!(vec!["depositor3".to_string()], user_.lockup_position_ids);
//         // let's verify the state
//         state_ = query_state(deps.as_ref()).unwrap();
//         assert_eq!(Uint256::from(1000000u64), state_.total_ust_locked);
//         assert_eq!(Uint256::from(270001u64), state_.total_deposits_weight);

//         // ***
//         // *** Test #3 :: Successfully withdraw UST  ***
//         // ***
//         withdrawal_msg = ExecuteMsg::WithdrawUst {
//             amount: Uint256::from(1000u64),
//             duration: 3u64,
//         };
//         withdrawal_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             withdrawal_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             withdrawal_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::WithdrawUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "3"),
//                 attr("ust_withdrawn", "1000")
//             ]
//         );
//         // let's verify the Lockdrop
//         lockdrop_ = query_lockup_info_with_id(deps.as_ref(), "depositor3".to_string()).unwrap();
//         assert_eq!(3u64, lockdrop_.duration);
//         assert_eq!(Uint256::from(999000u64), lockdrop_.ust_locked);
//         assert_eq!(101914410u64, lockdrop_.unlock_timestamp);
//         // let's verify the User
//         user_ = query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
//         assert_eq!(Uint256::from(999000u64), user_.total_ust_locked);
//         assert_eq!(vec!["depositor3".to_string()], user_.lockup_position_ids);
//         // let's verify the state
//         state_ = query_state(deps.as_ref()).unwrap();
//         assert_eq!(Uint256::from(999000u64), state_.total_ust_locked);
//         assert_eq!(Uint256::from(269731u64), state_.total_deposits_weight);
//     }

//     #[test]
//     fn test_deposit_ust_in_red_bank() {
//         let mut deps = th_setup(&[]);
//         let deposit_amount = 1000000u128;
//         let mut info =
//             cosmwasm_std::testing::mock_info("depositor", &[coin(deposit_amount, "uusd")]);
//         // Set tax data
//         deps.querier.set_native_tax(
//             Decimal::from_ratio(1u128, 100u128),
//             &[(String::from("uusd"), Uint128::new(100u128))],
//         );

//         // ***** Setup *****

//         let mut env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_000_000_15),
//             ..Default::default()
//         });
//         // Create a lockdrop position for testing
//         let mut deposit_msg = ExecuteMsg::DepositUst { duration: 3u64 };
//         let mut deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "3"),
//                 attr("ust_deposited", "1000000")
//             ]
//         );
//         deposit_msg = ExecuteMsg::DepositUst { duration: 5u64 };
//         deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "5"),
//                 attr("ust_deposited", "1000000")
//             ]
//         );

//         // ***
//         // *** Test :: Error "Unauthorized" ***
//         // ***
//         let deposit_in_redbank_msg = ExecuteMsg::DepositUstInRedBank {};
//         let deposit_in_redbank_response_f = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_in_redbank_msg.clone(),
//         );
//         assert_generic_error_message(deposit_in_redbank_response_f, "Unauthorized");

//         // ***
//         // *** Test :: Error "Lockdrop deposits haven't concluded yet" ***
//         // ***
//         info = mock_info("owner");
//         let mut deposit_in_redbank_response_f = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_in_redbank_msg.clone(),
//         );
//         assert_generic_error_message(
//             deposit_in_redbank_response_f,
//             "Lockdrop deposits haven't concluded yet",
//         );

//         env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_000_000_09),
//             ..Default::default()
//         });
//         deposit_in_redbank_response_f = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_in_redbank_msg.clone(),
//         );
//         assert_generic_error_message(
//             deposit_in_redbank_response_f,
//             "Lockdrop deposits haven't concluded yet",
//         );

//         env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_001_000_09),
//             ..Default::default()
//         });
//         deposit_in_redbank_response_f = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_in_redbank_msg.clone(),
//         );
//         assert_generic_error_message(
//             deposit_in_redbank_response_f,
//             "Lockdrop deposits haven't concluded yet",
//         );

//         // ***
//         // *** Successfully deposited ***
//         // ***
//         env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_001_000_11),
//             ..Default::default()
//         });
//         deps.querier.set_cw20_balances(
//             Addr::unchecked("ma_ust_token".to_string()),
//             &[(Addr::unchecked(MOCK_CONTRACT_ADDR), Uint128::new(0u128))],
//         );
//         let deposit_in_redbank_response_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_in_redbank_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_in_redbank_response_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::DepositInRedBank"),
//                 attr("ust_deposited_in_red_bank", "2000000"),
//                 attr("timestamp", "100100011")
//             ]
//         );
//         assert_eq!(
//             deposit_in_redbank_response_s.messages,
//             vec![
//                 SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
//                     contract_addr: "red_bank".to_string(),
//                     msg: to_binary(&mars::red_bank::msg::ExecuteMsg::DepositNative {
//                         denom: "uusd".to_string(),
//                     })
//                     .unwrap(),
//                     funds: vec![Coin {
//                         denom: "uusd".to_string(),
//                         amount: Uint128::from(1999900u128),
//                     }]
//                 })),
//                 SubMsg::new(
//                     CallbackMsg::UpdateStateOnRedBankDeposit {
//                         prev_ma_ust_balance: Uint256::from(0u64)
//                     }
//                     .to_cosmos_msg(&env.clone().contract.address)
//                     .unwrap()
//                 ),
//             ]
//         );
//         // let's verify the state
//         let state_ = query_state(deps.as_ref()).unwrap();
//         assert_eq!(Uint256::zero(), state_.final_ust_locked);
//         assert_eq!(Uint256::zero(), state_.final_maust_locked);
//         assert_eq!(Uint256::from(2000000u64), state_.total_ust_locked);
//         assert_eq!(Uint256::zero(), state_.total_maust_locked);
//         assert_eq!(Decimal256::zero(), state_.global_reward_index);
//         assert_eq!(Uint256::from(720000u64), state_.total_deposits_weight);
//     }

//     #[test]
//     fn test_update_state_on_red_bank_deposit_callback() {
//         let mut deps = th_setup(&[]);
//         let deposit_amount = 1000000u128;
//         let mut info =
//             cosmwasm_std::testing::mock_info("depositor", &[coin(deposit_amount, "uusd")]);
//         deps.querier
//             .set_incentives_address(Addr::unchecked("incentives".to_string()));
//         deps.querier
//             .set_unclaimed_rewards("cosmos2contract".to_string(), Uint128::from(0u64));
//         // Set tax data
//         deps.querier.set_native_tax(
//             Decimal::from_ratio(1u128, 100u128),
//             &[(String::from("uusd"), Uint128::new(100u128))],
//         );
//         deps.querier.set_cw20_balances(
//             Addr::unchecked("ma_ust_token".to_string()),
//             &[(
//                 Addr::unchecked(MOCK_CONTRACT_ADDR),
//                 Uint128::new(197000u128),
//             )],
//         );

//         // ***** Setup *****

//         let env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_000_000_15),
//             ..Default::default()
//         });
//         // Create a lockdrop position for testing
//         let mut deposit_msg = ExecuteMsg::DepositUst { duration: 3u64 };
//         let mut deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "3"),
//                 attr("ust_deposited", "1000000")
//             ]
//         );
//         deposit_msg = ExecuteMsg::DepositUst { duration: 5u64 };
//         deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "5"),
//                 attr("ust_deposited", "1000000")
//             ]
//         );

//         // ***
//         // *** Successfully updates the state post deposit in Red Bank ***
//         // ***
//         info = mock_info(&env.clone().contract.address.to_string());
//         let callback_msg = ExecuteMsg::Callback(CallbackMsg::UpdateStateOnRedBankDeposit {
//             prev_ma_ust_balance: Uint256::from(100u64),
//         });
//         let redbank_callback_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             callback_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             redbank_callback_s.attributes,
//             vec![
//                 attr("action", "lockdrop::CallbackMsg::RedBankDeposit"),
//                 attr("maUST_minted", "196900")
//             ]
//         );

//         // let's verify the state
//         let state_ = query_state(deps.as_ref()).unwrap();
//         // final : tracks Total UST deposited / Total MA-UST Minted
//         assert_eq!(Uint256::from(2000000u64), state_.final_ust_locked);
//         assert_eq!(Uint256::from(196900u64), state_.final_maust_locked);
//         // Total : tracks UST / MA-UST Available with the lockdrop contract
//         assert_eq!(Uint256::zero(), state_.total_ust_locked);
//         assert_eq!(Uint256::from(196900u64), state_.total_maust_locked);
//         // global_reward_index, total_deposits_weight :: Used for lockdrop / X-Mars distribution
//         assert_eq!(Decimal256::zero(), state_.global_reward_index);
//         assert_eq!(Uint256::from(720000u64), state_.total_deposits_weight);

//         // let's verify the User
//         let user_ = query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
//         assert_eq!(Uint256::from(2000000u64), user_.total_ust_locked);
//         assert_eq!(Uint256::from(196900u64), user_.total_maust_locked);
//         assert_eq!(false, user_.is_lockdrop_claimed);
//         assert_eq!(Decimal256::zero(), user_.reward_index);
//         assert_eq!(Uint256::zero(), user_.pending_xmars);
//         assert_eq!(
//             vec!["depositor3".to_string(), "depositor5".to_string()],
//             user_.lockup_position_ids
//         );

//         // let's verify the lockup #1
//         let mut lockdrop_ =
//             query_lockup_info_with_id(deps.as_ref(), "depositor3".to_string()).unwrap();
//         assert_eq!(3u64, lockdrop_.duration);
//         assert_eq!(Uint256::from(1000000u64), lockdrop_.ust_locked);
//         assert_eq!(Uint256::from(98450u64), lockdrop_.maust_balance);
//         assert_eq!(Uint256::from(8037158753u64), lockdrop_.lockdrop_reward);
//         assert_eq!(101914410u64, lockdrop_.unlock_timestamp);

//         // let's verify the lockup #2
//         lockdrop_ = query_lockup_info_with_id(deps.as_ref(), "depositor5".to_string()).unwrap();
//         assert_eq!(5u64, lockdrop_.duration);
//         assert_eq!(Uint256::from(1000000u64), lockdrop_.ust_locked);
//         assert_eq!(Uint256::from(98450u64), lockdrop_.maust_balance);
//         assert_eq!(Uint256::from(13395264589u64), lockdrop_.lockdrop_reward);
//         assert_eq!(103124010u64, lockdrop_.unlock_timestamp);
//     }

//     #[test]
//     fn test_try_claim() {
//         let mut deps = th_setup(&[]);
//         let deposit_amount = 1000000u128;
//         let mut info =
//             cosmwasm_std::testing::mock_info("depositor", &[coin(deposit_amount, "uusd")]);
//         // Set tax data
//         deps.querier.set_native_tax(
//             Decimal::from_ratio(1u128, 100u128),
//             &[(String::from("uusd"), Uint128::new(100u128))],
//         );
//         deps.querier
//             .set_incentives_address(Addr::unchecked("incentives".to_string()));

//         // ***** Setup *****

//         let mut env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_000_000_15),
//             ..Default::default()
//         });
//         // Create a lockdrop position for testing
//         let mut deposit_msg = ExecuteMsg::DepositUst { duration: 3u64 };
//         let mut deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "3"),
//                 attr("ust_deposited", "1000000")
//             ]
//         );
//         deposit_msg = ExecuteMsg::DepositUst { duration: 5u64 };
//         deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "5"),
//                 attr("ust_deposited", "1000000")
//             ]
//         );

//         // ***
//         // *** Test :: Error "Claim not allowed" ***
//         // ***
//         env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_001_000_09),
//             ..Default::default()
//         });
//         let claim_rewards_msg = ExecuteMsg::ClaimRewards {};
//         let mut claim_rewards_response_f = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             claim_rewards_msg.clone(),
//         );
//         assert_generic_error_message(claim_rewards_response_f, "Claim not allowed");

//         env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_001_000_09),
//             ..Default::default()
//         });
//         claim_rewards_response_f = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             claim_rewards_msg.clone(),
//         );
//         assert_generic_error_message(claim_rewards_response_f, "Claim not allowed");

//         // ***
//         // *** Test :: Error "No lockup to claim rewards for" ***
//         // ***
//         env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_001_001_09),
//             ..Default::default()
//         });
//         info = mock_info("not_depositor");
//         claim_rewards_response_f = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             claim_rewards_msg.clone(),
//         );
//         assert_generic_error_message(claim_rewards_response_f, "No lockup to claim rewards for");

//         // ***
//         // *** Test #1 :: Successfully Claim Rewards ***
//         // ***
//         deps.querier
//             .set_unclaimed_rewards("cosmos2contract".to_string(), Uint128::from(100u64));
//         deps.querier.set_cw20_balances(
//             Addr::unchecked("xmars_token".to_string()),
//             &[(Addr::unchecked(MOCK_CONTRACT_ADDR), Uint128::new(0u128))],
//         );
//         info = mock_info("depositor");
//         let mut claim_rewards_response_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             claim_rewards_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             claim_rewards_response_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::ClaimRewards"),
//                 attr("unclaimed_xMars", "100")
//             ]
//         );
//         assert_eq!(
//             claim_rewards_response_s.messages,
//             vec![
//                 SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
//                     contract_addr: "incentives".to_string(),
//                     msg: to_binary(&mars::incentives::msg::ExecuteMsg::ClaimRewards {}).unwrap(),
//                     funds: vec![]
//                 })),
//                 SubMsg::new(
//                     CallbackMsg::UpdateStateOnClaim {
//                         user: Addr::unchecked("depositor".to_string()),
//                         prev_xmars_balance: Uint256::from(0u64)
//                     }
//                     .to_cosmos_msg(&env.clone().contract.address)
//                     .unwrap()
//                 ),
//             ]
//         );

//         // ***
//         // *** Test #2 :: Successfully Claim Rewards (doesn't claim XMars as no rewards to claim) ***
//         // ***
//         deps.querier
//             .set_unclaimed_rewards("cosmos2contract".to_string(), Uint128::from(0u64));
//         deps.querier.set_cw20_balances(
//             Addr::unchecked("xmars_token".to_string()),
//             &[(Addr::unchecked(MOCK_CONTRACT_ADDR), Uint128::new(58460u128))],
//         );
//         claim_rewards_response_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             claim_rewards_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             claim_rewards_response_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::ClaimRewards"),
//                 attr("unclaimed_xMars", "0")
//             ]
//         );
//         assert_eq!(
//             claim_rewards_response_s.messages,
//             vec![SubMsg::new(
//                 CallbackMsg::UpdateStateOnClaim {
//                     user: Addr::unchecked("depositor".to_string()),
//                     prev_xmars_balance: Uint256::from(58460u64)
//                 }
//                 .to_cosmos_msg(&env.clone().contract.address)
//                 .unwrap()
//             ),]
//         );
//     }

//     #[test]
//     fn test_update_pool_on_dual_rewards_claim() {
//         let mut deps = th_setup(&[]);
//         let deposit_amount = 1000000u128;
//         let mut info =
//             cosmwasm_std::testing::mock_info("depositor", &[coin(deposit_amount, "uusd")]);
//         deps.querier
//             .set_unclaimed_rewards("cosmos2contract".to_string(), Uint128::from(0u64));
//         deps.querier
//             .set_incentives_address(Addr::unchecked("incentives".to_string()));
//         // Set tax data
//         deps.querier.set_native_tax(
//             Decimal::from_ratio(1u128, 100u128),
//             &[(String::from("uusd"), Uint128::new(100u128))],
//         );
//         deps.querier
//             .set_incentives_address(Addr::unchecked("incentives".to_string()));

//         // ***** Setup *****

//         let env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_000_000_15),
//             ..Default::default()
//         });
//         // Create some lockdrop positions for testing
//         let mut deposit_msg = ExecuteMsg::DepositUst { duration: 3u64 };
//         let mut deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "3"),
//                 attr("ust_deposited", "1000000")
//             ]
//         );
//         deposit_msg = ExecuteMsg::DepositUst { duration: 5u64 };
//         deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "5"),
//                 attr("ust_deposited", "1000000")
//             ]
//         );

//         info = cosmwasm_std::testing::mock_info("depositor2", &[coin(6450000u128, "uusd")]);
//         deposit_msg = ExecuteMsg::DepositUst { duration: 3u64 };
//         deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor2"),
//                 attr("duration", "3"),
//                 attr("ust_deposited", "6450000")
//             ]
//         );
//         deposit_msg = ExecuteMsg::DepositUst { duration: 5u64 };
//         deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor2"),
//                 attr("duration", "5"),
//                 attr("ust_deposited", "6450000")
//             ]
//         );

//         // *** Successfully updates the state post deposit in Red Bank ***
//         deps.querier.set_cw20_balances(
//             Addr::unchecked("ma_ust_token".to_string()),
//             &[(
//                 Addr::unchecked(MOCK_CONTRACT_ADDR),
//                 Uint128::new(197000u128),
//             )],
//         );
//         info = mock_info(&env.clone().contract.address.to_string());
//         let callback_msg = ExecuteMsg::Callback(CallbackMsg::UpdateStateOnRedBankDeposit {
//             prev_ma_ust_balance: Uint256::from(0u64),
//         });
//         let redbank_callback_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             callback_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             redbank_callback_s.attributes,
//             vec![
//                 attr("action", "lockdrop::CallbackMsg::RedBankDeposit"),
//                 attr("maUST_minted", "197000")
//             ]
//         );

//         // let's verify the state
//         let mut state_ = query_state(deps.as_ref()).unwrap();
//         // final : tracks Total UST deposited / Total MA-UST Minted
//         assert_eq!(Uint256::from(14900000u64), state_.final_ust_locked);
//         assert_eq!(Uint256::from(197000u64), state_.final_maust_locked);
//         // Total : tracks UST / MA-UST Available with the lockdrop contract
//         assert_eq!(Uint256::zero(), state_.total_ust_locked);
//         assert_eq!(Uint256::from(197000u64), state_.total_maust_locked);
//         // global_reward_index, total_deposits_weight :: Used for lockdrop / X-Mars distribution
//         assert_eq!(Decimal256::zero(), state_.global_reward_index);
//         assert_eq!(Uint256::from(5364000u64), state_.total_deposits_weight);

//         // ***
//         // *** Test #1 :: Successfully updates state on Reward claim (Claims both MARS and XMARS) ***
//         // ***

//         deps.querier.set_cw20_balances(
//             Addr::unchecked("xmars_token".to_string()),
//             &[(Addr::unchecked(MOCK_CONTRACT_ADDR), Uint128::new(58460u128))],
//         );
//         deps.querier.set_cw20_balances(
//             Addr::unchecked("mars_token".to_string()),
//             &[(
//                 Addr::unchecked(MOCK_CONTRACT_ADDR),
//                 Uint128::new(54568460u128),
//             )],
//         );

//         info = mock_info(&env.clone().contract.address.to_string());
//         let mut callback_msg = ExecuteMsg::Callback(CallbackMsg::UpdateStateOnClaim {
//             user: Addr::unchecked("depositor".to_string()),
//             prev_xmars_balance: Uint256::from(100u64),
//         });
//         let mut redbank_callback_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             callback_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             redbank_callback_s.attributes,
//             vec![
//                 attr("action", "lockdrop::CallbackMsg::ClaimRewards"),
//                 attr("total_xmars_claimed", "58360"),
//                 attr("user", "depositor"),
//                 attr("mars_claimed", "2876835347"),
//                 attr("xmars_claimed", "7833")
//             ]
//         );
//         assert_eq!(
//             redbank_callback_s.messages,
//             vec![
//                 SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
//                     contract_addr: "mars_token".to_string(),
//                     msg: to_binary(&cw20::Cw20ExecuteMsg::Transfer {
//                         recipient: "depositor".to_string(),
//                         amount: Uint128::from(2876835347u128),
//                     })
//                     .unwrap(),
//                     funds: vec![]
//                 })),
//                 SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
//                     contract_addr: "xmars_token".to_string(),
//                     msg: to_binary(&cw20::Cw20ExecuteMsg::Transfer {
//                         recipient: "depositor".to_string(),
//                         amount: Uint128::from(7833u128),
//                     })
//                     .unwrap(),
//                     funds: vec![]
//                 })),
//             ]
//         );
//         // let's verify the state
//         state_ = query_state(deps.as_ref()).unwrap();
//         assert_eq!(Uint256::zero(), state_.total_ust_locked);
//         assert_eq!(
//             Decimal256::from_ratio(58360u64, 197000u64),
//             state_.global_reward_index
//         );
//         // let's verify the User
//         let mut user_ =
//             query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
//         assert_eq!(Uint256::from(2000000u64), user_.total_ust_locked);
//         assert_eq!(Uint256::from(26442u64), user_.total_maust_locked);
//         assert_eq!(true, user_.is_lockdrop_claimed);
//         assert_eq!(
//             Decimal256::from_ratio(58360u64, 197000u64),
//             user_.reward_index
//         );
//         assert_eq!(Uint256::zero(), user_.pending_xmars);
//         assert_eq!(
//             vec!["depositor3".to_string(), "depositor5".to_string()],
//             user_.lockup_position_ids
//         );
//         // // let's verify user's lockup #1
//         let mut lockdrop_ =
//             query_lockup_info_with_id(deps.as_ref(), "depositor3".to_string()).unwrap();
//         assert_eq!(Uint256::from(1000000u64), lockdrop_.ust_locked);
//         assert_eq!(Uint256::from(13221u64), lockdrop_.maust_balance);
//         assert_eq!(Uint256::from(1078813255u64), lockdrop_.lockdrop_reward);
//         assert_eq!(101914410u64, lockdrop_.unlock_timestamp);
//         // // let's verify user's lockup #1
//         lockdrop_ = query_lockup_info_with_id(deps.as_ref(), "depositor5".to_string()).unwrap();
//         assert_eq!(Uint256::from(1000000u64), lockdrop_.ust_locked);
//         assert_eq!(Uint256::from(13221u64), lockdrop_.maust_balance);
//         assert_eq!(Uint256::from(1798022092u64), lockdrop_.lockdrop_reward);
//         assert_eq!(103124010u64, lockdrop_.unlock_timestamp);

//         // ***
//         // *** Test #2 :: Successfully updates state on Reward claim (Claims only XMARS) ***
//         // ***
//         deps.querier.set_cw20_balances(
//             Addr::unchecked("xmars_token".to_string()),
//             &[(
//                 Addr::unchecked(MOCK_CONTRACT_ADDR),
//                 Uint128::new(43534460u128),
//             )],
//         );
//         callback_msg = ExecuteMsg::Callback(CallbackMsg::UpdateStateOnClaim {
//             user: Addr::unchecked("depositor".to_string()),
//             prev_xmars_balance: Uint256::from(56430u64),
//         });
//         redbank_callback_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             callback_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             redbank_callback_s.attributes,
//             vec![
//                 attr("action", "lockdrop::CallbackMsg::ClaimRewards"),
//                 attr("total_xmars_claimed", "43478030"),
//                 attr("user", "depositor"),
//                 attr("mars_claimed", "0"),
//                 attr("xmars_claimed", "5835767")
//             ]
//         );
//         assert_eq!(
//             redbank_callback_s.messages,
//             vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
//                 contract_addr: "xmars_token".to_string(),
//                 msg: to_binary(&cw20::Cw20ExecuteMsg::Transfer {
//                     recipient: "depositor".to_string(),
//                     amount: Uint128::from(5835767u128),
//                 })
//                 .unwrap(),
//                 funds: vec![]
//             })),]
//         );
//         // let's verify the User
//         user_ = query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
//         assert_eq!(true, user_.is_lockdrop_claimed);
//         assert_eq!(Uint256::zero(), user_.pending_xmars);

//         // ***
//         // *** Test #3 :: Successfully updates state on Reward claim (Claims MARS and XMARS for 2nd depositor) ***
//         // ***
//         callback_msg = ExecuteMsg::Callback(CallbackMsg::UpdateStateOnClaim {
//             user: Addr::unchecked("depositor2".to_string()),
//             prev_xmars_balance: Uint256::from(0u64),
//         });
//         redbank_callback_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             callback_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             redbank_callback_s.attributes,
//             vec![
//                 attr("action", "lockdrop::CallbackMsg::ClaimRewards"),
//                 attr("total_xmars_claimed", "43534460"),
//                 attr("user", "depositor2"),
//                 attr("mars_claimed", "18555587994"),
//                 attr("xmars_claimed", "75383466")
//             ]
//         );
//         assert_eq!(
//             redbank_callback_s.messages,
//             vec![
//                 SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
//                     contract_addr: "mars_token".to_string(),
//                     msg: to_binary(&cw20::Cw20ExecuteMsg::Transfer {
//                         recipient: "depositor2".to_string(),
//                         amount: Uint128::from(18555587994u128),
//                     })
//                     .unwrap(),
//                     funds: vec![]
//                 })),
//                 SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
//                     contract_addr: "xmars_token".to_string(),
//                     msg: to_binary(&cw20::Cw20ExecuteMsg::Transfer {
//                         recipient: "depositor2".to_string(),
//                         amount: Uint128::from(75383466u128),
//                     })
//                     .unwrap(),
//                     funds: vec![]
//                 })),
//             ]
//         );
//         // let's verify the User
//         user_ = query_user_info(deps.as_ref(), env.clone(), "depositor2".to_string()).unwrap();
//         assert_eq!(Uint256::from(12900000u64), user_.total_ust_locked);
//         assert_eq!(Uint256::from(170557u64), user_.total_maust_locked);
//         assert_eq!(true, user_.is_lockdrop_claimed);
//         assert_eq!(Uint256::zero(), user_.pending_xmars);
//         assert_eq!(
//             vec!["depositor23".to_string(), "depositor25".to_string()],
//             user_.lockup_position_ids
//         );
//         // // let's verify user's lockup #1
//         lockdrop_ = query_lockup_info_with_id(deps.as_ref(), "depositor23".to_string()).unwrap();
//         assert_eq!(Uint256::from(6450000u64), lockdrop_.ust_locked);
//         assert_eq!(Uint256::from(85278u64), lockdrop_.maust_balance);
//         assert_eq!(Uint256::from(6958345498u64), lockdrop_.lockdrop_reward);
//         assert_eq!(101914410u64, lockdrop_.unlock_timestamp);
//         // // let's verify user's lockup #1
//         lockdrop_ = query_lockup_info_with_id(deps.as_ref(), "depositor25".to_string()).unwrap();
//         assert_eq!(Uint256::from(6450000u64), lockdrop_.ust_locked);
//         assert_eq!(Uint256::from(85278u64), lockdrop_.maust_balance);
//         assert_eq!(Uint256::from(11597242496u64), lockdrop_.lockdrop_reward);
//         assert_eq!(103124010u64, lockdrop_.unlock_timestamp);
//     }

//     #[test]
//     fn test_try_unlock_position() {
//         let mut deps = th_setup(&[]);
//         let deposit_amount = 1000000u128;
//         let mut info =
//             cosmwasm_std::testing::mock_info("depositor", &[coin(deposit_amount, "uusd")]);
//         // Set tax data
//         deps.querier.set_native_tax(
//             Decimal::from_ratio(1u128, 100u128),
//             &[(String::from("uusd"), Uint128::new(100u128))],
//         );
//         deps.querier
//             .set_incentives_address(Addr::unchecked("incentives".to_string()));

//         // ***** Setup *****

//         let mut env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_000_000_15),
//             ..Default::default()
//         });

//         // Create a lockdrop position for testing
//         let mut deposit_msg = ExecuteMsg::DepositUst { duration: 3u64 };
//         let mut deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "3"),
//                 attr("ust_deposited", "1000000")
//             ]
//         );
//         deposit_msg = ExecuteMsg::DepositUst { duration: 5u64 };
//         deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "5"),
//                 attr("ust_deposited", "1000000")
//             ]
//         );

//         // *** Successfully updates the state post deposit in Red Bank ***
//         deps.querier.set_cw20_balances(
//             Addr::unchecked("ma_ust_token".to_string()),
//             &[(
//                 Addr::unchecked(MOCK_CONTRACT_ADDR),
//                 Uint128::new(19700000u128),
//             )],
//         );
//         info = mock_info(&env.clone().contract.address.to_string());
//         let callback_msg = ExecuteMsg::Callback(CallbackMsg::UpdateStateOnRedBankDeposit {
//             prev_ma_ust_balance: Uint256::from(0u64),
//         });
//         execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             callback_msg.clone(),
//         )
//         .unwrap();

//         // ***
//         // *** Test :: Error "Invalid lockup" ***
//         // ***
//         let mut unlock_msg = ExecuteMsg::Unlock { duration: 4u64 };
//         let mut unlock_f = execute(deps.as_mut(), env.clone(), info.clone(), unlock_msg.clone());
//         assert_generic_error_message(unlock_f, "Invalid lockup");

//         // ***
//         // *** Test :: Error "{} seconds to Unlock" ***
//         // ***
//         info = mock_info("depositor");
//         env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_000_040_95),
//             ..Default::default()
//         });
//         unlock_msg = ExecuteMsg::Unlock { duration: 3u64 };
//         unlock_f = execute(deps.as_mut(), env.clone(), info.clone(), unlock_msg.clone());
//         assert_generic_error_message(unlock_f, "1910315 seconds to Unlock");

//         // ***
//         // *** Test :: Should unlock successfully ***
//         // ***
//         deps.querier
//             .set_incentives_address(Addr::unchecked("incentives".to_string()));
//         deps.querier
//             .set_unclaimed_rewards("cosmos2contract".to_string(), Uint128::from(8706700u64));
//         deps.querier.set_cw20_balances(
//             Addr::unchecked("xmars_token".to_string()),
//             &[(
//                 Addr::unchecked(MOCK_CONTRACT_ADDR),
//                 Uint128::new(19700000u128),
//             )],
//         );
//         env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_020_040_95),
//             ..Default::default()
//         });
//         let unlock_s =
//             execute(deps.as_mut(), env.clone(), info.clone(), unlock_msg.clone()).unwrap();
//         assert_eq!(
//             unlock_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::UnlockPosition"),
//                 attr("owner", "depositor"),
//                 attr("duration", "3"),
//                 attr("maUST_unlocked", "9850000")
//             ]
//         );
//         assert_eq!(
//             unlock_s.messages,
//             vec![
//                 SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
//                     contract_addr: "incentives".to_string(),
//                     msg: to_binary(&mars::incentives::msg::ExecuteMsg::ClaimRewards {}).unwrap(),
//                     funds: vec![]
//                 })),
//                 SubMsg::new(
//                     CallbackMsg::UpdateStateOnClaim {
//                         user: Addr::unchecked("depositor".to_string()),
//                         prev_xmars_balance: Uint256::from(19700000u64)
//                     }
//                     .to_cosmos_msg(&env.clone().contract.address)
//                     .unwrap()
//                 ),
//                 SubMsg::new(
//                     CallbackMsg::DissolvePosition {
//                         user: Addr::unchecked("depositor".to_string()),
//                         duration: 3u64
//                     }
//                     .to_cosmos_msg(&env.clone().contract.address)
//                     .unwrap()
//                 ),
//                 SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
//                     contract_addr: "ma_ust_token".to_string(),
//                     msg: to_binary(&cw20::Cw20ExecuteMsg::Transfer {
//                         recipient: "depositor".to_string(),
//                         amount: Uint128::from(9850000u128),
//                     })
//                     .unwrap(),
//                     funds: vec![]
//                 })),
//             ]
//         );
//     }

//     #[test]
//     fn test_try_dissolve_position() {
//         let mut deps = th_setup(&[]);
//         let deposit_amount = 1000000u128;
//         let mut info =
//             cosmwasm_std::testing::mock_info("depositor", &[coin(deposit_amount, "uusd")]);
//         deps.querier
//             .set_incentives_address(Addr::unchecked("incentives".to_string()));
//         // Set tax data
//         deps.querier.set_native_tax(
//             Decimal::from_ratio(1u128, 100u128),
//             &[(String::from("uusd"), Uint128::new(100u128))],
//         );
//         deps.querier
//             .set_incentives_address(Addr::unchecked("incentives".to_string()));

//         // ***** Setup *****

//         let env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_000_000_15),
//             ..Default::default()
//         });

//         // Create a lockdrop position for testing
//         let mut deposit_msg = ExecuteMsg::DepositUst { duration: 3u64 };
//         let mut deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "3"),
//                 attr("ust_deposited", "1000000")
//             ]
//         );
//         deposit_msg = ExecuteMsg::DepositUst { duration: 5u64 };
//         deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "5"),
//                 attr("ust_deposited", "1000000")
//             ]
//         );

//         // *** Successfully updates the state post deposit in Red Bank ***
//         deps.querier.set_cw20_balances(
//             Addr::unchecked("ma_ust_token".to_string()),
//             &[(
//                 Addr::unchecked(MOCK_CONTRACT_ADDR),
//                 Uint128::new(19700000u128),
//             )],
//         );
//         info = mock_info(&env.clone().contract.address.to_string());
//         let callback_msg = ExecuteMsg::Callback(CallbackMsg::UpdateStateOnRedBankDeposit {
//             prev_ma_ust_balance: Uint256::from(0u64),
//         });
//         execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             callback_msg.clone(),
//         )
//         .unwrap();

//         // ***
//         // *** Test #1 :: Should successfully dissolve the position ***
//         // ***
//         let mut callback_dissolve_msg = ExecuteMsg::Callback(CallbackMsg::DissolvePosition {
//             user: Addr::unchecked("depositor".to_string()),
//             duration: 3u64,
//         });
//         let mut dissolve_position_callback_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             callback_dissolve_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             dissolve_position_callback_s.attributes,
//             vec![
//                 attr("action", "lockdrop::Callback::DissolvePosition"),
//                 attr("user", "depositor"),
//                 attr("duration", "3"),
//             ]
//         );
//         // let's verify the state
//         let mut state_ = query_state(deps.as_ref()).unwrap();
//         assert_eq!(Uint256::from(2000000u64), state_.final_ust_locked);
//         assert_eq!(Uint256::from(19700000u64), state_.final_maust_locked);
//         assert_eq!(Uint256::from(9850000u64), state_.total_maust_locked);
//         assert_eq!(Uint256::from(720000u64), state_.total_deposits_weight);
//         // let's verify the User
//         deps.querier
//             .set_unclaimed_rewards("cosmos2contract".to_string(), Uint128::from(0u64));
//         let mut user_ =
//             query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
//         assert_eq!(Uint256::from(1000000u64), user_.total_ust_locked);
//         assert_eq!(Uint256::from(9850000u64), user_.total_maust_locked);
//         assert_eq!(vec!["depositor5".to_string()], user_.lockup_position_ids);
//         // let's verify user's lockup #1 (which is dissolved)
//         let mut lockdrop_ =
//             query_lockup_info_with_id(deps.as_ref(), "depositor3".to_string()).unwrap();
//         assert_eq!(Uint256::from(0u64), lockdrop_.ust_locked);
//         assert_eq!(Uint256::from(0u64), lockdrop_.maust_balance);

//         // ***
//         // *** Test #2 :: Should successfully dissolve the position ***
//         // ***
//         callback_dissolve_msg = ExecuteMsg::Callback(CallbackMsg::DissolvePosition {
//             user: Addr::unchecked("depositor".to_string()),
//             duration: 5u64,
//         });
//         dissolve_position_callback_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             callback_dissolve_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             dissolve_position_callback_s.attributes,
//             vec![
//                 attr("action", "lockdrop::Callback::DissolvePosition"),
//                 attr("user", "depositor"),
//                 attr("duration", "5"),
//             ]
//         );
//         // let's verify the state
//         state_ = query_state(deps.as_ref()).unwrap();
//         assert_eq!(Uint256::from(2000000u64), state_.final_ust_locked);
//         assert_eq!(Uint256::from(19700000u64), state_.final_maust_locked);
//         assert_eq!(Uint256::from(0u64), state_.total_maust_locked);
//         assert_eq!(Uint256::from(720000u64), state_.total_deposits_weight);
//         // let's verify the User
//         user_ = query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
//         assert_eq!(Uint256::from(0u64), user_.total_ust_locked);
//         assert_eq!(Uint256::from(0u64), user_.total_maust_locked);
//         // let's verify user's lockup #1 (which is dissolved)
//         lockdrop_ = query_lockup_info_with_id(deps.as_ref(), "depositor5".to_string()).unwrap();
//         assert_eq!(Uint256::from(0u64), lockdrop_.ust_locked);
//         assert_eq!(Uint256::from(0u64), lockdrop_.maust_balance);
//     }

//     fn th_setup(contract_balances: &[Coin]) -> OwnedDeps<MockStorage, MockApi, MarsMockQuerier> {
//         let mut deps = mock_dependencies(contract_balances);
//         let info = mock_info("owner");
//         let env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_000_000_00),
//             ..Default::default()
//         });
//         // Config with valid base params
//         let base_config = InstantiateMsg {
//             owner: "owner".to_string(),
//             address_provider: Some("address_provider".to_string()),
//             ma_ust_token: Some("ma_ust_token".to_string()),
//             init_timestamp: 1_000_000_10,
//             deposit_window: 100000u64,
//             withdrawal_window: 72000u64,
//             min_duration: 3u64,
//             max_duration: 9u64,
//             seconds_per_week: 7 * 86400 as u64, 
//             denom: Some("uusd".to_string()),
//             weekly_multiplier: Some(Decimal256::from_ratio(9u64, 100u64)),
//             lockdrop_incentives: Some(Uint256::from(21432423343u64)),
//         };
//         instantiate(deps.as_mut(), env, info, base_config).unwrap();
//         deps
//     }
// }
