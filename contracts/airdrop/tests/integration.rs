use astroport_periphery::airdrop::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg, StateResponse,
};
use cosmwasm_std::testing::{mock_env, MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{Addr, Uint128};
use terra_multi_test::{App, BankKeeper, ContractWrapper, Executor, TerraMockQuerier};

fn mock_app() -> App {
    let api = MockApi::default();
    let env = mock_env();
    let bank = BankKeeper::new();
    let storage = MockStorage::new();
    let tmq = TerraMockQuerier::new(MockQuerier::new(&[]));

    App::new(api, env.block, bank, storage, tmq)
}

fn init_contracts(app: &mut App, aidrop_instantiate_msg: &InstantiateMsg) -> Addr {
    let owner = Addr::unchecked("owner_address");

    let airdrop_contract = Box::new(ContractWrapper::new(
        astro_airdrop::contract::execute,
        astro_airdrop::contract::instantiate,
        astro_airdrop::contract::query,
    ));

    let airdrop_code_id = app.store_code(airdrop_contract);

    // Init contract
    let airdrop_instance = app
        .instantiate_contract(
            airdrop_code_id,
            owner.clone(),
            aidrop_instantiate_msg,
            &[],
            "airdrop",
            None,
        )
        .unwrap();

    airdrop_instance
}

fn airdrop_init_msg() -> InstantiateMsg {
    // Config with valid base params
    InstantiateMsg {
        owner: Some(String::from("contract_owner")),
        astro_token_address: String::from("astro_token_contract"),
        terra_merkle_roots: Some(vec!["terra_merkle_roots".to_string()]),
        evm_merkle_roots: Some(vec!["evm_merkle_roots".to_string()]),
        from_timestamp: Some(1_000_000_000),
        to_timestamp: 100_000_000_000,
        boostrap_auction_address: String::from("boostrap_auction_address"),
        total_airdrop_size: Uint128::new(100_000_000_000),
    }
}

#[test]
fn proper_initialization() {
    let mut app = mock_app();

    let init_msg = airdrop_init_msg();
    let airdrop_instance = init_contracts(&mut app, &init_msg);

    let resp: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&airdrop_instance, &QueryMsg::Config {})
        .unwrap();

    // Check config
    assert_eq!(init_msg.astro_token_address, resp.astro_token_address);
    assert_eq!(
        init_msg.boostrap_auction_address,
        resp.boostrap_auction_address
    );
    assert_eq!(init_msg.owner.unwrap(), resp.owner);
    assert_eq!(
        init_msg.terra_merkle_roots.unwrap(),
        resp.terra_merkle_roots
    );
    assert_eq!(init_msg.evm_merkle_roots.unwrap(), resp.evm_merkle_roots);
    assert_eq!(init_msg.from_timestamp.unwrap(), resp.from_timestamp);
    assert_eq!(init_msg.to_timestamp, resp.to_timestamp);

    // Check state
    let resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&airdrop_instance, &QueryMsg::State {})
        .unwrap();

    assert_eq!(init_msg.total_airdrop_size, resp.total_airdrop_size);
    assert_eq!(init_msg.total_airdrop_size, resp.unclaimed_tokens);
    assert_eq!(Uint128::zero(), resp.total_delegated_amount);
}

#[test]
fn update_config() {
    let mut app = mock_app();

    let init_msg = airdrop_init_msg();
    let airdrop_instance = init_contracts(&mut app, &init_msg);

    // Only owner can update
    let err = app
        .execute_contract(
            Addr::unchecked("wrong_owner"),
            airdrop_instance.clone(),
            &ExecuteMsg::UpdateConfig {
                owner: None,
                terra_merkle_roots: None,
                evm_merkle_roots: None,
                from_timestamp: None,
                to_timestamp: None,
            },
            &[],
        )
        .unwrap_err();

    assert_eq!(
        err.to_string(),
        "Generic error: Only owner can update configuration"
    );

    let new_owner = String::from("new_owner");
    let terra_merkle_roots = vec!["new_terra_merkle_roots".to_string()];
    let evm_merkle_roots = vec!["new_evm_merkle_roots".to_string()];
    let from_timestamp = 2_000_000_000;
    let to_timestamp = 200_000_000_000;

    let update_msg = ExecuteMsg::UpdateConfig {
        owner: Some(new_owner.clone()),
        terra_merkle_roots: Some(terra_merkle_roots.clone()),
        evm_merkle_roots: Some(evm_merkle_roots.clone()),
        from_timestamp: Some(from_timestamp),
        to_timestamp: Some(to_timestamp),
    };

    // should be a success
    app.execute_contract(
        Addr::unchecked(init_msg.owner.unwrap()),
        airdrop_instance.clone(),
        &update_msg,
        &[],
    )
    .unwrap();

    let resp: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&airdrop_instance, &QueryMsg::Config {})
        .unwrap();

    // Check config and make sure all fields are updated
    assert_eq!(new_owner, resp.owner);
    assert_eq!(terra_merkle_roots, resp.terra_merkle_roots);
    assert_eq!(evm_merkle_roots, resp.evm_merkle_roots);
    assert_eq!(from_timestamp, resp.from_timestamp);
    assert_eq!(to_timestamp, resp.to_timestamp);
}
