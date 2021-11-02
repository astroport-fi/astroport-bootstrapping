use std::ops::Add;

use astroport_periphery::lockdrop::{
    self, ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, PoolResponse, QueryMsg,
    StateResponse, UpdateConfigMsg, UserInfoResponse,
};
use cosmwasm_bignumber::Uint256;
use cosmwasm_std::testing::{mock_env, MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{
    attr, to_binary, Addr, Coin, Decimal, Timestamp, Uint128, Uint256 as CUint256, Uint64,
};
use cw20_base::msg::ExecuteMsg as CW20ExecuteMsg;
use terra_multi_test::{App, BankKeeper, ContractWrapper, Executor, TerraMockQuerier};

fn mock_app() -> App {
    let api = MockApi::default();
    let env = mock_env();
    let bank = BankKeeper::new();
    let storage = MockStorage::new();
    let tmq = TerraMockQuerier::new(MockQuerier::new(&[]));

    App::new(api, env.block, bank, storage, tmq)
}

// Instantiate ASTRO Token Contract
fn instantiate_astro_token(app: &mut App, owner: Addr) -> Addr {
    let astro_token_contract = Box::new(ContractWrapper::new(
        cw20_base::contract::execute,
        cw20_base::contract::instantiate,
        cw20_base::contract::query,
    ));

    let astro_token_code_id = app.store_code(astro_token_contract);

    let msg = cw20_base::msg::InstantiateMsg {
        name: String::from("Astro token"),
        symbol: String::from("ASTRO"),
        decimals: 6,
        initial_balances: vec![],
        mint: Some(cw20::MinterResponse {
            minter: owner.to_string(),
            cap: None,
        }),
        marketing: None,
    };

    let astro_token_instance = app
        .instantiate_contract(
            astro_token_code_id,
            owner.clone(),
            &msg,
            &[],
            String::from("ASTRO"),
            None,
        )
        .unwrap();
    astro_token_instance
}

// Instantiate Terraswap
fn instantiate_terraswap(app: &mut App, owner: Addr) -> Addr {
    // Terraswap Pair
    let terraswap_pair_contract = Box::new(ContractWrapper::new(
        terraswap_pair::contract::execute,
        terraswap_pair::contract::instantiate,
        terraswap_pair::contract::query,
    ));
    let terraswap_pair_code_id = app.store_code(terraswap_pair_contract);

    // Terraswap LP Token
    let terraswap_token_contract = Box::new(ContractWrapper::new(
        terraswap_token::contract::execute,
        terraswap_token::contract::instantiate,
        terraswap_token::contract::query,
    ));
    let terraswap_token_code_id = app.store_code(terraswap_token_contract);

    // Terraswap Factory Contract
    let terraswap_factory_contract = Box::new(ContractWrapper::new(
        terraswap_factory::contract::execute,
        terraswap_factory::contract::instantiate,
        terraswap_factory::contract::query,
    ));

    let terraswap_factory_code_id = app.store_code(terraswap_factory_contract);

    let msg = terraswap::factory::InstantiateMsg {
        pair_code_id: terraswap_pair_code_id,
        token_code_id: terraswap_token_code_id,
    };

    let terraswap_factory_instance = app
        .instantiate_contract(
            terraswap_factory_code_id,
            owner.clone(),
            &msg,
            &[],
            String::from("Terraswap_Factory"),
            None,
        )
        .unwrap();
    terraswap_factory_instance
}

// Instantiate Astroport
fn instantiate_astroport(app: &mut App, owner: Addr) -> Addr {
    let mut pair_configs = vec![];
    // Astroport Pair
    let astroport_pair_contract = Box::new(ContractWrapper::new(
        astroport_pair::contract::execute,
        astroport_pair::contract::instantiate,
        astroport_pair::contract::query,
    ));
    let astroport_pair_code_id = app.store_code(astroport_pair_contract);
    pair_configs.push(astroport::factory::PairConfig {
        code_id: astroport_pair_code_id,
        pair_type: astroport::factory::PairType::Xyk {},
        total_fee_bps: 5u16,
        maker_fee_bps: 3u16,
    });

    // Astroport Pair :: Stable
    let astroport_pair_stable_contract = Box::new(ContractWrapper::new(
        astroport_pair_stable::contract::execute,
        astroport_pair_stable::contract::instantiate,
        astroport_pair_stable::contract::query,
    ));
    let astroport_pair_stable_code_id = app.store_code(astroport_pair_stable_contract);
    pair_configs.push(astroport::factory::PairConfig {
        code_id: astroport_pair_stable_code_id,
        pair_type: astroport::factory::PairType::Stable {},
        total_fee_bps: 5u16,
        maker_fee_bps: 3u16,
    });

    // Astroport LP Token
    let astroport_token_contract = Box::new(ContractWrapper::new(
        astroport_token::contract::execute,
        astroport_token::contract::instantiate,
        astroport_token::contract::query,
    ));
    let astroport_token_code_id = app.store_code(astroport_token_contract);

    // Astroport Factory Contract
    let astroport_factory_contract = Box::new(ContractWrapper::new(
        astroport_factory::contract::execute,
        astroport_factory::contract::instantiate,
        astroport_factory::contract::query,
    ));

    let astroport_factory_code_id = app.store_code(astroport_factory_contract);

    let msg = astroport::factory::InstantiateMsg {
        /// Pair contract code IDs which are allowed to create pairs
        pair_configs: pair_configs,
        token_code_id: astroport_token_code_id,
        init_hook: None,
        fee_address: Some(Addr::unchecked("fee_address".to_string())),
        generator_address: Addr::unchecked("generator_address".to_string()),
        gov: Some(Addr::unchecked("gov".to_string())),
        owner: owner.clone().to_string(),
    };

    let astroport_factory_instance = app
        .instantiate_contract(
            astroport_factory_code_id,
            owner.clone(),
            &msg,
            &[],
            String::from("Astroport_Factory"),
            None,
        )
        .unwrap();
    astroport_factory_instance
}

// Instantiate Astroport's generator and vesting contracts
fn instantiate_generator_and_vesting(
    mut app: &mut App,
    owner: Addr,
    astro_token_instance: Addr,
) -> (Addr, Addr) {
    // Vesting
    let vesting_contract = Box::new(ContractWrapper::new(
        astroport_vesting::contract::execute,
        astroport_vesting::contract::instantiate,
        astroport_vesting::contract::query,
    ));
    let vesting_code_id = app.store_code(vesting_contract);

    let init_msg = astroport::vesting::InstantiateMsg {
        owner: owner.to_string(),
        token_addr: astro_token_instance.clone().to_string(),
    };

    let vesting_instance = app
        .instantiate_contract(
            vesting_code_id,
            owner.clone(),
            &init_msg,
            &[],
            "Vesting",
            None,
        )
        .unwrap();

    mint_some_astro(
        &mut app,
        owner.clone(),
        astro_token_instance.clone(),
        Uint128::new(900_000_000_000),
        owner.to_string(),
    );
    app.execute_contract(
        owner.clone(),
        astro_token_instance.clone(),
        &CW20ExecuteMsg::IncreaseAllowance {
            spender: vesting_instance.clone().to_string(),
            amount: Uint128::new(900_000_000_000),
            expires: None,
        },
        &[],
    )
    .unwrap();

    // Generator
    let generator_contract = Box::new(
        ContractWrapper::new(
            astroport_generator::contract::execute,
            astroport_generator::contract::instantiate,
            astroport_generator::contract::query,
        )
        .with_reply(astroport_generator::contract::reply),
    );

    let generator_code_id = app.store_code(generator_contract);

    let init_msg = astroport::generator::InstantiateMsg {
        allowed_reward_proxies: vec![],
        start_block: Uint64::from(app.block_info().height),
        astro_token: astro_token_instance.to_string(),
        tokens_per_block: Uint128::from(0u128),
        vesting_contract: vesting_instance.clone().to_string(),
    };

    let generator_instance = app
        .instantiate_contract(
            generator_code_id,
            owner.clone(),
            &init_msg,
            &[],
            "Guage",
            None,
        )
        .unwrap();

    let tokens_per_block = Uint128::new(10_000000);

    let msg = astroport::generator::ExecuteMsg::SetTokensPerBlock {
        amount: tokens_per_block,
    };
    app.execute_contract(owner.clone(), generator_instance.clone(), &msg, &[])
        .unwrap();

    let msg = astroport::generator::QueryMsg::Config {};
    let res: astroport::generator::ConfigResponse = app
        .wrap()
        .query_wasm_smart(&generator_instance, &msg)
        .unwrap();
    assert_eq!(res.tokens_per_block, tokens_per_block);

    // vesting to generator:

    let current_block = app.block_info();

    let amount = Uint128::new(630720000000);

    let msg = CW20ExecuteMsg::IncreaseAllowance {
        spender: vesting_instance.clone().to_string(),
        amount,
        expires: None,
    };

    app.execute_contract(owner.clone(), astro_token_instance.clone(), &msg, &[])
        .unwrap();

    let msg = astroport::vesting::ExecuteMsg::RegisterVestingAccounts {
        vesting_accounts: vec![astroport::vesting::VestingAccount {
            address: generator_instance.to_string(),
            schedules: vec![astroport::vesting::VestingSchedule {
                start_point: astroport::vesting::VestingSchedulePoint {
                    time: current_block.time,
                    amount,
                },
                end_point: None,
            }],
        }],
    };

    app.execute_contract(owner.clone(), vesting_instance.clone(), &msg, &[])
        .unwrap();

    // let msg = astroport::generator::ExecuteMsg::Add {
    //     alloc_point: Uint64::from(10u64),
    //     reward_proxy: None,
    //     lp_token: lp_token_instance.clone(),
    //     with_update: true,
    // };
    // app.execute_contract(
    //     Addr::unchecked(owner.clone()),
    //     generator_instance.clone(),
    //     &msg,
    //     &[],
    // )
    // .unwrap();

    (generator_instance, vesting_instance)
}

// Mints some ASTRO to "to" recepient
fn mint_some_astro(
    app: &mut App,
    owner: Addr,
    astro_token_instance: Addr,
    amount: Uint128,
    to: String,
) {
    let msg = cw20::Cw20ExecuteMsg::Mint {
        recipient: to.clone(),
        amount: amount,
    };
    let res = app
        .execute_contract(owner.clone(), astro_token_instance.clone(), &msg, &[])
        .unwrap();
    assert_eq!(res.events[1].attributes[1], attr("action", "mint"));
    assert_eq!(res.events[1].attributes[2], attr("to", to));
    assert_eq!(res.events[1].attributes[3], attr("amount", amount));
}

// Instantiate AUCTION Contract
fn instantiate_auction_contract(
    app: &mut App,
    owner: Addr,
    astro_token_instance: Addr,
    airdrop_instance: Addr,
    lockdrop_instance: Addr,
    pair_instance: Addr,
    lp_token_instance: Addr,
) -> (Addr, astroport_periphery::auction::InstantiateMsg) {
    let auction_contract = Box::new(ContractWrapper::new(
        astro_auction::contract::execute,
        astro_auction::contract::instantiate,
        astro_auction::contract::query,
    ));

    let auction_code_id = app.store_code(auction_contract);

    let auction_instantiate_msg = astroport_periphery::auction::InstantiateMsg {
        owner: owner.clone().to_string(),
        astro_token_address: astro_token_instance.clone().into_string(),
        airdrop_contract_address: airdrop_instance.to_string(),
        lockdrop_contract_address: lockdrop_instance.to_string(),
        astroport_lp_pool: Some(pair_instance.to_string()),
        lp_token_address: Some(lp_token_instance.to_string()),
        generator_contract: None,
        astro_rewards: Uint256::from(1000000000000u64),
        astro_vesting_duration: 7776000u64,
        lp_tokens_vesting_duration: 7776000u64,
        init_timestamp: 1_000_00,
        deposit_window: 100_000_00,
        withdrawal_window: 5_000_00,
    };

    // Init contract
    let auction_instance = app
        .instantiate_contract(
            auction_code_id,
            owner.clone(),
            &auction_instantiate_msg,
            &[],
            "auction",
            None,
        )
        .unwrap();
    (auction_instance, auction_instantiate_msg)
}

// Instantiate LOCKDROP Contract
fn instantiate_lockdrop_contract(app: &mut App, owner: Addr) -> (Addr, InstantiateMsg) {
    let lockdrop_contract = Box::new(ContractWrapper::new(
        astroport_lockdrop::contract::execute,
        astroport_lockdrop::contract::instantiate,
        astroport_lockdrop::contract::query,
    ));

    let lockdrop_code_id = app.store_code(lockdrop_contract);

    let lockdrop_instantiate_msg = InstantiateMsg {
        owner: Some(owner.clone().to_string()),
        init_timestamp: 1_000_00,
        deposit_window: 100_000_00,
        withdrawal_window: 5_000_00,
        min_lock_duration: 1u64,
        max_lock_duration: 52u64,
        weekly_multiplier: 1u64,
        weekly_divider: 12u64,
    };

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(900_00)
    });

    // Init contract
    let lockdrop_instance = app
        .instantiate_contract(
            lockdrop_code_id,
            owner.clone(),
            &lockdrop_instantiate_msg,
            &[],
            "lockdrop",
            None,
        )
        .unwrap();
    (lockdrop_instance, lockdrop_instantiate_msg)
}

// Instantiate
fn instantiate_all_contracts(
    mut app: &mut App,
    owner: Addr,
) -> (Addr, Addr, Addr, Addr, UpdateConfigMsg) {
    let (lockdrop_instance, lockdrop_instantiate_msg) =
        instantiate_lockdrop_contract(&mut app, owner.clone());

    let astro_token = instantiate_astro_token(&mut app, owner.clone());

    // Initiate Terraswap
    let terraswap_factory_instance = instantiate_terraswap(&mut app, owner.clone());

    // Initiate ASTRO-UST Pair on Astroport
    let astroport_factory_instance = instantiate_astroport(&mut app, owner.clone());
    let pair_info = [
        astroport::asset::AssetInfo::Token {
            contract_addr: astro_token.clone(),
        },
        astroport::asset::AssetInfo::NativeToken {
            denom: "uusd".to_string(),
        },
    ];
    app.execute_contract(
        Addr::unchecked("user"),
        astroport_factory_instance.clone(),
        &astroport::factory::ExecuteMsg::CreatePair {
            asset_infos: pair_info.clone(),
            init_hook: None,
            pair_type: astroport::factory::PairType::Xyk {},
        },
        &[],
    )
    .unwrap();
    let pair_resp: astroport::asset::PairInfo = app
        .wrap()
        .query_wasm_smart(
            &astroport_factory_instance,
            &astroport::factory::QueryMsg::Pair {
                asset_infos: pair_info.clone(),
            },
        )
        .unwrap();
    let pool_address = pair_resp.contract_addr;
    let lp_token_address = pair_resp.liquidity_token;

    // Initiate Auction contract
    let (auction_contract, _) = instantiate_auction_contract(
        &mut app,
        owner.clone(),
        astro_token.clone(),
        Addr::unchecked("auction_instance"),
        lockdrop_instance.clone(),
        pool_address,
        lp_token_address,
    );

    let (generator_address, _) =
        instantiate_generator_and_vesting(&mut app, owner.clone(), astro_token.clone());

    let update_msg = UpdateConfigMsg {
        owner: None,
        astro_token_address: Some(astro_token.to_string()),
        auction_contract_address: Some(auction_contract.to_string()),
        generator_address: Some(generator_address.to_string()),
        lockdrop_incentives: Some(Uint128::from(1000000000u64)),
    };
    app.execute_contract(
        owner.clone(),
        astro_token.clone(),
        &CW20ExecuteMsg::IncreaseAllowance {
            spender: lockdrop_instance.clone().to_string(),
            amount: Uint128::new(1000000000u128),
            expires: None,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        owner.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::UpdateConfig {
            new_config: update_msg.clone(),
        },
        &[],
    )
    .unwrap();

    return (
        astro_token,
        lockdrop_instance,
        astroport_factory_instance,
        terraswap_factory_instance,
        update_msg,
    );
}

#[test]
fn proper_initialization_lockdrop() {
    let mut app = mock_app();
    let owner = Addr::unchecked("contract_owner");

    let (lockdrop_instance, lockdrop_instantiate_msg) =
        instantiate_lockdrop_contract(&mut app, owner);

    let resp: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&lockdrop_instance, &QueryMsg::Config {})
        .unwrap();

    // Check config
    assert_eq!(
        lockdrop_instantiate_msg.owner.unwrap().to_string(),
        resp.owner
    );
    assert_eq!(None, resp.astro_token);
    assert_eq!(None, resp.auction_contract);
    assert_eq!(None, resp.generator);
    assert_eq!(lockdrop_instantiate_msg.init_timestamp, resp.init_timestamp);
    assert_eq!(lockdrop_instantiate_msg.deposit_window, resp.deposit_window);
    assert_eq!(
        lockdrop_instantiate_msg.withdrawal_window,
        resp.withdrawal_window
    );
    assert_eq!(
        lockdrop_instantiate_msg.min_lock_duration,
        resp.min_lock_duration
    );
    assert_eq!(
        lockdrop_instantiate_msg.max_lock_duration,
        resp.max_lock_duration
    );
    assert_eq!(
        lockdrop_instantiate_msg.weekly_multiplier,
        resp.weekly_multiplier
    );
    assert_eq!(lockdrop_instantiate_msg.weekly_divider, resp.weekly_divider);
    assert_eq!(None, resp.lockdrop_incentives);

    // Check state
    let resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&lockdrop_instance, &QueryMsg::State {})
        .unwrap();

    assert_eq!(0u64, resp.total_incentives_share);
    assert_eq!(Uint128::zero(), resp.total_astro_delegated);
    assert_eq!(Uint128::zero(), resp.total_astro_returned_available);
    assert_eq!(false, resp.are_claims_allowed);
}

#[test]
fn test_update_config() {
    let mut app = mock_app();
    let owner = Addr::unchecked("contract_owner");

    let (lockdrop_instance, lockdrop_instantiate_msg) =
        instantiate_lockdrop_contract(&mut app, owner.clone());

    let astro_token = instantiate_astro_token(&mut app, owner.clone());

    // Initiate ASTRO-UST Pair on Astroport
    let astroport_factory_instance = instantiate_astroport(&mut app, owner.clone());
    let pair_info = [
        astroport::asset::AssetInfo::Token {
            contract_addr: astro_token.clone(),
        },
        astroport::asset::AssetInfo::NativeToken {
            denom: "uusd".to_string(),
        },
    ];
    app.execute_contract(
        Addr::unchecked("user"),
        astroport_factory_instance.clone(),
        &astroport::factory::ExecuteMsg::CreatePair {
            asset_infos: pair_info.clone(),
            init_hook: None,
            pair_type: astroport::factory::PairType::Xyk {},
        },
        &[],
    )
    .unwrap();
    let pair_resp: astroport::asset::PairInfo = app
        .wrap()
        .query_wasm_smart(
            &astroport_factory_instance,
            &astroport::factory::QueryMsg::Pair {
                asset_infos: pair_info.clone(),
            },
        )
        .unwrap();
    let pool_address = pair_resp.contract_addr;
    let lp_token_address = pair_resp.liquidity_token;

    // Initiate Auction contract
    let (auction_contract, _) = instantiate_auction_contract(
        &mut app,
        owner.clone(),
        astro_token.clone(),
        Addr::unchecked("auction_instance"),
        lockdrop_instance.clone(),
        pool_address,
        lp_token_address,
    );

    let (generator_address, _) =
        instantiate_generator_and_vesting(&mut app, owner.clone(), astro_token.clone());

    let update_msg = UpdateConfigMsg {
        owner: Some("new_owner".to_string()),
        astro_token_address: Some(astro_token.to_string()),
        auction_contract_address: Some(auction_contract.to_string()),
        generator_address: Some(generator_address.to_string()),
        lockdrop_incentives: Some(Uint128::from(1000000000u64)),
    };

    // ######    ERROR :: Unauthorized     ######
    let err = app
        .execute_contract(
            Addr::unchecked("wrong_owner"),
            lockdrop_instance.clone(),
            &ExecuteMsg::UpdateConfig {
                new_config: update_msg.clone(),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Unauthorized");

    // ######    ERROR :: No allowance for this account     ######
    let err = app
        .execute_contract(
            owner.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::UpdateConfig {
                new_config: update_msg.clone(),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "No allowance for this account");

    // ######    SUCCESS :: Should have successfully updated   ######

    app.execute_contract(
        owner.clone(),
        astro_token.clone(),
        &CW20ExecuteMsg::IncreaseAllowance {
            spender: lockdrop_instance.clone().to_string(),
            amount: Uint128::new(1000000000u128),
            expires: None,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        owner.clone(),
        lockdrop_instance.clone(),
        &ExecuteMsg::UpdateConfig {
            new_config: update_msg.clone(),
        },
        &[],
    )
    .unwrap();
    let resp: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&lockdrop_instance, &QueryMsg::Config {})
        .unwrap();

    assert_eq!(update_msg.clone().owner.unwrap(), resp.owner);
    assert_eq!(
        update_msg.clone().astro_token_address.unwrap(),
        resp.astro_token.unwrap()
    );
    assert_eq!(
        update_msg.clone().auction_contract_address.unwrap(),
        resp.auction_contract.unwrap()
    );
    assert_eq!(
        update_msg.clone().generator_address.unwrap(),
        resp.generator.unwrap()
    );
    assert_eq!(
        update_msg.clone().lockdrop_incentives.unwrap(),
        resp.lockdrop_incentives.unwrap()
    );

    // ######    ERROR :: ASTRO is already being distributed     ######

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10600001)
    });

    let err = app
        .execute_contract(
            Addr::unchecked("new_owner".to_string()),
            lockdrop_instance.clone(),
            &ExecuteMsg::UpdateConfig {
                new_config: update_msg.clone(),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: ASTRO is already being distributed"
    );

    // ######    ERROR :: ASTRO tokens are live. Configuration cannot be updated now     ######
    app.execute_contract(
        Addr::unchecked(auction_contract),
        lockdrop_instance.clone(),
        &ExecuteMsg::EnableClaims {},
        &[],
    )
    .unwrap();

    let err = app
        .execute_contract(
            Addr::unchecked("new_owner".to_string()),
            lockdrop_instance.clone(),
            &ExecuteMsg::UpdateConfig {
                new_config: update_msg.clone(),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: ASTRO tokens are live. Configuration cannot be updated now"
    );
}

#[test]
fn test_initialize_pool() {
    let mut app = mock_app();
    let owner = Addr::unchecked("contract_owner");

    let (_, lockdrop_instance, _, _, update_msg) =
        instantiate_all_contracts(&mut app, owner.clone());

    // Terraswap LP Token
    let terraswap_token_contract = Box::new(ContractWrapper::new(
        terraswap_token::contract::execute,
        terraswap_token::contract::instantiate,
        terraswap_token::contract::query,
    ));
    let terraswap_token_code_id = app.store_code(terraswap_token_contract);

    let terraswap_token_instance = app
        .instantiate_contract(
            terraswap_token_code_id,
            Addr::unchecked("user".to_string()),
            &terraswap::token::InstantiateMsg {
                name: "terraswap liquidity token".to_string(),
                symbol: "uLP".to_string(),
                decimals: 6,
                initial_balances: vec![],
                mint: Some(cw20::MinterResponse {
                    minter: "pair_instance".to_string(),
                    cap: None,
                }),
            },
            &[],
            String::from("terraswap_lp_token"),
            None,
        )
        .unwrap();

    let initialize_pool_msg = astroport_periphery::lockdrop::ExecuteMsg::InitializePool {
        terraswap_lp_token: terraswap_token_instance.to_string(),
        incentives_share: 10000000u64,
    };

    // ######    ERROR :: Unauthorized     ######
    let err = app
        .execute_contract(
            Addr::unchecked("wrong_owner"),
            lockdrop_instance.clone(),
            &initialize_pool_msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Unauthorized");

    // ######    SUCCESS :: SHOULD SUCCESSFULLY INITIALIZE     ######
    app.execute_contract(
        owner.clone(),
        lockdrop_instance.clone(),
        &initialize_pool_msg,
        &[],
    )
    .unwrap();
    // check state
    let state_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&lockdrop_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(10000000u64, state_resp.total_incentives_share);
    assert_eq!(Uint128::zero(), state_resp.total_astro_delegated);
    assert_eq!(Uint128::zero(), state_resp.total_astro_returned_available);
    assert_eq!(false, state_resp.are_claims_allowed);
    assert_eq!(
        vec![terraswap_token_instance.clone()],
        state_resp.supported_pairs_list
    );
    // check Pool Info
    let pool_resp: PoolResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::Pool {
                terraswap_lp_token: terraswap_token_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!("pair_instance".to_string(), pool_resp.terraswap_pool);
    assert_eq!(Uint128::zero(), pool_resp.terraswap_amount_in_lockups);
    assert_eq!(None, pool_resp.migration_info);
    assert_eq!(10000000u64, pool_resp.incentives_share);
    assert_eq!(CUint256::zero(), pool_resp.weighted_amount);
    assert_eq!(Decimal::zero(), pool_resp.generator_astro_per_share);
    assert_eq!(Decimal::zero(), pool_resp.generator_proxy_per_share);
    assert_eq!(false, pool_resp.is_staked);

    // ######    ERROR :: Already supported     ######
    let err = app
        .execute_contract(
            owner.clone(),
            lockdrop_instance.clone(),
            &initialize_pool_msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Already supported");

    // ######    SUCCESS :: SHOULD SUCCESSFULLY INITIALIZE #2    ######

    let terraswap_token_instance2 = app
        .instantiate_contract(
            terraswap_token_code_id,
            Addr::unchecked("user".to_string()),
            &terraswap::token::InstantiateMsg {
                name: "terraswap liquidity token #2".to_string(),
                symbol: "uLP".to_string(),
                decimals: 6,
                initial_balances: vec![],
                mint: Some(cw20::MinterResponse {
                    minter: "pair_instance#2".to_string(),
                    cap: None,
                }),
            },
            &[],
            String::from("terraswap_lp_token#2"),
            None,
        )
        .unwrap();

    app.execute_contract(
        owner.clone(),
        lockdrop_instance.clone(),
        &astroport_periphery::lockdrop::ExecuteMsg::InitializePool {
            terraswap_lp_token: terraswap_token_instance2.to_string(),
            incentives_share: 10400000u64,
        },
        &[],
    )
    .unwrap();
    // check state
    let state_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&lockdrop_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(20400000u64, state_resp.total_incentives_share);
    assert_eq!(
        vec![
            terraswap_token_instance.clone(),
            terraswap_token_instance2.clone()
        ],
        state_resp.supported_pairs_list
    );
    // check Pool Info
    let pool_resp: PoolResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::Pool {
                terraswap_lp_token: terraswap_token_instance2.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!("pair_instance#2".to_string(), pool_resp.terraswap_pool);
    assert_eq!(Uint128::zero(), pool_resp.terraswap_amount_in_lockups);
    assert_eq!(None, pool_resp.migration_info);
    assert_eq!(10400000u64, pool_resp.incentives_share);

    // ######    ERROR :: Pools cannot be added post deposit window closure     ######
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(900000_00)
    });
    let err = app
        .execute_contract(
            owner.clone(),
            lockdrop_instance.clone(),
            &initialize_pool_msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Pools cannot be added post deposit window closure"
    );
}

#[test]
fn test_update_pool() {
    let mut app = mock_app();
    let owner = Addr::unchecked("contract_owner");

    let (_, lockdrop_instance, _, _, update_msg) =
        instantiate_all_contracts(&mut app, owner.clone());

    // Terraswap LP Token
    let terraswap_token_contract = Box::new(ContractWrapper::new(
        terraswap_token::contract::execute,
        terraswap_token::contract::instantiate,
        terraswap_token::contract::query,
    ));
    let terraswap_token_code_id = app.store_code(terraswap_token_contract);

    let terraswap_token_instance = app
        .instantiate_contract(
            terraswap_token_code_id,
            Addr::unchecked("user".to_string()),
            &terraswap::token::InstantiateMsg {
                name: "terraswap liquidity token".to_string(),
                symbol: "uLP".to_string(),
                decimals: 6,
                initial_balances: vec![],
                mint: Some(cw20::MinterResponse {
                    minter: "pair_instance".to_string(),
                    cap: None,
                }),
            },
            &[],
            String::from("terraswap_lp_token"),
            None,
        )
        .unwrap();

    // SUCCESSFULLY INITIALIZES POOL
    app.execute_contract(
        owner.clone(),
        lockdrop_instance.clone(),
        &astroport_periphery::lockdrop::ExecuteMsg::InitializePool {
            terraswap_lp_token: terraswap_token_instance.to_string(),
            incentives_share: 10000000u64,
        },
        &[],
    )
    .unwrap();

    // ######    ERROR :: Unauthorized     ######
    let err = app
        .execute_contract(
            Addr::unchecked("wrong_owner"),
            lockdrop_instance.clone(),
            &astroport_periphery::lockdrop::ExecuteMsg::UpdatePool {
                terraswap_lp_token: terraswap_token_instance.to_string(),
                incentives_share: 3434543u64,
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Unauthorized");

    // ######    SUCCESS :: SHOULD SUCCESSFULLY UPDATE POOL     ######
    app.execute_contract(
        owner.clone(),
        lockdrop_instance.clone(),
        &astroport_periphery::lockdrop::ExecuteMsg::UpdatePool {
            terraswap_lp_token: terraswap_token_instance.to_string(),
            incentives_share: 3434543u64,
        },
        &[],
    )
    .unwrap();
    // check state
    let state_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&lockdrop_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(3434543u64, state_resp.total_incentives_share);
    assert_eq!(Uint128::zero(), state_resp.total_astro_delegated);
    assert_eq!(Uint128::zero(), state_resp.total_astro_returned_available);
    assert_eq!(false, state_resp.are_claims_allowed);
    assert_eq!(
        vec![terraswap_token_instance.clone()],
        state_resp.supported_pairs_list
    );
    // check Pool Info
    let pool_resp: PoolResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::Pool {
                terraswap_lp_token: terraswap_token_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!("pair_instance".to_string(), pool_resp.terraswap_pool);
    assert_eq!(Uint128::zero(), pool_resp.terraswap_amount_in_lockups);
    assert_eq!(None, pool_resp.migration_info);
    assert_eq!(3434543u64, pool_resp.incentives_share);
    assert_eq!(CUint256::zero(), pool_resp.weighted_amount);
    assert_eq!(Decimal::zero(), pool_resp.generator_astro_per_share);
    assert_eq!(Decimal::zero(), pool_resp.generator_proxy_per_share);
    assert_eq!(false, pool_resp.is_staked);

    // ######    ERROR :: Pools cannot be added post deposit window closure     ######
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(900000_00)
    });
    let err = app
        .execute_contract(
            owner.clone(),
            lockdrop_instance.clone(),
            &astroport_periphery::lockdrop::ExecuteMsg::UpdatePool {
                terraswap_lp_token: terraswap_token_instance.to_string(),
                incentives_share: 3434543u64,
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Pools cannot be updated post deposit window closure"
    );
}

#[test]
fn test_increase_lockup() {
    let mut app = mock_app();
    let owner = Addr::unchecked("contract_owner");

    let (_, lockdrop_instance, _, _, update_msg) =
        instantiate_all_contracts(&mut app, owner.clone());

    // Terraswap LP Token
    let terraswap_token_contract = Box::new(ContractWrapper::new(
        terraswap_token::contract::execute,
        terraswap_token::contract::instantiate,
        terraswap_token::contract::query,
    ));
    let terraswap_token_code_id = app.store_code(terraswap_token_contract);

    // LP Token #1
    let terraswap_token_instance = app
        .instantiate_contract(
            terraswap_token_code_id,
            Addr::unchecked("user".to_string()),
            &terraswap::token::InstantiateMsg {
                name: "terraswap liquidity token".to_string(),
                symbol: "uLP".to_string(),
                decimals: 6,
                initial_balances: vec![],
                mint: Some(cw20::MinterResponse {
                    minter: "pair_instance".to_string(),
                    cap: None,
                }),
            },
            &[],
            String::from("terraswap_lp_token"),
            None,
        )
        .unwrap();

    // LP Token #2
    let terraswap_token_instance2 = app
        .instantiate_contract(
            terraswap_token_code_id,
            Addr::unchecked("user".to_string()),
            &terraswap::token::InstantiateMsg {
                name: "terraswap liquidity token".to_string(),
                symbol: "uLP".to_string(),
                decimals: 6,
                initial_balances: vec![],
                mint: Some(cw20::MinterResponse {
                    minter: "pair2_instance".to_string(),
                    cap: None,
                }),
            },
            &[],
            String::from("terraswap_lp_token2"),
            None,
        )
        .unwrap();

    // SUCCESSFULLY INITIALIZES POOL
    app.execute_contract(
        owner.clone(),
        lockdrop_instance.clone(),
        &astroport_periphery::lockdrop::ExecuteMsg::InitializePool {
            terraswap_lp_token: terraswap_token_instance.to_string(),
            incentives_share: 10000000u64,
        },
        &[],
    )
    .unwrap();

    let user_address = "user".to_string();
    let user2_address = "user2".to_string();

    // Mint some LP tokens to user#1
    app.execute_contract(
        Addr::unchecked("pair_instance".to_string()),
        terraswap_token_instance.clone(),
        &cw20::Cw20ExecuteMsg::Mint {
            recipient: user_address.clone(),
            amount: Uint128::from(124231343u128),
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        Addr::unchecked("pair2_instance".to_string()),
        terraswap_token_instance2.clone(),
        &cw20::Cw20ExecuteMsg::Mint {
            recipient: user_address.clone(),
            amount: Uint128::from(100000000u128),
        },
        &[],
    )
    .unwrap();

    // Mint some LP tokens to user#2
    app.execute_contract(
        Addr::unchecked("pair_instance".to_string()),
        terraswap_token_instance.clone(),
        &cw20::Cw20ExecuteMsg::Mint {
            recipient: user2_address.clone(),
            amount: Uint128::from(124231343u128),
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        Addr::unchecked("pair2_instance".to_string()),
        terraswap_token_instance2.clone(),
        &cw20::Cw20ExecuteMsg::Mint {
            recipient: user2_address.clone(),
            amount: Uint128::from(100000000u128),
        },
        &[],
    )
    .unwrap();

    // ######    ERROR :: LP Pool not supported    ######
    let err = app
        .execute_contract(
            Addr::unchecked(user_address.clone()),
            terraswap_token_instance2.clone(),
            &cw20::Cw20ExecuteMsg::Send {
                contract: lockdrop_instance.clone().to_string(),
                amount: Uint128::from(10000u128),
                msg: to_binary(&lockdrop::Cw20HookMsg::IncreaseLockup { duration: 4u64 }).unwrap(),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "astroport_lockdrop::state::PoolInfo not found"
    );

    // ######    ERROR :: Deposit window closed (havent opened)   ######

    let err = app
        .execute_contract(
            Addr::unchecked(user_address.clone()),
            terraswap_token_instance.clone(),
            &cw20::Cw20ExecuteMsg::Send {
                contract: lockdrop_instance.clone().to_string(),
                amount: Uint128::from(10000u128),
                msg: to_binary(&lockdrop::Cw20HookMsg::IncreaseLockup { duration: 5u64 }).unwrap(),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Deposit window closed");

    // ######    ERROR :: Lockup duration needs to be between 1 and 52   ######

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_00)
    });

    let err = app
        .execute_contract(
            Addr::unchecked(user_address.clone()),
            terraswap_token_instance.clone(),
            &cw20::Cw20ExecuteMsg::Send {
                contract: lockdrop_instance.clone().to_string(),
                amount: Uint128::from(10000u128),
                msg: to_binary(&lockdrop::Cw20HookMsg::IncreaseLockup { duration: 0u64 }).unwrap(),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Lockup duration needs to be between 1 and 52"
    );

    let err = app
        .execute_contract(
            Addr::unchecked(user_address.clone()),
            terraswap_token_instance.clone(),
            &cw20::Cw20ExecuteMsg::Send {
                contract: lockdrop_instance.clone().to_string(),
                amount: Uint128::from(10000u128),
                msg: to_binary(&lockdrop::Cw20HookMsg::IncreaseLockup { duration: 53u64 }).unwrap(),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Lockup duration needs to be between 1 and 52"
    );

    // ######    SUCCESS :: SHOULD SUCCESSFULLY DEPOSIT LP TOKENS INTO POOL     ######
    app.execute_contract(
        Addr::unchecked(user_address.clone()),
        terraswap_token_instance.clone(),
        &cw20::Cw20ExecuteMsg::Send {
            contract: lockdrop_instance.clone().to_string(),
            amount: Uint128::from(10000u128),
            msg: to_binary(&lockdrop::Cw20HookMsg::IncreaseLockup { duration: 5u64 }).unwrap(),
        },
        &[],
    )
    .unwrap();

    // check Pool Info
    let pool_resp: PoolResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::Pool {
                terraswap_lp_token: terraswap_token_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(10000u128),
        pool_resp.terraswap_amount_in_lockups
    );
    assert_eq!(CUint256::from(13333u64), pool_resp.weighted_amount);
    assert_eq!(10000000u64, pool_resp.incentives_share);

    // check User Info
    let user_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::UserInfo {
                address: user_address.clone(),
            },
        )
        .unwrap();
    assert_eq!(
        update_msg.lockdrop_incentives.unwrap(),
        user_resp.total_astro_rewards
    );
    assert_eq!(Uint128::zero(), user_resp.delegated_astro_rewards);
    assert_eq!(
        Uint128::from(10000u128),
        user_resp.lockup_infos[0].lp_units_locked
    );
    assert_eq!(false, user_resp.lockup_infos[0].withdrawal_flag);
    assert_eq!(
        user_resp.total_astro_rewards,
        user_resp.lockup_infos[0].astro_rewards
    );
    assert_eq!(false, user_resp.lockup_infos[0].astro_transferred);
    assert_eq!(5u64, user_resp.lockup_infos[0].duration);
    assert_eq!(
        Uint128::zero(),
        user_resp.lockup_infos[0].generator_astro_debt
    );
    assert_eq!(
        Uint128::zero(),
        user_resp.lockup_infos[0].generator_proxy_debt
    );
    assert_eq!(13624000u64, user_resp.lockup_infos[0].unlock_timestamp);
    assert_eq!(None, user_resp.lockup_infos[0].astroport_lp_units);
    assert_eq!(None, user_resp.lockup_infos[0].astroport_lp_token);

    // ######    SUCCESS :: SHOULD SUCCESSFULLY DEPOSIT LP TOKENS INTO POOL (2nd USER)     ######
    app.execute_contract(
        Addr::unchecked(user2_address.clone()),
        terraswap_token_instance.clone(),
        &cw20::Cw20ExecuteMsg::Send {
            contract: lockdrop_instance.clone().to_string(),
            amount: Uint128::from(10000u128),
            msg: to_binary(&lockdrop::Cw20HookMsg::IncreaseLockup { duration: 10u64 }).unwrap(),
        },
        &[],
    )
    .unwrap();

    // check Pool Info
    let pool_resp: PoolResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::Pool {
                terraswap_lp_token: terraswap_token_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(20000u128),
        pool_resp.terraswap_amount_in_lockups
    );
    assert_eq!(CUint256::from(30833u64), pool_resp.weighted_amount);

    // check User Info
    let user_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::UserInfo {
                address: user2_address.clone(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(567573703u64), user_resp.total_astro_rewards);
    assert_eq!(
        Uint128::from(10000u128),
        user_resp.lockup_infos[0].lp_units_locked
    );
    assert_eq!(
        user_resp.total_astro_rewards,
        user_resp.lockup_infos[0].astro_rewards
    );
    assert_eq!(16648000u64, user_resp.lockup_infos[0].unlock_timestamp);

    // check User#1 Info (ASTRO rewards should be the latest one)
    let user_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::UserInfo {
                address: user_address.clone(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(432426296u128), user_resp.total_astro_rewards);
    assert_eq!(
        Uint128::from(10000u128),
        user_resp.lockup_infos[0].lp_units_locked
    );

    // ######    SUCCESS :: SHOULD SUCCESSFULLY AGAIN DEPOSIT LP TOKENS INTO POOL     ######
    app.execute_contract(
        Addr::unchecked(user_address.clone()),
        terraswap_token_instance.clone(),
        &cw20::Cw20ExecuteMsg::Send {
            contract: lockdrop_instance.clone().to_string(),
            amount: Uint128::from(10u128),
            msg: to_binary(&lockdrop::Cw20HookMsg::IncreaseLockup { duration: 51u64 }).unwrap(),
        },
        &[],
    )
    .unwrap();

    // check Pool Info
    let pool_resp: PoolResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::Pool {
                terraswap_lp_token: terraswap_token_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(20010u128),
        pool_resp.terraswap_amount_in_lockups
    );

    // check User Info
    let user_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::UserInfo {
                address: user_address.clone(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(10u128),
        user_resp.lockup_infos[1].lp_units_locked
    );
    assert_eq!(51u64, user_resp.lockup_infos[1].duration);
    assert_eq!(Uint128::from(433363553u128), user_resp.total_astro_rewards);

    // ######    ERROR :: Deposit window closed   ######
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(900_000000)
    });

    let err = app
        .execute_contract(
            Addr::unchecked(user_address.clone()),
            terraswap_token_instance.clone(),
            &cw20::Cw20ExecuteMsg::Send {
                contract: lockdrop_instance.clone().to_string(),
                amount: Uint128::from(100u128),
                msg: to_binary(&lockdrop::Cw20HookMsg::IncreaseLockup { duration: 5u64 }).unwrap(),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Deposit window closed");
}

#[test]
fn test_withdraw_from_lockup() {
    let mut app = mock_app();
    let owner = Addr::unchecked("contract_owner");

    let (_, lockdrop_instance, _, _, update_msg) =
        instantiate_all_contracts(&mut app, owner.clone());

    // Terraswap LP Token
    let terraswap_token_contract = Box::new(ContractWrapper::new(
        terraswap_token::contract::execute,
        terraswap_token::contract::instantiate,
        terraswap_token::contract::query,
    ));
    let terraswap_token_code_id = app.store_code(terraswap_token_contract);

    // LP Token #1
    let terraswap_token_instance = app
        .instantiate_contract(
            terraswap_token_code_id,
            Addr::unchecked("user".to_string()),
            &terraswap::token::InstantiateMsg {
                name: "terraswap liquidity token".to_string(),
                symbol: "uLP".to_string(),
                decimals: 6,
                initial_balances: vec![],
                mint: Some(cw20::MinterResponse {
                    minter: "pair_instance".to_string(),
                    cap: None,
                }),
            },
            &[],
            String::from("terraswap_lp_token"),
            None,
        )
        .unwrap();

    // SUCCESSFULLY INITIALIZES POOL
    app.execute_contract(
        owner.clone(),
        lockdrop_instance.clone(),
        &astroport_periphery::lockdrop::ExecuteMsg::InitializePool {
            terraswap_lp_token: terraswap_token_instance.to_string(),
            incentives_share: 10000000u64,
        },
        &[],
    )
    .unwrap();

    let user_address = "user".to_string();
    let user2_address = "user2".to_string();

    // Mint some LP tokens to user#1
    app.execute_contract(
        Addr::unchecked("pair_instance".to_string()),
        terraswap_token_instance.clone(),
        &cw20::Cw20ExecuteMsg::Mint {
            recipient: user_address.clone(),
            amount: Uint128::from(124231343u128),
        },
        &[],
    )
    .unwrap();

    // Deposit into Lockup Position

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_00)
    });

    app.execute_contract(
        Addr::unchecked(user_address.clone()),
        terraswap_token_instance.clone(),
        &cw20::Cw20ExecuteMsg::Send {
            contract: lockdrop_instance.clone().to_string(),
            amount: Uint128::from(10000000u128),
            msg: to_binary(&lockdrop::Cw20HookMsg::IncreaseLockup { duration: 10u64 }).unwrap(),
        },
        &[],
    )
    .unwrap();

    // ######    ERROR :: Invalid withdrawal request   ######

    let err = app
        .execute_contract(
            Addr::unchecked(user_address.clone()),
            lockdrop_instance.clone(),
            &ExecuteMsg::WithdrawFromLockup {
                terraswap_lp_token: terraswap_token_instance.clone().to_string(),
                amount: Uint128::from(0u128),
                duration: 1u64,
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Invalid withdrawal request");

    // ######    ERROR :: LP Token not supported   ######

    let err = app
        .execute_contract(
            Addr::unchecked(user_address.clone()),
            lockdrop_instance.clone(),
            &ExecuteMsg::WithdrawFromLockup {
                terraswap_lp_token: "wrong_terraswap_token_instance".to_string(),
                amount: Uint128::from(10u128),
                duration: 1u64,
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "astroport_lockdrop::state::PoolInfo not found"
    );

    // ######    ERROR :: Invalid lockup position   ######

    let err = app
        .execute_contract(
            Addr::unchecked(user_address.clone()),
            lockdrop_instance.clone(),
            &ExecuteMsg::WithdrawFromLockup {
                terraswap_lp_token: terraswap_token_instance.clone().to_string(),
                amount: Uint128::from(10u128),
                duration: 1u64,
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "astroport_lockdrop::state::LockupInfo not found"
    );

    // ######    SUCCESS :: SHOULD SUCCESSFULLY WITHDRAW LP TOKENS FROM POOL     ######
    app.execute_contract(
        Addr::unchecked(user_address.clone()),
        lockdrop_instance.clone(),
        &ExecuteMsg::WithdrawFromLockup {
            terraswap_lp_token: terraswap_token_instance.clone().to_string(),
            amount: Uint128::from(10000000u128),
            duration: 10u64,
        },
        &[],
    )
    .unwrap();

    // check Pool Info
    let pool_resp: PoolResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::Pool {
                terraswap_lp_token: terraswap_token_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(0u128), pool_resp.terraswap_amount_in_lockups);
    assert_eq!(CUint256::from(0u64), pool_resp.weighted_amount);

    // check User Info
    let user_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::UserInfo {
                address: user_address.clone(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(0u128), user_resp.total_astro_rewards);
    assert_eq!(Uint128::zero(), user_resp.delegated_astro_rewards);
    assert_eq!(0, user_resp.lockup_infos.len());

    // Deposit Again into Lockup
    app.execute_contract(
        Addr::unchecked(user_address.clone()),
        terraswap_token_instance.clone(),
        &cw20::Cw20ExecuteMsg::Send {
            contract: lockdrop_instance.clone().to_string(),
            amount: Uint128::from(10000000u128),
            msg: to_binary(&lockdrop::Cw20HookMsg::IncreaseLockup { duration: 10u64 }).unwrap(),
        },
        &[],
    )
    .unwrap();

    // ######    ERROR :: Amount exceeds maximum allowed withdrawal limit of {}    ######
    // First half of withdrawal window
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10350000)
    });

    let err = app
        .execute_contract(
            Addr::unchecked(user_address.clone()),
            lockdrop_instance.clone(),
            &ExecuteMsg::WithdrawFromLockup {
                terraswap_lp_token: terraswap_token_instance.clone().to_string(),
                amount: Uint128::from(5000001u128),
                duration: 10u64,
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Amount exceeds maximum allowed withdrawal limit of 5000000"
    );

    // 2nd half of withdrawal window
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10390000)
    });

    let err = app
        .execute_contract(
            Addr::unchecked(user_address.clone()),
            lockdrop_instance.clone(),
            &ExecuteMsg::WithdrawFromLockup {
                terraswap_lp_token: terraswap_token_instance.clone().to_string(),
                amount: Uint128::from(5000001u128),
                duration: 10u64,
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Amount exceeds maximum allowed withdrawal limit of 4200000"
    );

    // ######    SUCCESS :: SHOULD SUCCESSFULLY WITHDRAW LP TOKENS FROM POOL     ######
    app.execute_contract(
        Addr::unchecked(user_address.clone()),
        lockdrop_instance.clone(),
        &ExecuteMsg::WithdrawFromLockup {
            terraswap_lp_token: terraswap_token_instance.clone().to_string(),
            amount: Uint128::from(4200000u128),
            duration: 10u64,
        },
        &[],
    )
    .unwrap();

    // check Pool Info
    let pool_resp: PoolResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::Pool {
                terraswap_lp_token: terraswap_token_instance.clone().to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(5800000u128),
        pool_resp.terraswap_amount_in_lockups
    );
    assert_eq!(CUint256::from(10150000u64), pool_resp.weighted_amount);

    // check User Info
    let user_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &QueryMsg::UserInfo {
                address: user_address.clone(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(1000000000u128), user_resp.total_astro_rewards);
    assert_eq!(1, user_resp.lockup_infos.len());
    assert_eq!(1, user_resp.lockup_infos.len());
    assert_eq!(
        Uint128::from(5800000u128),
        user_resp.lockup_infos[0].lp_units_locked
    );
    assert_eq!(true, user_resp.lockup_infos[0].withdrawal_flag);

    // ######    ERROR :: Amount exceeds maximum allowed withdrawal limit of {}    ######

    let err = app
        .execute_contract(
            Addr::unchecked(user_address.clone()),
            lockdrop_instance.clone(),
            &ExecuteMsg::WithdrawFromLockup {
                terraswap_lp_token: terraswap_token_instance.clone().to_string(),
                amount: Uint128::from(1u128),
                duration: 10u64,
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Withdrawal already happened. No more withdrawals accepted"
    );
}

#[test]
fn test_migrate_liquidity() {
    let mut app = mock_app();
    let owner = Addr::unchecked("contract_owner");

    let (_, lockdrop_instance, _, _, update_msg) =
        instantiate_all_contracts(&mut app, owner.clone());
}
