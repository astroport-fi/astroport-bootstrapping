use std::cmp::Ordering;

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, from_binary, to_binary, Addr, Binary, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env,
    MessageInfo, Order, Response, StdError, StdResult, SubMsg, Uint128, WasmMsg,
};

use astroport_periphery::airdrop::ExecuteMsg::EnableClaims as AirdropEnableClaims;
use astroport_periphery::auction::{
    CallbackMsg, ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, StateResponse,
    UpdateConfigMsg, UserInfoResponse,
};
use astroport_periphery::helpers::{build_approve_cw20_msg, cw20_get_balance};
use astroport_periphery::lockdrop::ExecuteMsg::EnableClaims as LockdropEnableClaims;

use astroport::asset::{Asset, AssetInfo, PairInfo};
use astroport::generator::{
    ExecuteMsg as GenExecuteMsg, PendingTokenResponse, QueryMsg as GenQueryMsg, RewardInfoResponse,
};
use astroport::pair::QueryMsg as AstroportPairQueryMsg;

use crate::state::{Config, State, UserInfo, CONFIG, STATE, USERS};
use astroport::querier::query_token_balance;
use cw20::{BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg, Cw20ReceiveMsg};

const UUSD_DENOM: &str = "uusd";

// TODO: there will be orphan generator ASTROs and proxy rewards because of rounding

//----------------------------------------------------------------------------------------
// Entry points
//----------------------------------------------------------------------------------------

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let pair_info: PairInfo = deps
        .querier
        .query_wasm_smart(&msg.astro_ust_pair_address, &AstroportPairQueryMsg::Pair {})?;

    let config = Config {
        owner: msg
            .owner
            .map(|v| deps.api.addr_validate(&v))
            .transpose()?
            .unwrap_or(info.sender),
        astro_token_address: deps.api.addr_validate(&msg.astro_token_address)?,
        airdrop_contract_address: deps.api.addr_validate(&msg.airdrop_contract_address)?,
        lockdrop_contract_address: deps.api.addr_validate(&msg.lockdrop_contract_address)?,
        astro_ust_pool_address: deps.api.addr_validate(&msg.astro_ust_pair_address)?,
        astro_ust_lp_token_address: pair_info.liquidity_token,
        generator_contract: deps.api.addr_validate(&msg.generator_contract_address)?,
        astro_incentive_amount: None,
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
        ExecuteMsg::UpdateConfig { new_config } => {
            handle_update_config(deps, env, info, new_config)
        }
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, msg),
        ExecuteMsg::DepositUst {} => handle_deposit_ust(deps, env, info),
        ExecuteMsg::WithdrawUst { amount } => handle_withdraw_ust(deps, env, info, amount),
        ExecuteMsg::InitPool { slippage } => handle_init_pool(deps, env, info, slippage),
        ExecuteMsg::StakeLpTokens {} => handle_stake_lp_tokens(deps, env, info),

        ExecuteMsg::ClaimRewards { withdraw_lp_shares } => {
            handle_claim_rewards_and_withdraw_lp_shares(deps, env, info, withdraw_lp_shares)
        }
        ExecuteMsg::Callback(msg) => handle_callback(deps, env, info, msg),
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
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
            handle_delegate_astro_tokens(deps, env, user_address, cw20_msg.amount)
        }
    }
}

fn handle_callback(
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
            prev_astro_balance,
            prev_proxy_reward_balance,
        } => update_state_on_reward_claim(deps, env, prev_astro_balance, prev_proxy_reward_balance),
        CallbackMsg::WithdrawUserRewardsCallback {
            user_address,
            withdraw_lp_shares,
        } => {
            callback_withdraw_user_rewards_and_optionally_lp(deps, user_address, withdraw_lp_shares)
        }
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
    env: Env,
    info: MessageInfo,
    new_config: UpdateConfigMsg,
) -> StdResult<Response> {
    let mut config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;
    let mut messages: Vec<WasmMsg> = vec![];
    let mut attributes = vec![attr("action", "update_config")];

    // CHECK :: ONLY OWNER CAN CALL THIS FUNCTION
    if info.sender != config.owner {
        return Err(StdError::generic_err("Only owner can update configuration"));
    }

    // UPDATE :: ADDRESSES IF PROVIDED
    if let Some(owner) = new_config.owner {
        config.owner = deps.api.addr_validate(&owner)?;
    }

    if let Some(generator_contract) = new_config.generator_contract {
        config.generator_contract = deps.api.addr_validate(&generator_contract)?;
    }

    if let Some(new_incentives) = new_config.astro_incentive_amount {
        if state.lp_shares_minted.is_some() {
            return Err(StdError::generic_err("ASTRO is already being distributed"));
        };
        let prev_incentives = config.astro_incentive_amount.unwrap_or_default();
        match prev_incentives.cmp(&new_incentives) {
            Ordering::Equal => {}
            Ordering::Greater => {
                let amount = prev_incentives - new_incentives;
                messages.push(WasmMsg::Execute {
                    contract_addr: config.astro_token_address.to_string(),
                    funds: vec![],
                    msg: to_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: info.sender.to_string(),
                        amount,
                    })?,
                });
                attributes.push(attr("incentives returned", amount));
            }
            Ordering::Less => {
                let amount = new_incentives - prev_incentives;
                messages.push(WasmMsg::Execute {
                    contract_addr: config.astro_token_address.to_string(),
                    funds: vec![],
                    msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                        owner: info.sender.to_string(),
                        recipient: env.contract.address.to_string(),
                        amount,
                    })?,
                });
                attributes.push(attr("incentives received", amount));
            }
        };
        config.astro_incentive_amount = Some(new_incentives);
    }

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new()
        .add_messages(messages)
        .add_attributes(attributes))
}

/// @dev Accepts ASTRO tokens to be used for the LP Bootstrapping via auction. Callable only by Airdrop / Lockdrop contracts
/// @param user_address : User address who is delegating the ASTRO tokens for LP Pool bootstrap via auction
/// @param amount : Number of ASTRO Tokens being deposited
pub fn handle_delegate_astro_tokens(
    deps: DepsMut,
    env: Env,
    user_address: String,
    amount: Uint128,
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;

    let user_address = deps.api.addr_validate(&user_address)?;

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
    if info.funds.len() != 1 || info.funds[0].denom != UUSD_DENOM {
        return Err(StdError::generic_err("You may delegate USD coin only"));
    }

    let fund = &info.funds[0];

    // CHECK ::: Amount needs to be valid
    if fund.amount.is_zero() {
        return Err(StdError::generic_err("Amount must be greater than 0"));
    }

    // UPDATE STATE
    state.total_ust_delegated += fund.amount;
    user_info.ust_delegated += fund.amount;

    // SAVE UPDATED STATE
    STATE.save(deps.storage, &state)?;
    USERS.save(deps.storage, &info.sender, &user_info)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "Auction::ExecuteMsg::DelegateUst"),
        attr("user", info.sender.to_string()),
        attr("ust_delegated", fund.amount),
    ]))
}

/// true if deposits are allowed
fn is_deposit_open(current_timestamp: u64, config: &Config) -> bool {
    current_timestamp >= config.init_timestamp
        && current_timestamp < config.init_timestamp + config.deposit_window
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

    let mut user_info = USERS.load(deps.storage, &user_address)?;

    // CHECK :: Has the user already withdrawn during the current window
    if user_info.ust_withdrawn {
        return Err(StdError::generic_err("Max 1 withdrawal allowed"));
    }

    // Check :: Amount should be within the allowed withdrawal limit bounds
    let max_withdrawal_percent = allowed_withdrawal_percent(env.block.time.seconds(), &config);
    let max_withdrawal_allowed = user_info.ust_delegated * max_withdrawal_percent;

    if amount > max_withdrawal_allowed {
        return Err(StdError::generic_err(format!(
            "Amount exceeds maximum allowed withdrawal limit of {}",
            max_withdrawal_percent
        )));
    }

    // After deposit window is closed, we allow to withdraw only once
    if env.block.time.seconds() >= config.init_timestamp + config.deposit_window {
        user_info.ust_withdrawn = true;
    }

    // UPDATE STATE
    state.total_ust_delegated -= amount;
    user_info.ust_delegated -= amount;

    // SAVE UPDATED STATE
    STATE.save(deps.storage, &state)?;
    USERS.save(deps.storage, &user_address, &user_info)?;

    // Transfer UST to the user
    let transfer_ust = Asset {
        amount,
        info: AssetInfo::NativeToken {
            denom: String::from(UUSD_DENOM),
        },
    };

    Ok(Response::new()
        .add_attributes(vec![
            attr("action", "Auction::ExecuteMsg::WithdrawUst"),
            attr("user", user_address.to_string()),
            attr("ust_withdrawn", amount),
            attr("ust_comission", transfer_ust.compute_tax(&deps.querier)?),
        ])
        .add_message(transfer_ust.into_msg(&deps.querier, user_address)?))
}

///  @dev Helper function to calculate maximum % of their total UST deposited that can be withdrawn
/// Returns % UST that can be withdrawn and 'more_withdrawals_allowed' boolean which indicates whether more withdrawls by the user
/// will be allowed or not
fn allowed_withdrawal_percent(current_timestamp: u64, config: &Config) -> Decimal {
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
    } else {
        // Withdrawals not allowed
        Decimal::from_ratio(0u32, 100u32)
    }
}

/// @dev Admin function to bootstrap the ASTRO-UST Liquidity pool by depositing all ASTRO, UST tokens deposited to the Astroport pool
/// @param slippage Optional, to handle slippage that may be there when adding liquidity to the pool
pub fn handle_init_pool(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    slippage: Option<Decimal>,
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;

    // CHECK :: Only admin can call this function
    if info.sender != config.owner {
        return Err(StdError::generic_err("Unauthorized"));
    }

    if state.lp_shares_minted.is_some() {
        return Err(StdError::generic_err("Liquidity already added"));
    }

    // CHECK :: Deposit / withdrawal windows need to be over
    if !are_windows_closed(env.block.time.seconds(), &config) {
        return Err(StdError::generic_err(
            "Deposit/withdrawal windows are still open",
        ));
    }

    let astro_balance = {
        let res: cw20::BalanceResponse = deps.querier.query_wasm_smart(
            &config.astro_token_address,
            &Cw20QueryMsg::Balance {
                address: env.contract.address.to_string(),
            },
        )?;
        res.balance
    };

    let ust_coin = deps
        .querier
        .query_balance(&env.contract.address, UUSD_DENOM)?;

    let mut msgs = vec![];

    // QUERY CURRENT LP TOKEN BALANCE (FOR SAFETY - IN ANY CASE)
    let cur_lp_balance = query_token_balance(
        &deps.querier,
        config.astro_ust_lp_token_address.clone(),
        env.contract.address.clone(),
    )?;

    // COSMOS MSGS
    // :: 1.  APPROVE ASTRO WITH LP POOL ADDRESS AS BENEFICIARY
    // :: 2.  ADD LIQUIDITY
    // :: 3. CallbackMsg :: Update state on liquidity addition to LP Pool
    msgs.push(build_approve_cw20_msg(
        config.astro_token_address.to_string(),
        config.astro_ust_pool_address.to_string(),
        astro_balance,
    )?);

    msgs.push(build_provide_liquidity_to_lp_pool_msg(
        deps.as_ref(),
        &config,
        ust_coin.amount,
        astro_balance,
        slippage,
    )?);

    msgs.push(
        CallbackMsg::UpdateStateOnLiquidityAdditionToPool {
            prev_lp_balance: cur_lp_balance,
        }
        .to_cosmos_msg(&env)?,
    );

    Ok(Response::new().add_messages(msgs).add_attributes(vec![
        attr("action", "Auction::ExecuteMsg::AddLiquidityToAstroportPool"),
        attr("astro_provided", astro_balance),
        attr("ust_provided", ust_coin.amount),
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
            contract_addr: config.astro_token_address.clone(),
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
            auto_stack: None,
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

    let lp_shares_minted = state
        .lp_shares_minted
        .expect("Should be provided to the ASTRO/UST pool!");

    // Init response
    let mut response = Response::new()
        .add_attribute("action", "Auction::ExecuteMsg::StakeLPTokens")
        .add_attribute("staked_amount", lp_shares_minted);

    // COSMOS MSGs
    // :: Add increase allowance msg so generator contract can transfer tokens to itself
    // :: To stake LP Tokens to the Astroport generator contract
    response.messages.push(SubMsg::new(build_approve_cw20_msg(
        config.astro_ust_lp_token_address.to_string(),
        config.generator_contract.to_string(),
        lp_shares_minted,
    )?));
    response.messages.push(SubMsg::new(WasmMsg::Execute {
        contract_addr: config.generator_contract.to_string(),
        msg: to_binary(&astroport::generator::ExecuteMsg::Deposit {
            lp_token: config.astro_ust_lp_token_address,
            amount: lp_shares_minted,
        })?,
        funds: vec![],
    }));

    state.is_lp_staked = true;
    STATE.save(deps.storage, &state)?;

    Ok(response)
}

/// @dev Facilitates ASTRO Reward claim for users
pub fn handle_claim_rewards_and_withdraw_lp_shares(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    withdraw_lp_shares: Option<Uint128>,
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;
    let user_address = info.sender;
    let user_info = USERS.load(deps.storage, &user_address)?;

    // CHECK :: User has valid delegation / deposit balances
    if user_info.astro_delegated.is_zero() && user_info.ust_delegated.is_zero() {
        return Err(StdError::generic_err("No delegated assets"));
    }

    if let Some(withdraw_lp_shares) = withdraw_lp_shares {
        let max_withdrawable =
            calculate_withdrawable_lp_shares(env.block.time.seconds(), &config, &state, &user_info);
        if withdraw_lp_shares > max_withdrawable {
            return Err(StdError::generic_err("No available LP shares to withdraw"));
        }
    }

    let mut cosmos_msgs = vec![];

    if state.lp_shares_minted.is_some() {
        if state.is_lp_staked {
            // QUERY :: Check if there are any pending staking rewards
            let pending_rewards: PendingTokenResponse = deps.querier.query_wasm_smart(
                &config.generator_contract,
                &GenQueryMsg::PendingToken {
                    lp_token: config.astro_ust_lp_token_address.clone(),
                    user: env.contract.address.clone(),
                },
            )?;

            if !pending_rewards.pending.is_zero()
                || (pending_rewards.pending_on_proxy.is_some()
                    && !pending_rewards.pending_on_proxy.unwrap().is_zero())
            {
                let rwi: RewardInfoResponse = deps.querier.query_wasm_smart(
                    &config.generator_contract,
                    &GenQueryMsg::RewardInfo {
                        lp_token: config.astro_ust_lp_token_address.clone(),
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
                    contract_addr: config.generator_contract.to_string(),
                    funds: vec![],
                    msg: to_binary(&GenExecuteMsg::Withdraw {
                        lp_token: config.astro_ust_lp_token_address,
                        amount: Uint128::zero(),
                    })?,
                }));

                cosmos_msgs.push(
                    CallbackMsg::UpdateStateOnRewardClaim {
                        prev_astro_balance: astro_balance,
                        prev_proxy_reward_balance: proxy_reward_balance,
                    }
                    .to_cosmos_msg(&env)?,
                );
            };
        }
    } else {
        return Err(StdError::generic_err(
            "Astro/USD should be provided to the pool!",
        ));
    };

    cosmos_msgs.push(
        CallbackMsg::WithdrawUserRewardsCallback {
            user_address,
            withdraw_lp_shares,
        }
        .to_cosmos_msg(&env)?,
    );

    Ok(Response::new().add_messages(cosmos_msgs))
}

/// @dev CALLBACK Function to withdraw user rewards and LP Tokens if available
/// @param user_address : User address who is withdrawing
/// @param withdraw_lp_shares : Optional amount to withdraw lp tokens
pub fn callback_withdraw_user_rewards_and_optionally_lp(
    deps: DepsMut,
    user_address: Addr,
    withdraw_lp_shares: Option<Uint128>,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;
    let mut user_info = USERS.load(deps.storage, &user_address)?;

    let mut cosmos_msgs = vec![];
    let mut attributes = vec![
        attr("action", "Withdraw rewards and lp tokens"),
        attr("user_address", &user_address),
    ];

    let astroport_lp_amount = user_info.lp_shares - user_info.claimed_lp_shares;

    if !user_info.astro_incentive_transfered {
        cosmos_msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.astro_token_address.to_string(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: user_address.to_string(),
                amount: user_info.auction_incentive_amount,
            })?,
        }));
        user_info.astro_incentive_transfered = true;
        attributes.push(attr(
            "auction_astro_reward",
            user_info.auction_incentive_amount,
        ));
    }

    if state.is_lp_staked {
        let rwi: RewardInfoResponse = deps.querier.query_wasm_smart(
            &config.generator_contract,
            &GenQueryMsg::RewardInfo {
                lp_token: config.astro_ust_lp_token_address.clone(),
            },
        )?;

        let total_user_astro_rewards = state.generator_astro_per_share * astroport_lp_amount;
        let pending_astro_rewards = total_user_astro_rewards - user_info.generator_astro_debt;
        user_info.generator_astro_debt = total_user_astro_rewards;

        cosmos_msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: rwi.base_reward_token.to_string(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: user_address.to_string(),
                amount: pending_astro_rewards,
            })?,
        }));
        attributes.push(attr("generator_astro_reward", pending_astro_rewards));

        if let Some(proxy_reward_token) = rwi.proxy_reward_token {
            let total_user_proxy_rewards = state.generator_proxy_per_share * astroport_lp_amount;
            let pending_proxy_rewards = total_user_proxy_rewards - user_info.generator_proxy_debt;
            user_info.generator_proxy_debt = total_user_proxy_rewards;

            cosmos_msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: proxy_reward_token.to_string(),
                funds: vec![],
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: user_address.to_string(),
                    amount: pending_proxy_rewards,
                })?,
            }));
            attributes.push(attr("generator_proxy_reward", pending_proxy_rewards));
        }

        //  COSMOSMSG :: If LP Tokens are staked, we unstake the amount which needs to be returned to the user
        if let Some(withdrawn_lp_shares) = withdraw_lp_shares {
            cosmos_msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.generator_contract.to_string(),
                funds: vec![],
                msg: to_binary(&GenExecuteMsg::Withdraw {
                    lp_token: config.astro_ust_lp_token_address.clone(),
                    amount: withdrawn_lp_shares,
                })?,
            }));
        }
    }

    if let Some(withdrawn_lp_shares) = withdraw_lp_shares {
        cosmos_msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.astro_ust_lp_token_address.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: user_address.to_string(),
                amount: withdrawn_lp_shares,
            })?,
            funds: vec![],
        }));
        attributes.push(attr("lp_withdrawn", withdrawn_lp_shares));
        user_info.claimed_lp_shares += withdrawn_lp_shares;

        if user_info.lp_shares == user_info.claimed_lp_shares {
            USERS.remove(deps.storage, &user_address);
        } else {
            USERS.save(deps.storage, &user_address, &user_info)?;
        };
    } else {
        USERS.save(deps.storage, &user_address, &user_info)?;
    }

    Ok(Response::new()
        .add_messages(cosmos_msgs)
        .add_attributes(attributes))
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
    state.lp_shares_minted = Some(cur_lp_balance - prev_lp_balance);
    state.pool_init_timestamp = env.block.time.seconds();
    STATE.save(deps.storage, &state)?;

    // Distribute lp tokens and incentives to users.
    // We have to distribute to every user at once
    // not to leave amounts at the end because of rounding
    let mut lp_to_distribute = cur_lp_balance;
    let astro_incentive_amount = config
        .astro_incentive_amount
        .expect("Astro incentives should be set");
    let mut astro_to_distribute = astro_incentive_amount;

    let user_addrs: Vec<Addr> = USERS
        .keys(deps.storage, None, None, Order::Ascending)
        .map(|v| Addr::unchecked(String::from_utf8(v).expect("Addr deserialization error!")))
        .collect();

    if user_addrs.len() > 1 {
        for key in user_addrs.iter().take(user_addrs.len() - 1) {
            let mut user_info = USERS.load(deps.storage, key)?;
            user_info.lp_shares = (Decimal::from_ratio(
                user_info.astro_delegated,
                state.total_astro_delegated * Uint128::new(2),
            ) + Decimal::from_ratio(
                user_info.ust_delegated,
                state.total_ust_delegated * Uint128::new(2),
            )) * cur_lp_balance;
            lp_to_distribute -= user_info.lp_shares;

            user_info.auction_incentive_amount =
                Decimal::from_ratio(user_info.lp_shares, cur_lp_balance) * astro_incentive_amount;
            astro_to_distribute -= user_info.auction_incentive_amount;
            USERS.save(deps.storage, key, &user_info)?;
        }
    }

    if !user_addrs.is_empty() {
        let key = &user_addrs[user_addrs.len() - 1];
        let mut user_info = USERS.load(deps.storage, key)?;
        user_info.lp_shares = lp_to_distribute;
        user_info.auction_incentive_amount = astro_to_distribute;
        USERS.save(deps.storage, key, &user_info)?;
    }

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

    Ok(Response::new()
        .add_messages(cosmos_msgs)
        .add_attributes(vec![
            (
                "action",
                "Auction::CallbackMsg::UpdateStateOnLiquidityAddition",
            ),
            ("lp_shares_minted", &cur_lp_balance.to_string()),
            (
                "pool_init_timestamp",
                &state.pool_init_timestamp.to_string(),
            ),
        ]))
}

// @dev CallbackMsg :: Facilitates state update and ASTRO rewards transfer to users post ASTRO incentives claim from the generator contract
pub fn update_state_on_reward_claim(
    deps: DepsMut,
    env: Env,
    prev_astro_balance: Uint128,
    prev_proxy_reward_balance: Option<Uint128>,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    let rwi: RewardInfoResponse = deps.querier.query_wasm_smart(
        &config.generator_contract,
        &GenQueryMsg::RewardInfo {
            lp_token: config.astro_ust_lp_token_address.clone(),
        },
    )?;

    let lp_balance: Uint128 = deps.querier.query_wasm_smart(
        &config.generator_contract,
        &GenQueryMsg::Deposit {
            lp_token: config.astro_ust_lp_token_address.clone(),
            user: env.contract.address.clone(),
        },
    )?;

    let base_reward_received;
    state.generator_astro_per_share = state.generator_astro_per_share + {
        let res: BalanceResponse = deps.querier.query_wasm_smart(
            rwi.base_reward_token,
            &Cw20QueryMsg::Balance {
                address: env.contract.address.to_string(),
            },
        )?;
        base_reward_received = res.balance - prev_astro_balance;
        Decimal::from_ratio(base_reward_received, lp_balance)
    };

    let mut proxy_reward_received = Uint128::zero();
    state.generator_proxy_per_share = state.generator_proxy_per_share + {
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
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("astro_reward_received", base_reward_received)
        .add_attribute("proxy_reward_received", proxy_reward_received)
        .add_attribute(
            "generator_astro_per_share",
            state.generator_astro_per_share.to_string(),
        )
        .add_attribute(
            "generator_proxy_per_share",
            state.generator_proxy_per_share.to_string(),
        ))
}

/// @dev Helper function. Returns true if the deposit & withdrawal windows are closed, else returns false
/// @param current_timestamp : Current timestamp
/// @param config : Configuration
fn are_windows_closed(current_timestamp: u64, config: &Config) -> bool {
    let window_end = config.init_timestamp + config.deposit_window + config.withdrawal_window;
    current_timestamp >= window_end
}

/// Returns LP Balance  that a user can withdraw based on a vesting schedule
pub fn calculate_withdrawable_lp_shares(
    cur_timestamp: u64,
    config: &Config,
    state: &State,
    user_info: &UserInfo,
) -> Uint128 {
    let time_elapsed = cur_timestamp - state.pool_init_timestamp;
    if time_elapsed >= config.lp_tokens_vesting_duration {
        return user_info.lp_shares - user_info.claimed_lp_shares;
    }

    let withdrawable_lp_balance =
        user_info.lp_shares * Decimal::from_ratio(time_elapsed, config.lp_tokens_vesting_duration);
    withdrawable_lp_balance - user_info.claimed_lp_shares
}

//----------------------------------------------------------------------------------------
// Query functions
//----------------------------------------------------------------------------------------

/// @dev Returns the airdrop configuration
fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        owner: config.owner,
        astro_token_address: config.astro_token_address,
        airdrop_contract_address: config.airdrop_contract_address,
        lockdrop_contract_address: config.lockdrop_contract_address,
        astro_ust_pool_address: config.astro_ust_pool_address,
        astro_ust_lp_token_address: config.astro_ust_lp_token_address,
        generator_contract: config.generator_contract,
        astro_incentive_amount: config.astro_incentive_amount,
        lp_tokens_vesting_duration: config.lp_tokens_vesting_duration,
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
        total_ust_delegated: state.total_ust_delegated,
        is_lp_staked: state.is_lp_staked,
        lp_shares_minted: state.lp_shares_minted,
        pool_init_timestamp: state.pool_init_timestamp,
        generator_astro_per_share: state.generator_astro_per_share,
        generator_proxy_per_share: state.generator_proxy_per_share,
    })
}

/// @dev Returns details around user's ASTRO Airdrop claim
fn query_user_info(deps: Deps, env: Env, user_address: String) -> StdResult<UserInfoResponse> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;
    let user_address = deps.api.addr_validate(&user_address)?;
    let user_info = USERS
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    let withdrawable_lp_shares =
        calculate_withdrawable_lp_shares(env.block.time.seconds(), &config, &state, &user_info);

    // TODO: query pending generator rewards

    Ok(UserInfoResponse {
        astro_delegated: user_info.astro_delegated,
        ust_delegated: user_info.ust_delegated,
        ust_withdrawn: user_info.ust_withdrawn,
        lp_shares: user_info.lp_shares,
        claimed_lp_shares: user_info.claimed_lp_shares,
        withdrawable_lp_shares,
        auction_incentive_amount: user_info.auction_incentive_amount,
        astro_incentive_transfered: user_info.astro_incentive_transfered,
        generator_astro_debt: user_info.generator_astro_debt,
        generator_proxy_debt: user_info.generator_proxy_debt,
    })
}
