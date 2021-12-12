use astroport_periphery::auction::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, StateResponse,
    UpdateConfigMsg, UserInfoResponse,
};
use cosmwasm_std::testing::{mock_env, MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{attr, to_binary, Addr, Coin, Timestamp, Uint128, Uint64};
use cw20::Cw20ExecuteMsg;
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
        astroport_token::contract::execute,
        astroport_token::contract::instantiate,
        astroport_token::contract::query,
    ));

    let astro_token_code_id = app.store_code(astro_token_contract);

    let msg = astroport::token::InstantiateMsg {
        name: String::from("Astro token"),
        symbol: String::from("ASTRO"),
        decimals: 6,
        initial_balances: vec![],
        mint: Some(cw20::MinterResponse {
            minter: owner.to_string(),
            cap: None,
        }),
    };

    app.instantiate_contract(
        astro_token_code_id,
        owner,
        &msg,
        &[],
        String::from("ASTRO"),
        None,
    )
    .unwrap()
}

// Instantiate AUCTION Contract
fn instantiate_auction_contract(
    app: &mut App,
    owner: Addr,
    astro_token_instance: Addr,
    airdrop_instance: Addr,
    lockdrop_instance: Addr,
    pair_instance: Addr,
) -> (Addr, InstantiateMsg) {
    let auction_contract = Box::new(ContractWrapper::new(
        astroport_auction::contract::execute,
        astroport_auction::contract::instantiate,
        astroport_auction::contract::query,
    ));

    let auction_code_id = app.store_code(auction_contract);

    let auction_instantiate_msg = astroport_periphery::auction::InstantiateMsg {
        owner: Some(owner.to_string()),
        astro_token_address: astro_token_instance.clone().into_string(),
        airdrop_contract_address: airdrop_instance.to_string(),
        lockdrop_contract_address: lockdrop_instance.to_string(),
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

    app.execute_contract(
        owner.clone(),
        auction_instance.clone(),
        &ExecuteMsg::UpdateConfig {
            new_config: UpdateConfigMsg {
                astro_ust_pair_address: Some(pair_instance.to_string()),
                owner: None,
                generator_contract: None,
            },
        },
        &[],
    )
    .unwrap();
    (auction_instance, auction_instantiate_msg)
}

fn init_auction_astro_contracts(app: &mut App) -> (Addr, Addr, Addr, Addr, InstantiateMsg) {
    let owner = Addr::unchecked("contract_owner");
    let astro_token_instance = instantiate_astro_token(app, owner.clone());

    // Instantiate LP Pair &  Airdrop / Lockdrop Contracts
    let (pair_instance, _) = instantiate_pair(app, owner.clone(), astro_token_instance.clone());
    let (airdrop_instance, lockdrop_instance) =
        instantiate_airdrop_lockdrop_contracts(app, owner.clone(), astro_token_instance.clone());

    // Instantiate Auction Contract
    let (auction_instance, auction_instantiate_msg) = instantiate_auction_contract(
        app,
        owner,
        astro_token_instance.clone(),
        airdrop_instance.clone(),
        lockdrop_instance.clone(),
        pair_instance,
    );

    (
        airdrop_instance,
        lockdrop_instance,
        auction_instance,
        astro_token_instance,
        auction_instantiate_msg,
    )
}

// Initiates Auction, Astro token, Airdrop, Lockdrop and Astroport Pair contracts
fn init_all_contracts(app: &mut App) -> (Addr, Addr, Addr, Addr, Addr, Addr, InstantiateMsg) {
    let owner = Addr::unchecked("contract_owner");
    let astro_token_instance = instantiate_astro_token(app, owner.clone());

    // Instantiate LP Pair &  Airdrop / Lockdrop Contracts
    let (pair_instance, lp_token_instance) =
        instantiate_pair(app, owner.clone(), astro_token_instance.clone());
    let (airdrop_instance, lockdrop_instance) =
        instantiate_airdrop_lockdrop_contracts(app, owner.clone(), astro_token_instance.clone());

    // Instantiate Auction Contract
    let (auction_instance, auction_instantiate_msg) = instantiate_auction_contract(
        app,
        owner.clone(),
        astro_token_instance.clone(),
        airdrop_instance.clone(),
        lockdrop_instance.clone(),
        pair_instance.clone(),
    );

    // Update Airdrop / Lockdrop Configs
    app.execute_contract(
        owner.clone(),
        airdrop_instance.clone(),
        &astroport_periphery::airdrop::ExecuteMsg::UpdateConfig {
            owner: None,
            auction_contract_address: Some(auction_instance.to_string()),
            merkle_roots: None,
            from_timestamp: None,
            to_timestamp: None,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        owner,
        lockdrop_instance.clone(),
        &astroport_periphery::lockdrop::ExecuteMsg::UpdateConfig {
            new_config: astroport_periphery::lockdrop::UpdateConfigMsg {
                owner: None,
                auction_contract_address: Some(auction_instance.to_string()),
                generator_address: None,
                astro_token_address: None,
            },
        },
        &[],
    )
    .unwrap();

    (
        auction_instance,
        astro_token_instance,
        airdrop_instance,
        lockdrop_instance,
        pair_instance,
        lp_token_instance,
        auction_instantiate_msg,
    )
}

// Initiates Astroport Pair for ASTRO-UST Pool
fn instantiate_pair(app: &mut App, owner: Addr, astro_token_instance: Addr) -> (Addr, Addr) {
    let lp_token_contract = Box::new(ContractWrapper::new(
        astroport_token::contract::execute,
        astroport_token::contract::instantiate,
        astroport_token::contract::query,
    ));

    let pair_contract = Box::new(
        ContractWrapper::new(
            astroport_pair::contract::execute,
            astroport_pair::contract::instantiate,
            astroport_pair::contract::query,
        )
        .with_reply(astroport_pair::contract::reply),
    );

    let lp_token_code_id = app.store_code(lp_token_contract);
    let pair_code_id = app.store_code(pair_contract);

    let msg = astroport::pair::InstantiateMsg {
        asset_infos: [
            astroport::asset::AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
            astroport::asset::AssetInfo::Token {
                contract_addr: astro_token_instance,
            },
        ],
        token_code_id: lp_token_code_id,
        init_params: None,
        factory_addr: Addr::unchecked("factory"),
    };

    let pair_instance = app
        .instantiate_contract(
            pair_code_id,
            owner.clone(),
            &msg,
            &[],
            String::from("PAIR"),
            None,
        )
        .unwrap();

    let resp: astroport::asset::PairInfo = app
        .wrap()
        .query_wasm_smart(&pair_instance, &astroport::pair::QueryMsg::Pair {})
        .unwrap();
    let lp_token_instance = resp.liquidity_token;

    (pair_instance, lp_token_instance)
}

// Initiates Airdrop and lockdrop contracts
fn instantiate_airdrop_lockdrop_contracts(
    app: &mut App,
    owner: Addr,
    astro_token_instance: Addr,
) -> (Addr, Addr) {
    let airdrop_contract = Box::new(ContractWrapper::new(
        astroport_airdrop::contract::execute,
        astroport_airdrop::contract::instantiate,
        astroport_airdrop::contract::query,
    ));

    let lockdrop_contract = Box::new(ContractWrapper::new(
        astroport_lockdrop::contract::execute,
        astroport_lockdrop::contract::instantiate,
        astroport_lockdrop::contract::query,
    ));

    let airdrop_code_id = app.store_code(airdrop_contract);
    let lockdrop_code_id = app.store_code(lockdrop_contract);

    let airdrop_msg = astroport_periphery::airdrop::InstantiateMsg {
        owner: Some(owner.clone().to_string()),
        astro_token_address: astro_token_instance.clone().into_string(),
        merkle_roots: Some(vec!["merkle_roots".to_string()]),
        from_timestamp: Some(1_000_00),
        to_timestamp: 100_000_00,
    };

    let lockdrop_msg = astroport_periphery::lockdrop::InstantiateMsg {
        owner: Some(owner.to_string()),
        init_timestamp: 1_000_00,
        deposit_window: 100_000_00,
        withdrawal_window: 5_000_00,
        weekly_multiplier: 3,
        weekly_divider: 51,
        min_lock_duration: 1u64,
        max_lock_duration: 52u64,
        max_positions_per_user: 24,
    };

    let airdrop_instance = app
        .instantiate_contract(
            airdrop_code_id,
            owner.clone(),
            &airdrop_msg,
            &[],
            String::from("airdrop_instance"),
            None,
        )
        .unwrap();

    // mint ASTRO for to Owner
    mint_some_astro(
        app,
        Addr::unchecked(owner.clone()),
        astro_token_instance.clone(),
        Uint128::from(100_000_000_000u64),
        owner.clone().to_string(),
    );

    // Set ASTRO airdrop incentives
    app.execute_contract(
        Addr::unchecked(owner.clone()),
        astro_token_instance.clone(),
        &Cw20ExecuteMsg::Send {
            amount: Uint128::from(100_000_000_000u64),
            contract: airdrop_instance.to_string(),
            msg: to_binary(&Cw20HookMsg::IncreaseAstroIncentives {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    // open claim period for successful deposit
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(900_00)
    });

    let lockdrop_instance = app
        .instantiate_contract(
            lockdrop_code_id,
            owner.clone(),
            &lockdrop_msg,
            &[],
            String::from("lockdrop_instance"),
            None,
        )
        .unwrap();

    mint_some_astro(
        app,
        owner.clone(),
        astro_token_instance.clone(),
        Uint128::new(100_000_00u128),
        owner.to_string(),
    );
    app.execute_contract(
        owner.clone(),
        astro_token_instance.clone(),
        &Cw20ExecuteMsg::IncreaseAllowance {
            spender: lockdrop_instance.clone().to_string(),
            amount: Uint128::new(900_000_000_000),
            expires: None,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        owner.clone(),
        lockdrop_instance.clone(),
        &astroport_periphery::lockdrop::ExecuteMsg::UpdateConfig {
            new_config: astroport_periphery::lockdrop::UpdateConfigMsg {
                owner: None,
                astro_token_address: Some(astro_token_instance.clone().into_string()),
                auction_contract_address: None,
                generator_address: None,
            },
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        owner.clone(),
        astro_token_instance,
        &Cw20ExecuteMsg::Send {
            amount: Uint128::from(100_000_00u64),
            contract: lockdrop_instance.to_string(),
            msg: to_binary(&Cw20HookMsg::IncreaseAstroIncentives {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    (airdrop_instance, lockdrop_instance)
}

// Instantiate Astroport's generator and vesting contracts
fn instantiate_generator_and_vesting(
    mut app: &mut App,
    owner: Addr,
    astro_token_instance: Addr,
    lp_token_instance: Addr,
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
        &Cw20ExecuteMsg::IncreaseAllowance {
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
        owner: owner.to_string(),
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

    let msg = astroport::generator::ExecuteMsg::UpdateConfig {
        owner: None,
        tokens_per_block: Some(tokens_per_block),
        vesting_contract: None,
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

    let msg = Cw20ExecuteMsg::Send {
        contract: vesting_instance.to_string(),
        amount,
        msg: to_binary(&astroport::vesting::Cw20HookMsg::RegisterVestingAccounts {
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
        })
        .unwrap(),
    };

    app.execute_contract(owner.clone(), astro_token_instance.clone(), &msg, &[])
        .unwrap();

    let msg = astroport::generator::ExecuteMsg::Add {
        alloc_point: Uint64::from(10u64),
        reward_proxy: None,
        lp_token: lp_token_instance.clone(),
    };
    app.execute_contract(
        Addr::unchecked(owner.clone()),
        generator_instance.clone(),
        &msg,
        &[],
    )
    .unwrap();

    (generator_instance, vesting_instance)
}

// Mints some ASTRO to "to" recipient
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

// Makes ASTRO & UST deposits into Auction contract
fn make_astro_ust_deposits(
    app: &mut App,
    auction_instance: Addr,
    auction_init_msg: InstantiateMsg,
    astro_token_instance: Addr,
) -> (Addr, Addr, Addr) {
    let user1_address = Addr::unchecked("user1");
    let user2_address = Addr::unchecked("user2");
    let user3_address = Addr::unchecked("user3");

    // open claim period for successful deposit
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_01)
    });

    // ######    SUCCESS :: ASTRO Successfully deposited     ######
    app.execute_contract(
        Addr::unchecked(auction_init_msg.lockdrop_contract_address.clone()),
        astro_token_instance.clone(),
        &Cw20ExecuteMsg::Send {
            contract: auction_instance.clone().to_string(),
            amount: Uint128::new(100000000),
            msg: to_binary(&Cw20HookMsg::DelegateAstroTokens {
                user_address: user1_address.to_string(),
            })
            .unwrap(),
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked(auction_init_msg.lockdrop_contract_address.clone()),
        astro_token_instance.clone(),
        &Cw20ExecuteMsg::Send {
            contract: auction_instance.clone().to_string(),
            amount: Uint128::new(65435340),
            msg: to_binary(&Cw20HookMsg::DelegateAstroTokens {
                user_address: user2_address.to_string(),
            })
            .unwrap(),
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked(auction_init_msg.lockdrop_contract_address.clone()),
        astro_token_instance.clone(),
        &Cw20ExecuteMsg::Send {
            contract: auction_instance.clone().to_string(),
            amount: Uint128::new(76754654),
            msg: to_binary(&Cw20HookMsg::DelegateAstroTokens {
                user_address: user3_address.to_string(),
            })
            .unwrap(),
        },
        &[],
    )
    .unwrap();

    // Set user balances
    app.init_bank_balance(
        &user1_address.clone(),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(20000000u128),
        }],
    )
    .unwrap();
    app.init_bank_balance(
        &user2_address.clone(),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(5435435u128),
        }],
    )
    .unwrap();
    app.init_bank_balance(
        &user3_address.clone(),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(43534534u128),
        }],
    )
    .unwrap();

    // deposit UST Msg
    let deposit_ust_msg = &ExecuteMsg::DepositUst {};

    // ######    SUCCESS :: UST Successfully deposited     ######
    app.execute_contract(
        user1_address.clone(),
        auction_instance.clone(),
        &deposit_ust_msg,
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(432423u128),
        }],
    )
    .unwrap();

    app.execute_contract(
        user2_address.clone(),
        auction_instance.clone(),
        &deposit_ust_msg,
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(454353u128),
        }],
    )
    .unwrap();

    app.execute_contract(
        user3_address.clone(),
        auction_instance.clone(),
        &deposit_ust_msg,
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(5643543u128),
        }],
    )
    .unwrap();

    (user1_address, user2_address, user3_address)
}

#[test]
fn proper_initialization_only_auction_astro() {
    let mut app = mock_app();
    let (_, _, auction_instance, _, auction_init_msg) = init_auction_astro_contracts(&mut app);

    let resp: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::Config {})
        .unwrap();

    // Check config
    assert_eq!(Addr::unchecked(auction_init_msg.owner.unwrap()), resp.owner);
    assert_eq!(
        auction_init_msg.astro_token_address,
        resp.astro_token_address
    );
    assert_eq!(
        auction_init_msg.airdrop_contract_address,
        resp.airdrop_contract_address
    );
    assert_eq!(
        auction_init_msg.lockdrop_contract_address,
        resp.lockdrop_contract_address
    );
    assert_eq!(auction_init_msg.init_timestamp, resp.init_timestamp);
    assert_eq!(auction_init_msg.deposit_window, resp.deposit_window);
    assert_eq!(auction_init_msg.withdrawal_window, resp.withdrawal_window);

    // Check state
    let resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::State {})
        .unwrap();

    assert!(resp.total_astro_delegated.is_zero());
    assert!(resp.total_ust_delegated.is_zero());
    assert!(resp.lp_shares_minted.is_none());
    assert!(!resp.is_lp_staked);
    assert_eq!(0u64, resp.pool_init_timestamp);
    assert!(resp.generator_astro_per_share.is_zero());
}

#[test]
fn proper_initialization_all_contracts() {
    let mut app = mock_app();
    let (auction_instance, _, _, _, _, _, auction_init_msg) = init_all_contracts(&mut app);

    let resp: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::Config {})
        .unwrap();

    // Check config
    assert_eq!(auction_init_msg.owner, Some(resp.owner.to_string()));
    assert_eq!(
        auction_init_msg.astro_token_address,
        resp.astro_token_address
    );
    assert_eq!(
        auction_init_msg.airdrop_contract_address,
        resp.airdrop_contract_address
    );
    assert_eq!(
        auction_init_msg.lockdrop_contract_address,
        resp.lockdrop_contract_address
    );
    assert_eq!(auction_init_msg.init_timestamp, resp.init_timestamp);
    assert_eq!(auction_init_msg.deposit_window, resp.deposit_window);
    assert_eq!(auction_init_msg.withdrawal_window, resp.withdrawal_window);

    // Check state
    let resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::State {})
        .unwrap();

    assert!(resp.total_astro_delegated.is_zero());
    assert!(resp.total_ust_delegated.is_zero());
    assert!(resp.lp_shares_minted.is_none());
    assert!(!resp.is_lp_staked);
    assert_eq!(0u64, resp.pool_init_timestamp);
    assert!(resp.generator_astro_per_share.is_zero());
}

#[test]
fn test_delegate_astro_tokens_from_airdrop() {
    let mut app = mock_app();
    let (airdrop_instance, _, auction_instance, astro_token_instance, auction_init_msg) =
        init_auction_astro_contracts(&mut app);

    // mint ASTRO for to Airdrop Contract
    mint_some_astro(
        &mut app,
        Addr::unchecked(auction_init_msg.owner.clone().unwrap()),
        astro_token_instance.clone(),
        Uint128::new(100_000_000_000),
        airdrop_instance.to_string(),
    );

    // mint ASTRO for to Wrong Airdrop Contract
    mint_some_astro(
        &mut app,
        Addr::unchecked(auction_init_msg.owner.unwrap()),
        astro_token_instance.clone(),
        Uint128::new(100_000_000_000),
        "not_airdrop_instance".to_string(),
    );

    // deposit ASTRO Msg
    let send_cw20_msg = &Cw20ExecuteMsg::Send {
        contract: auction_instance.clone().to_string(),
        amount: Uint128::new(100000000),
        msg: to_binary(&Cw20HookMsg::DelegateAstroTokens {
            user_address: "airdrop_recipient".to_string(),
        })
        .unwrap(),
    };

    // ######    ERROR :: Unauthorized     ######
    let mut err = app
        .execute_contract(
            Addr::unchecked("not_airdrop_instance"),
            astro_token_instance.clone(),
            &send_cw20_msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Unauthorized");

    // ######    ERROR :: Amount must be greater than 0     ######
    err = app
        .execute_contract(
            airdrop_instance.clone(),
            astro_token_instance.clone(),
            &Cw20ExecuteMsg::Send {
                contract: auction_instance.clone().to_string(),
                amount: Uint128::new(0),
                msg: to_binary(&Cw20HookMsg::DelegateAstroTokens {
                    user_address: "airdrop_recipient".to_string(),
                })
                .unwrap(),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Invalid zero amount");

    // ######    ERROR :: Deposit window closed     ######
    err = app
        .execute_contract(
            airdrop_instance.clone(),
            astro_token_instance.clone(),
            &send_cw20_msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Deposit window closed");

    // open claim period for successful deposit
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_01)
    });

    // ######    SUCCESS :: ASTRO Successfully deposited     ######
    app.execute_contract(
        airdrop_instance.clone(),
        astro_token_instance.clone(),
        &send_cw20_msg,
        &[],
    )
    .unwrap();
    // Check state response
    let state_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(
        Uint128::from(100000000u64),
        state_resp.total_astro_delegated
    );
    assert_eq!(Uint128::from(0u64), state_resp.total_ust_delegated);
    assert_eq!(None, state_resp.lp_shares_minted);
    assert!(!state_resp.is_lp_staked);
    assert!(state_resp.generator_astro_per_share.is_zero());
    // Check user response
    let user_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &QueryMsg::UserInfo {
                address: "airdrop_recipient".to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(100000000u64), user_resp.astro_delegated);
    assert_eq!(Uint128::from(0u64), user_resp.ust_delegated);
    assert_eq!(None, user_resp.lp_shares);
    assert_eq!(None, user_resp.withdrawable_lp_shares);
    assert_eq!(None, user_resp.auction_incentive_amount);

    // ######    SUCCESS :: ASTRO Successfully deposited again   ######
    app.execute_contract(
        airdrop_instance.clone(),
        astro_token_instance.clone(),
        &send_cw20_msg,
        &[],
    )
    .unwrap();
    // Check state response
    let state_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(
        Uint128::from(200000000u64),
        state_resp.total_astro_delegated
    );
    assert_eq!(Uint128::from(0u64), state_resp.total_ust_delegated);
    assert_eq!(None, state_resp.lp_shares_minted);
    assert!(!state_resp.is_lp_staked);
    assert!(state_resp.generator_astro_per_share.is_zero());
    // Check user response
    let user_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &QueryMsg::UserInfo {
                address: "airdrop_recipient".to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(200000000u64), user_resp.astro_delegated);
    assert_eq!(Uint128::from(0u64), user_resp.ust_delegated);
    assert_eq!(None, user_resp.lp_shares);
    assert_eq!(None, user_resp.withdrawable_lp_shares);
    assert_eq!(None, user_resp.auction_incentive_amount);

    // ######    ERROR :: Deposit window closed     ######

    // finish claim period for deposit failure
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10100001)
    });
    err = app
        .execute_contract(
            airdrop_instance,
            astro_token_instance.clone(),
            &send_cw20_msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Deposit window closed");
}

#[test]
fn test_delegate_astro_tokens_from_lockdrop() {
    let mut app = mock_app();
    let (_, lockdrop_instance, auction_instance, astro_token_instance, auction_init_msg) =
        init_auction_astro_contracts(&mut app);

    // mint ASTRO for to Lockdrop Contract
    mint_some_astro(
        &mut app,
        Addr::unchecked(auction_init_msg.owner.clone().unwrap()),
        astro_token_instance.clone(),
        Uint128::new(100_000_000_000),
        lockdrop_instance.to_string(),
    );

    // mint ASTRO for to Wrong Lockdrop Contract
    mint_some_astro(
        &mut app,
        Addr::unchecked(auction_init_msg.owner.unwrap()),
        astro_token_instance.clone(),
        Uint128::new(100_000_000_000),
        "not_lockdrop_instance".to_string(),
    );

    // deposit ASTRO Msg
    let send_cw20_msg = &Cw20ExecuteMsg::Send {
        contract: auction_instance.clone().to_string(),
        amount: Uint128::new(100000000),
        msg: to_binary(&Cw20HookMsg::DelegateAstroTokens {
            user_address: "lockdrop_participant".to_string(),
        })
        .unwrap(),
    };

    // ######    ERROR :: Unauthorized     ######
    let mut err = app
        .execute_contract(
            Addr::unchecked("not_lockdrop_instance"),
            astro_token_instance.clone(),
            &send_cw20_msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Unauthorized");

    // ######    ERROR :: Amount must be greater than 0     ######
    err = app
        .execute_contract(
            lockdrop_instance.clone(),
            astro_token_instance.clone(),
            &Cw20ExecuteMsg::Send {
                contract: auction_instance.clone().to_string(),
                amount: Uint128::new(0),
                msg: to_binary(&Cw20HookMsg::DelegateAstroTokens {
                    user_address: "lockdrop_participant".to_string(),
                })
                .unwrap(),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Invalid zero amount");

    // ######    ERROR :: Deposit window closed     ######
    err = app
        .execute_contract(
            lockdrop_instance.clone(),
            astro_token_instance.clone(),
            &send_cw20_msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Deposit window closed");

    // open claim period for successful deposit
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_01)
    });

    // ######    SUCCESS :: ASTRO Successfully deposited     ######
    app.execute_contract(
        lockdrop_instance.clone(),
        astro_token_instance.clone(),
        &send_cw20_msg,
        &[],
    )
    .unwrap();
    // Check state response
    let state_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(
        Uint128::from(100000000u64),
        state_resp.total_astro_delegated
    );

    // Check user response
    let user_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &QueryMsg::UserInfo {
                address: "lockdrop_participant".to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(100000000u64), user_resp.astro_delegated);
    assert_eq!(Uint128::from(0u64), user_resp.ust_delegated);

    // ######    SUCCESS :: ASTRO Successfully deposited again   ######
    app.execute_contract(
        lockdrop_instance.clone(),
        astro_token_instance.clone(),
        &send_cw20_msg,
        &[],
    )
    .unwrap();
    // Check state response
    let state_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(
        Uint128::from(200000000u64),
        state_resp.total_astro_delegated
    );
    assert_eq!(Uint128::from(0u64), state_resp.total_ust_delegated);

    // Check user response
    let user_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &QueryMsg::UserInfo {
                address: "lockdrop_participant".to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(200000000u64), user_resp.astro_delegated);

    // ######    ERROR :: Deposit window closed     ######

    // finish claim period for deposit failure
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10100001)
    });
    err = app
        .execute_contract(
            lockdrop_instance,
            astro_token_instance.clone(),
            &send_cw20_msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Deposit window closed");
}

#[test]
fn test_update_config() {
    let mut app = mock_app();
    let (_, _, auction_instance, _, auction_init_msg) = init_auction_astro_contracts(&mut app);

    let update_msg = UpdateConfigMsg {
        owner: Some("new_owner".to_string()),
        astro_ust_pair_address: None,
        generator_contract: Some("generator_contract".to_string()),
    };

    // ######    ERROR :: Only owner can update configuration     ######
    let err = app
        .execute_contract(
            Addr::unchecked("wrong_owner"),
            auction_instance.clone(),
            &ExecuteMsg::UpdateConfig {
                new_config: update_msg.clone(),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Only owner can update configuration"
    );

    // ######    SUCCESS :: Should have successfully updated   ######
    app.execute_contract(
        Addr::unchecked(auction_init_msg.owner.unwrap()),
        auction_instance.clone(),
        &ExecuteMsg::UpdateConfig {
            new_config: update_msg.clone(),
        },
        &[],
    )
    .unwrap();

    let resp: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::Config {})
        .unwrap();
    // Check config
    assert_eq!(update_msg.clone().owner.unwrap(), resp.owner);
    assert_eq!(
        update_msg.clone().generator_contract.unwrap(),
        resp.generator_contract.unwrap()
    );
}

#[test]
fn test_deposit_ust() {
    let mut app = mock_app();
    let (_, _, auction_instance, _, _) = init_auction_astro_contracts(&mut app);
    let user_address = Addr::unchecked("user");

    // Set user balances
    app.init_bank_balance(
        &user_address.clone(),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(20000000u128),
        }],
    )
    .unwrap();

    // deposit UST Msg
    let deposit_ust_msg = &ExecuteMsg::DepositUst {};
    let coins = [Coin {
        denom: "uusd".to_string(),
        amount: Uint128::from(10000u128),
    }];

    // ######    ERROR :: Deposit window closed     ######
    let mut err = app
        .execute_contract(
            user_address.clone(),
            auction_instance.clone(),
            &deposit_ust_msg,
            &coins,
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Deposit window closed");

    // open claim period for successful deposit
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_01)
    });

    // ######    ERROR :: Amount must be greater than 0     ######
    err = app
        .execute_contract(
            user_address.clone(),
            auction_instance.clone(),
            &deposit_ust_msg,
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(0u128),
            }],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Amount must be greater than 0"
    );

    // ######    SUCCESS :: UST Successfully deposited     ######
    app.execute_contract(
        user_address.clone(),
        auction_instance.clone(),
        &deposit_ust_msg,
        &coins,
    )
    .unwrap();
    // Check state response
    let mut state_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(Uint128::from(00u64), state_resp.total_astro_delegated);
    assert_eq!(Uint128::from(10000u64), state_resp.total_ust_delegated);
    assert_eq!(None, state_resp.lp_shares_minted);
    assert!(!state_resp.is_lp_staked);

    // Check user response
    let mut user_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &QueryMsg::UserInfo {
                address: user_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(0u64), user_resp.astro_delegated);
    assert_eq!(Uint128::from(10000u64), user_resp.ust_delegated);
    assert_eq!(None, user_resp.lp_shares);
    assert_eq!(None, user_resp.withdrawable_lp_shares);
    assert_eq!(None, user_resp.auction_incentive_amount);

    // ######    SUCCESS :: UST Successfully deposited again     ######
    app.execute_contract(
        user_address.clone(),
        auction_instance.clone(),
        &deposit_ust_msg,
        &coins,
    )
    .unwrap();
    // Check state response
    state_resp = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(Uint128::from(00u64), state_resp.total_astro_delegated);
    assert_eq!(Uint128::from(20000u64), state_resp.total_ust_delegated);

    // Check user response
    user_resp = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &QueryMsg::UserInfo {
                address: user_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(0u64), user_resp.astro_delegated);
    assert_eq!(Uint128::from(20000u64), user_resp.ust_delegated);

    // finish claim period for deposit failure
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10100001)
    });
    err = app
        .execute_contract(
            user_address.clone(),
            auction_instance.clone(),
            &deposit_ust_msg,
            &coins,
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Deposit window closed");
}

#[test]
fn test_withdraw_ust() {
    let mut app = mock_app();
    let (_, _, auction_instance, _, _) = init_auction_astro_contracts(&mut app);
    let user1_address = Addr::unchecked("user1");
    let user2_address = Addr::unchecked("user2");
    let user3_address = Addr::unchecked("user3");

    // Set user balances
    app.init_bank_balance(
        &user1_address.clone(),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(20000000u128),
        }],
    )
    .unwrap();
    app.init_bank_balance(
        &user2_address.clone(),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(20000000u128),
        }],
    )
    .unwrap();
    app.init_bank_balance(
        &user3_address.clone(),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(20000000u128),
        }],
    )
    .unwrap();

    // deposit UST Msg
    let deposit_ust_msg = &ExecuteMsg::DepositUst {};
    let coins = [Coin {
        denom: "uusd".to_string(),
        amount: Uint128::from(10000u128),
    }];

    // open claim period for successful deposit
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_01)
    });

    // ######    SUCCESS :: UST Successfully deposited     ######
    app.execute_contract(
        user1_address.clone(),
        auction_instance.clone(),
        &deposit_ust_msg,
        &coins,
    )
    .unwrap();
    app.execute_contract(
        user2_address.clone(),
        auction_instance.clone(),
        &deposit_ust_msg,
        &coins,
    )
    .unwrap();
    app.execute_contract(
        user3_address.clone(),
        auction_instance.clone(),
        &deposit_ust_msg,
        &coins,
    )
    .unwrap();

    // ######    SUCCESS :: UST Successfully withdrawn (when withdrawals allowed)     ######
    app.execute_contract(
        user1_address.clone(),
        auction_instance.clone(),
        &ExecuteMsg::WithdrawUst {
            amount: Uint128::from(10000u64),
        },
        &[],
    )
    .unwrap();
    // Check state response
    let mut state_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(Uint128::from(20000u64), state_resp.total_ust_delegated);

    // Check user response
    let mut user_resp: UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &QueryMsg::UserInfo {
                address: user1_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(0u64), user_resp.ust_delegated);

    app.execute_contract(
        user1_address.clone(),
        auction_instance.clone(),
        &deposit_ust_msg,
        &coins,
    )
    .unwrap();

    // close deposit window. Max 50% withdrawals allowed now
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10100001)
    });

    // ######    ERROR :: Amount exceeds maximum allowed withdrawal limit of {}   ######

    let mut err = app
        .execute_contract(
            user1_address.clone(),
            auction_instance.clone(),
            &ExecuteMsg::WithdrawUst {
                amount: Uint128::from(10000u64),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Amount exceeds maximum allowed withdrawal limit of 0.5"
    );

    // ######    SUCCESS :: Withdraw 50% successfully   ######

    app.execute_contract(
        user1_address.clone(),
        auction_instance.clone(),
        &ExecuteMsg::WithdrawUst {
            amount: Uint128::from(5000u64),
        },
        &[],
    )
    .unwrap();
    // Check state response
    state_resp = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(Uint128::from(25000u64), state_resp.total_ust_delegated);

    // Check user response
    user_resp = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &QueryMsg::UserInfo {
                address: user1_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(5000u64), user_resp.ust_delegated);

    // ######    ERROR :: Max 1 withdrawal allowed during current window   ######

    err = app
        .execute_contract(
            user1_address.clone(),
            auction_instance.clone(),
            &ExecuteMsg::WithdrawUst {
                amount: Uint128::from(10u64),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Max 1 withdrawal allowed");

    // 50% of withdrawal window over. Max withdrawal % decreasing linearly now
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10351001)
    });

    // ######    ERROR :: Amount exceeds maximum allowed withdrawal limit of {}   ######

    let mut err = app
        .execute_contract(
            user2_address.clone(),
            auction_instance.clone(),
            &ExecuteMsg::WithdrawUst {
                amount: Uint128::from(10000u64),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Amount exceeds maximum allowed withdrawal limit of 0.497998"
    );

    // ######    SUCCESS :: Withdraw some UST successfully   ######

    app.execute_contract(
        user2_address.clone(),
        auction_instance.clone(),
        &ExecuteMsg::WithdrawUst {
            amount: Uint128::from(2000u64),
        },
        &[],
    )
    .unwrap();
    // Check state response
    state_resp = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(Uint128::from(23000u64), state_resp.total_ust_delegated);

    // Check user response
    user_resp = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &QueryMsg::UserInfo {
                address: user2_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(8000u64), user_resp.ust_delegated);

    // ######    ERROR :: Max 1 withdrawal allowed during current window   ######

    err = app
        .execute_contract(
            user2_address.clone(),
            auction_instance.clone(),
            &ExecuteMsg::WithdrawUst {
                amount: Uint128::from(10u64),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Max 1 withdrawal allowed");

    // finish deposit period for deposit failure
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10611001)
    });

    err = app
        .execute_contract(
            user3_address.clone(),
            auction_instance.clone(),
            &ExecuteMsg::WithdrawUst {
                amount: Uint128::from(10u64),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Amount exceeds maximum allowed withdrawal limit of 0"
    );
}

#[test]
fn test_add_liquidity_to_astroport_pool() {
    let mut app = mock_app();
    let (
        auction_instance,
        astro_token_instance,
        airdrop_instance,
        lockdrop_instance,
        pair_instance,
        _,
        auction_init_msg,
    ) = init_all_contracts(&mut app);

    // mint ASTRO to Lockdrop Contract
    mint_some_astro(
        &mut app,
        Addr::unchecked(auction_init_msg.owner.clone().unwrap()),
        astro_token_instance.clone(),
        Uint128::new(100_000_000_000),
        auction_init_msg.lockdrop_contract_address.to_string(),
    );

    let (user1_address, user2_address, user3_address) = make_astro_ust_deposits(
        &mut app,
        auction_instance.clone(),
        auction_init_msg.clone(),
        astro_token_instance.clone(),
    );

    // ######    ERROR :: Unauthorized   ######

    let mut err = app
        .execute_contract(
            Addr::unchecked("not_owner".to_string()),
            auction_instance.clone(),
            &ExecuteMsg::InitPool { slippage: None },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Unauthorized");

    // ######    ERROR :: Deposit/withdrawal windows are still open   ######

    err = app
        .execute_contract(
            Addr::unchecked(auction_init_msg.owner.clone().unwrap()),
            auction_instance.clone(),
            &ExecuteMsg::InitPool { slippage: None },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Deposit/withdrawal windows are still open"
    );

    // finish deposit / withdraw period
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10611001)
    });

    // mint ASTRO to owner
    mint_some_astro(
        &mut app,
        Addr::unchecked(auction_init_msg.owner.clone().unwrap()),
        astro_token_instance.clone(),
        Uint128::new(100_000_000_000),
        lockdrop_instance.to_string(),
    );

    app.execute_contract(
        lockdrop_instance.clone(),
        astro_token_instance,
        &Cw20ExecuteMsg::Send {
            amount: Uint128::new(100_000_000000),
            contract: auction_instance.to_string(),
            msg: to_binary(&Cw20HookMsg::IncreaseAstroIncentives {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    let success_ = app
        .execute_contract(
            Addr::unchecked(auction_init_msg.owner.clone().unwrap()),
            auction_instance.clone(),
            &ExecuteMsg::InitPool { slippage: None },
            &[],
        )
        .unwrap();
    assert_eq!(
        success_.events[1].attributes[1],
        attr("action", "Auction::ExecuteMsg::AddLiquidityToAstroportPool")
    );
    assert_eq!(
        success_.events[1].attributes[2],
        attr("astro_provided", "242189994")
    );
    assert_eq!(
        success_.events[1].attributes[3],
        attr("ust_provided", "6530319")
    );

    // Auction :: Check state response
    let state_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(
        Uint128::from(242189994u64),
        state_resp.total_astro_delegated
    );
    assert_eq!(Uint128::from(6530319u64), state_resp.total_ust_delegated);
    assert_eq!(
        Some(Uint128::from(39769057u64)),
        state_resp.lp_shares_minted
    );
    assert!(!state_resp.is_lp_staked);
    assert!(state_resp.generator_astro_per_share.is_zero());
    assert_eq!(10611001u64, state_resp.pool_init_timestamp);

    // Astroport Pool :: Check response
    let pool_resp: astroport::pair::PoolResponse = app
        .wrap()
        .query_wasm_smart(&pair_instance, &astroport::pair::QueryMsg::Pool {})
        .unwrap();
    assert_eq!(Uint128::from(39769057u64), pool_resp.total_share);

    // Airdrop :: Check config for claims
    let airdrop_config_resp: astroport_periphery::airdrop::ConfigResponse = app
        .wrap()
        .query_wasm_smart(
            &airdrop_instance,
            &astroport_periphery::airdrop::QueryMsg::Config {},
        )
        .unwrap();
    assert_eq!(true, airdrop_config_resp.are_claims_allowed);

    // Lockdrop :: Check state for claims
    let lockdrop_config_resp: astroport_periphery::lockdrop::StateResponse = app
        .wrap()
        .query_wasm_smart(
            &lockdrop_instance,
            &astroport_periphery::lockdrop::QueryMsg::State {},
        )
        .unwrap();
    assert_eq!(true, lockdrop_config_resp.are_claims_allowed);

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10911001)
    });

    // Auction :: Check user-1 state
    let user1info_resp: astroport_periphery::auction::UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &astroport_periphery::auction::QueryMsg::UserInfo {
                address: user1_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(100000000u64), user1info_resp.astro_delegated);
    assert_eq!(Uint128::from(432423u64), user1info_resp.ust_delegated);
    assert_eq!(Some(Uint128::from(9527010u64)), user1info_resp.lp_shares);
    assert_eq!(
        Some(Uint128::from(367554u64)),
        user1info_resp.withdrawable_lp_shares
    );
    assert_eq!(
        Some(Uint128::from(23955835814u64)),
        user1info_resp.auction_incentive_amount
    );
    assert!(user1info_resp.generator_astro_debt.is_zero());

    // Auction :: Check user-2 state
    let user2info_resp: astroport_periphery::auction::UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &astroport_periphery::auction::QueryMsg::UserInfo {
                address: user2_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(65435340u64), user2info_resp.astro_delegated);
    assert_eq!(Uint128::from(454353u64), user2info_resp.ust_delegated);
    assert_eq!(Some(Uint128::from(6755923u64)), user2info_resp.lp_shares);
    assert_eq!(
        Some(Uint128::from(260645u64)),
        user2info_resp.withdrawable_lp_shares
    );
    assert_eq!(
        Some(Uint128::from(16987888347u64)),
        user2info_resp.auction_incentive_amount
    );

    // Auction :: Check user-3 state
    let user3info_resp: astroport_periphery::auction::UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &astroport_periphery::auction::QueryMsg::UserInfo {
                address: user3_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(76754654u64), user3info_resp.astro_delegated);
    assert_eq!(Uint128::from(5643543u64), user3info_resp.ust_delegated);
    assert_eq!(Some(Uint128::from(23486123u64)), user3info_resp.lp_shares);
    assert_eq!(
        Some(Uint128::from(906100u64)),
        user3info_resp.withdrawable_lp_shares
    );
    assert_eq!(
        Some(Uint128::from(59056273323u64)),
        user3info_resp.auction_incentive_amount
    );

    // ######    ERROR :: Liquidity already added   ######
    // user1_address, user2_address, user3_address
    err = app
        .execute_contract(
            Addr::unchecked(auction_init_msg.owner.unwrap()),
            auction_instance.clone(),
            &ExecuteMsg::InitPool { slippage: None },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Liquidity already added");
}

#[test]
fn test_stake_lp_tokens() {
    let mut app = mock_app();
    let (auction_instance, astro_token_instance, _, _, _, lp_token_instance, auction_init_msg) =
        init_all_contracts(&mut app);

    let owner = Addr::unchecked(auction_init_msg.owner.clone().unwrap());

    // mint ASTRO to Lockdrop Contract
    mint_some_astro(
        &mut app,
        Addr::unchecked(auction_init_msg.owner.clone().unwrap()),
        astro_token_instance.clone(),
        Uint128::new(100_000_000_000),
        auction_init_msg.lockdrop_contract_address.to_string(),
    );

    let (user1_address, user2_address, user3_address) = make_astro_ust_deposits(
        &mut app,
        auction_instance.clone(),
        auction_init_msg.clone(),
        astro_token_instance.clone(),
    );

    // ######    Initialize generator and vesting instance   ######
    let (generator_instance, _) = instantiate_generator_and_vesting(
        &mut app,
        Addr::unchecked(auction_init_msg.owner.clone().unwrap()),
        astro_token_instance.clone(),
        lp_token_instance.clone(),
    );

    let update_msg = UpdateConfigMsg {
        owner: None,
        astro_ust_pair_address: None,
        generator_contract: Some(generator_instance.to_string()),
    };

    app.execute_contract(
        Addr::unchecked(auction_init_msg.owner.clone().unwrap()),
        auction_instance.clone(),
        &ExecuteMsg::UpdateConfig {
            new_config: update_msg.clone(),
        },
        &[],
    )
    .unwrap();

    // finish deposit / withdraw period
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10611001)
    });

    app.execute_contract(
        owner.clone(),
        astro_token_instance,
        &Cw20ExecuteMsg::Send {
            amount: Uint128::new(100_000_000000),
            contract: auction_instance.to_string(),
            msg: to_binary(&Cw20HookMsg::IncreaseAstroIncentives {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    let _success = app
        .execute_contract(
            Addr::unchecked(auction_init_msg.owner.clone().unwrap()),
            auction_instance.clone(),
            &ExecuteMsg::InitPool { slippage: None },
            &[],
        )
        .unwrap();

    // ######    ERROR :: Unauthorized   ######

    let mut err = app
        .execute_contract(
            Addr::unchecked("not_owner".to_string()),
            auction_instance.clone(),
            &ExecuteMsg::StakeLpTokens {},
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Unauthorized");

    // ######    SUCCESS :: Stake successfully   ######

    let success_ = app
        .execute_contract(
            Addr::unchecked(auction_init_msg.owner.clone().unwrap()),
            auction_instance.clone(),
            &ExecuteMsg::StakeLpTokens {},
            &[],
        )
        .unwrap();
    assert_eq!(
        success_.events[1].attributes[1],
        attr("action", "Auction::ExecuteMsg::StakeLPTokens")
    );
    assert_eq!(
        success_.events[1].attributes[2],
        attr("staked_amount", "39769057")
    );

    // Auction :: Check state response
    let state_resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&auction_instance, &QueryMsg::State {})
        .unwrap();
    assert_eq!(
        Uint128::from(242189994u64),
        state_resp.total_astro_delegated
    );
    assert_eq!(Uint128::from(6530319u64), state_resp.total_ust_delegated);
    assert_eq!(
        Some(Uint128::from(39769057u64)),
        state_resp.lp_shares_minted
    );
    assert!(state_resp.is_lp_staked);
    assert_eq!(10611001u64, state_resp.pool_init_timestamp);

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10911001)
    });

    // Auction :: Check user-1 state
    let user1info_resp: astroport_periphery::auction::UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &astroport_periphery::auction::QueryMsg::UserInfo {
                address: user1_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(100000000u64), user1info_resp.astro_delegated);
    assert_eq!(Uint128::from(432423u64), user1info_resp.ust_delegated);
    assert_eq!(Some(Uint128::from(9527010u64)), user1info_resp.lp_shares);
    assert_eq!(Uint128::from(0u64), user1info_resp.claimed_lp_shares);
    assert_eq!(
        Some(Uint128::from(367554u64)),
        user1info_resp.withdrawable_lp_shares
    );
    assert_eq!(
        Some(Uint128::from(23955835814u64)),
        user1info_resp.auction_incentive_amount
    );

    // Auction :: Check user-2 state
    let user2info_resp: astroport_periphery::auction::UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &astroport_periphery::auction::QueryMsg::UserInfo {
                address: user2_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(65435340u64), user2info_resp.astro_delegated);
    assert_eq!(Uint128::from(454353u64), user2info_resp.ust_delegated);
    assert_eq!(Some(Uint128::from(6755923u64)), user2info_resp.lp_shares);
    assert_eq!(Uint128::from(0u64), user2info_resp.claimed_lp_shares);
    assert_eq!(
        Some(Uint128::from(260645u64)),
        user2info_resp.withdrawable_lp_shares
    );
    assert_eq!(
        Some(Uint128::from(16987888347u64)),
        user2info_resp.auction_incentive_amount
    );

    // Auction :: Check user-3 state
    let user3info_resp: astroport_periphery::auction::UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &astroport_periphery::auction::QueryMsg::UserInfo {
                address: user3_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(76754654u64), user3info_resp.astro_delegated);
    assert_eq!(Uint128::from(5643543u64), user3info_resp.ust_delegated);
    assert_eq!(Some(Uint128::from(23486123u64)), user3info_resp.lp_shares);
    assert_eq!(Uint128::from(0u64), user3info_resp.claimed_lp_shares);
    assert_eq!(
        Some(Uint128::from(906100u64)),
        user3info_resp.withdrawable_lp_shares
    );
    assert_eq!(
        Some(Uint128::from(59056273323u64)),
        user3info_resp.auction_incentive_amount
    );

    // ######    ERROR :: Already staked   ######

    err = app
        .execute_contract(
            Addr::unchecked(auction_init_msg.owner.unwrap()),
            auction_instance.clone(),
            &ExecuteMsg::StakeLpTokens {},
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Already staked");
}

#[test]
fn test_claim_rewards() {
    let mut app = mock_app();
    let (auction_instance, astro_token_instance, _, _, _, lp_token_instance, auction_init_msg) =
        init_all_contracts(&mut app);

    let owner = Addr::unchecked(auction_init_msg.owner.clone().unwrap());

    let claim_rewards_msg = ExecuteMsg::ClaimRewards {
        withdraw_lp_shares: None,
    };

    // mint ASTRO to Lockdrop Contract
    mint_some_astro(
        &mut app,
        Addr::unchecked(auction_init_msg.owner.clone().unwrap()),
        astro_token_instance.clone(),
        Uint128::new(100_000_000_000),
        auction_init_msg.lockdrop_contract_address.to_string(),
    );

    // mint ASTRO to owner
    mint_some_astro(
        &mut app,
        Addr::unchecked(auction_init_msg.owner.clone().unwrap()),
        astro_token_instance.clone(),
        Uint128::new(100_000_000_000),
        owner.to_string(),
    );

    let (user1_address, user2_address, user3_address) = make_astro_ust_deposits(
        &mut app,
        auction_instance.clone(),
        auction_init_msg.clone(),
        astro_token_instance.clone(),
    );

    // ######    Initialize generator and vesting instance   ######
    let (generator_instance, _) = instantiate_generator_and_vesting(
        &mut app,
        Addr::unchecked(auction_init_msg.owner.clone().unwrap()),
        astro_token_instance.clone(),
        lp_token_instance.clone(),
    );

    let update_msg = UpdateConfigMsg {
        owner: None,
        astro_ust_pair_address: None,
        generator_contract: Some(generator_instance.to_string()),
    };

    app.execute_contract(
        Addr::unchecked(auction_init_msg.owner.clone().unwrap()),
        auction_instance.clone(),
        &ExecuteMsg::UpdateConfig {
            new_config: update_msg.clone(),
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        owner.clone(),
        astro_token_instance,
        &Cw20ExecuteMsg::Send {
            amount: Uint128::new(100_000_000000),
            contract: auction_instance.to_string(),
            msg: to_binary(&Cw20HookMsg::IncreaseAstroIncentives {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    // ######    ERROR :: Deposit/withdrawal windows are open   ######

    let mut err = app
        .execute_contract(
            owner,
            auction_instance.clone(),
            &ExecuteMsg::InitPool { slippage: None },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Deposit/withdrawal windows are still open"
    );

    // Astro/USD should be provided to the pool

    err = app
        .execute_contract(
            user1_address.clone(),
            auction_instance.clone(),
            &claim_rewards_msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Astro/USD should be provided to the pool!"
    );

    // finish deposit / withdraw period
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10611001)
    });

    // ######    ERROR :: Invalid request   ######

    err = app
        .execute_contract(
            Addr::unchecked("not_user".to_string()),
            auction_instance.clone(),
            &claim_rewards_msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "astroport_auction::state::UserInfo not found"
    );

    // ######    Sucess :: Initialize ASTRO-UST Pool   ######

    app.execute_contract(
        Addr::unchecked(auction_init_msg.owner.clone().unwrap()),
        auction_instance.clone(),
        &ExecuteMsg::InitPool { slippage: None },
        &[],
    )
    .unwrap();

    // ######    SUCCESS :: Stake successfully   ######

    app.execute_contract(
        Addr::unchecked(auction_init_msg.owner.unwrap()),
        auction_instance.clone(),
        &ExecuteMsg::StakeLpTokens {},
        &[],
    )
    .unwrap();

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10911001)
    });

    // ######    SUCCESS :: Successfully claim staking rewards for User-1 ######

    // Auction :: Check user-1 state (before claim)
    let user1info_before_claim: astroport_periphery::auction::UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &astroport_periphery::auction::QueryMsg::UserInfo {
                address: user1_address.to_string(),
            },
        )
        .unwrap();

    // Auction :: Claim rewards for the user
    app.execute_contract(
        user1_address.clone(),
        auction_instance.clone(),
        &claim_rewards_msg,
        &[],
    )
    .unwrap();

    // Auction :: Check user-1 state (After Claim)
    let user1info_after_claim: astroport_periphery::auction::UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &astroport_periphery::auction::QueryMsg::UserInfo {
                address: user1_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        user1info_before_claim.claimed_lp_shares,
        user1info_after_claim.claimed_lp_shares
    );
    assert_eq!(
        user1info_before_claim.withdrawable_lp_shares,
        user1info_after_claim.withdrawable_lp_shares
    );

    // ######    SUCCESS :: Successfully claim staking rewards for User-2 ######

    // Auction :: Check user-2 state (before claim)
    let user2info_before_claim: astroport_periphery::auction::UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &astroport_periphery::auction::QueryMsg::UserInfo {
                address: user2_address.to_string(),
            },
        )
        .unwrap();

    // Auction :: Claim rewards for the user 2
    app.execute_contract(
        user2_address.clone(),
        auction_instance.clone(),
        &claim_rewards_msg,
        &[],
    )
    .unwrap();

    // Auction :: Check user-2 state (After Claim)
    let user2info_after_claim: astroport_periphery::auction::UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &astroport_periphery::auction::QueryMsg::UserInfo {
                address: user2_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        user2info_before_claim.claimed_lp_shares,
        user2info_after_claim.claimed_lp_shares
    );
    assert_eq!(
        user2info_before_claim.withdrawable_lp_shares,
        user2info_after_claim.withdrawable_lp_shares
    );

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10991001)
    });

    // ######    SUCCESS :: Successfully claim staking rewards for User-3 ######

    // Auction :: Check user-3 state (before claim)
    let user3info_before_claim: astroport_periphery::auction::UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &astroport_periphery::auction::QueryMsg::UserInfo {
                address: user3_address.to_string(),
            },
        )
        .unwrap();

    // Auction :: Claim rewards for the user 3
    app.execute_contract(
        user3_address.clone(),
        auction_instance.clone(),
        &claim_rewards_msg,
        &[],
    )
    .unwrap();

    // Auction :: Check user-3 state (After Claim)
    let user3info_after_claim: astroport_periphery::auction::UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &astroport_periphery::auction::QueryMsg::UserInfo {
                address: user3_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        user3info_before_claim.claimed_lp_shares,
        user3info_after_claim.claimed_lp_shares
    );
    assert_eq!(
        user3info_before_claim.withdrawable_lp_shares,
        user3info_after_claim.withdrawable_lp_shares
    );

    // ######    SUCCESS :: Successfully again claim staking rewards for User-1 ######

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10992001)
    });

    // Auction :: Check user-1 state (before claim)
    let user1info_before_claim2: astroport_periphery::auction::UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &astroport_periphery::auction::QueryMsg::UserInfo {
                address: user1_address.to_string(),
            },
        )
        .unwrap();

    // Auction :: Claim rewards for the user
    app.execute_contract(
        user1_address.clone(),
        auction_instance.clone(),
        &claim_rewards_msg,
        &[],
    )
    .unwrap();

    // Auction :: Check user-1 state (After Claim)
    let user1info_after_claim2: astroport_periphery::auction::UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &astroport_periphery::auction::QueryMsg::UserInfo {
                address: user1_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        user1info_before_claim2.claimed_lp_shares,
        user1info_after_claim2.claimed_lp_shares
    );
    assert_eq!(
        user1info_before_claim2.withdrawable_lp_shares,
        user1info_after_claim2.withdrawable_lp_shares
    );
}

#[test]
fn test_withdraw_unlocked_lp_shares() {
    let mut app = mock_app();
    let (auction_instance, astro_token_instance, _, _, _, lp_token_instance, auction_init_msg) =
        init_all_contracts(&mut app);

    let owner = Addr::unchecked(auction_init_msg.owner.clone().unwrap());

    let withdraw_lp_msg = ExecuteMsg::ClaimRewards {
        withdraw_lp_shares: Some(Uint128::new(1)),
    };

    // mint ASTRO to Lockdrop Contract
    mint_some_astro(
        &mut app,
        Addr::unchecked(auction_init_msg.owner.clone().unwrap()),
        astro_token_instance.clone(),
        Uint128::new(100_000_000_000),
        auction_init_msg.lockdrop_contract_address.to_string(),
    );

    // mint ASTRO to Auction Contract
    mint_some_astro(
        &mut app,
        Addr::unchecked(auction_init_msg.owner.clone().unwrap()),
        astro_token_instance.clone(),
        Uint128::new(100_000_000_000),
        auction_instance.to_string(),
    );

    let (user1_address, user2_address, user3_address) = make_astro_ust_deposits(
        &mut app,
        auction_instance.clone(),
        auction_init_msg.clone(),
        astro_token_instance.clone(),
    );

    // ######    Initialize generator and vesting instance   ######
    let (generator_instance, _) = instantiate_generator_and_vesting(
        &mut app,
        Addr::unchecked(auction_init_msg.owner.clone().unwrap()),
        astro_token_instance.clone(),
        lp_token_instance.clone(),
    );

    let update_msg = UpdateConfigMsg {
        owner: None,
        astro_ust_pair_address: None,
        generator_contract: Some(generator_instance.to_string()),
    };

    app.execute_contract(
        Addr::unchecked(auction_init_msg.owner.clone().unwrap()),
        auction_instance.clone(),
        &ExecuteMsg::UpdateConfig {
            new_config: update_msg.clone(),
        },
        &[],
    )
    .unwrap();

    // ######    ERROR :: Deposit/withdrawal windows are open   ######

    let mut err = app
        .execute_contract(
            user1_address.clone(),
            auction_instance.clone(),
            &withdraw_lp_msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Astro/USD should be provided to the pool!"
    );

    // finish deposit / withdraw period
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10611001)
    });

    // ######    ERROR :: Invalid request. No LP Tokens to claim   ######

    err = app
        .execute_contract(
            Addr::unchecked("not_user".to_string()),
            auction_instance.clone(),
            &withdraw_lp_msg,
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "astroport_auction::state::UserInfo not found"
    );

    // ######    Sucess :: Initialize ASTRO-UST Pool   ######

    app.execute_contract(
        owner.clone(),
        astro_token_instance,
        &Cw20ExecuteMsg::Send {
            amount: Uint128::new(100_000_000000),
            contract: auction_instance.to_string(),
            msg: to_binary(&Cw20HookMsg::IncreaseAstroIncentives {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked(auction_init_msg.owner.clone().unwrap()),
        auction_instance.clone(),
        &ExecuteMsg::InitPool { slippage: None },
        &[],
    )
    .unwrap();

    // ######    SUCCESS :: Stake successfully   ######

    app.execute_contract(
        Addr::unchecked(auction_init_msg.owner.unwrap()),
        auction_instance.clone(),
        &ExecuteMsg::StakeLpTokens {},
        &[],
    )
    .unwrap();

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10911001)
    });

    // ######    SUCCESS :: Successfully withdraw LP shares (which also claims rewards) for User-1 ######

    // Auction :: Check user-1 state (before claim)
    let user1info_before_claim: astroport_periphery::auction::UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &astroport_periphery::auction::QueryMsg::UserInfo {
                address: user1_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(0u64),
        user1info_before_claim.claimed_lp_shares
    );

    // Auction :: Withdraw unvested LP shares for the user
    app.execute_contract(
        user1_address.clone(),
        auction_instance.clone(),
        &ExecuteMsg::ClaimRewards {
            withdraw_lp_shares: Some(user1info_before_claim.withdrawable_lp_shares.unwrap()),
        },
        &[],
    )
    .unwrap();

    // Auction :: Check user-1 state (After Claim)
    let user1info_after_claim: astroport_periphery::auction::UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &astroport_periphery::auction::QueryMsg::UserInfo {
                address: user1_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        user1info_before_claim.withdrawable_lp_shares.unwrap(),
        user1info_after_claim.claimed_lp_shares
    );
    assert_eq!(
        Uint128::from(0u64),
        user1info_after_claim.withdrawable_lp_shares.unwrap()
    );

    // ######    SUCCESS :: Successfully withdraw LP shares (which also claims rewards) for User-2 ######

    // Auction :: Check user-2 state (before claim)
    let user2info_before_claim: astroport_periphery::auction::UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &astroport_periphery::auction::QueryMsg::UserInfo {
                address: user2_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(0u64),
        user2info_before_claim.claimed_lp_shares
    );

    // Auction :: Withdraw unvested LP shares for the user
    app.execute_contract(
        user2_address.clone(),
        auction_instance.clone(),
        &ExecuteMsg::ClaimRewards {
            withdraw_lp_shares: Some(user2info_before_claim.withdrawable_lp_shares.unwrap()),
        },
        &[],
    )
    .unwrap();

    // Auction :: Check user-2 state (After Claim)
    let user2info_after_claim: astroport_periphery::auction::UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &astroport_periphery::auction::QueryMsg::UserInfo {
                address: user2_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        user2info_before_claim.withdrawable_lp_shares.unwrap(),
        user2info_after_claim.claimed_lp_shares
    );
    assert_eq!(
        Uint128::from(0u64),
        user2info_after_claim.withdrawable_lp_shares.unwrap()
    );

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10991001)
    });

    // ######    SUCCESS :: Successfully withdraw LP shares (which also claims rewards) for User-3 ######

    // Auction :: Check user-3 state (before claim)
    let user3info_before_claim: astroport_periphery::auction::UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &astroport_periphery::auction::QueryMsg::UserInfo {
                address: user3_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        Uint128::from(0u64),
        user3info_before_claim.claimed_lp_shares
    );

    // Auction :: Withdraw unvested LP shares for the user
    app.execute_contract(
        user3_address.clone(),
        auction_instance.clone(),
        &ExecuteMsg::ClaimRewards {
            withdraw_lp_shares: Some(user3info_before_claim.withdrawable_lp_shares.unwrap()),
        },
        &[],
    )
    .unwrap();

    // Auction :: Check user-3 state (After Claim)
    let user3info_after_claim: astroport_periphery::auction::UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &astroport_periphery::auction::QueryMsg::UserInfo {
                address: user3_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        user3info_before_claim.withdrawable_lp_shares.unwrap(),
        user3info_after_claim.claimed_lp_shares
    );
    assert_eq!(
        Some(Uint128::zero()),
        user3info_after_claim.withdrawable_lp_shares
    );

    // ######    SUCCESS :: Successfully again withdraw LP shares (which also claims rewards) for User-1 ######

    // Auction :: Check user-1 state (before claim)
    let user1info_before_claim2: astroport_periphery::auction::UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &astroport_periphery::auction::QueryMsg::UserInfo {
                address: user1_address.to_string(),
            },
        )
        .unwrap();

    // Auction :: Withdraw LP for the user
    app.execute_contract(
        user1_address.clone(),
        auction_instance.clone(),
        &ExecuteMsg::ClaimRewards {
            withdraw_lp_shares: Some(user1info_before_claim2.withdrawable_lp_shares.unwrap()),
        },
        &[],
    )
    .unwrap();

    // Auction :: Check user-1 state (After Claim)
    let user1info_after_claim2: astroport_periphery::auction::UserInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &auction_instance,
            &astroport_periphery::auction::QueryMsg::UserInfo {
                address: user1_address.to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        user1info_before_claim2.claimed_lp_shares
            + user1info_before_claim2.withdrawable_lp_shares.unwrap(),
        user1info_after_claim2.claimed_lp_shares
    );
    assert_eq!(
        Some(Uint128::zero()),
        user1info_after_claim2.withdrawable_lp_shares
    );
}
