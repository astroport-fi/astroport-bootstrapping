use astroport_periphery::airdrop::{ConfigResponse, InstantiateMsg, QueryMsg};
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

#[test]
fn proper_initialization() {
    let mut app = mock_app();

    let owner = Addr::unchecked("owner");

    let airdrop_contract = Box::new(ContractWrapper::new(
        astro_airdrop::contract::execute,
        astro_airdrop::contract::instantiate,
        astro_airdrop::contract::query,
    ));

    let airdrop_code_id = app.store_code(airdrop_contract);

    let terra_merkle_roots = vec!["terra_merkle_roots".to_string()];
    let evm_merkle_roots = vec!["evm_merkle_roots".to_string()];
    let till_timestamp = 1_000_000_00000;
    let from_timestamp = 1_000_000_000;

    // Config with valid base params
    let msg = InstantiateMsg {
        owner: Some(owner.clone()),
        astro_token_address: Some(Addr::unchecked("ASTRO")),
        terra_merkle_roots: Some(terra_merkle_roots.clone()),
        evm_merkle_roots: Some(evm_merkle_roots.clone()),
        from_timestamp: Some(from_timestamp),
        till_timestamp: Some(till_timestamp),
        boostrap_auction_address: Some(Addr::unchecked("AUCTION")),
        total_airdrop_size: Uint128::new(100_000_000_000),
    };

    let airdrop_instance = app
        .instantiate_contract(airdrop_code_id, owner.clone(), &msg, &[], "airdrop", None)
        .unwrap();

    let msg = QueryMsg::Config {};
    let config_res: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&airdrop_instance, &msg)
        .unwrap();

    assert_eq!(config_res.owner, owner)
}
