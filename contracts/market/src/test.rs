#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events},
    token::{StellarAssetClient, TokenClient},
    vec, Address, Env, IntoVal, Map, Symbol, Val,
};

fn create_token<'a>(env: &Env, admin: &Address) -> (TokenClient<'a>, StellarAssetClient<'a>) {
    let contract_address = env.register_stellar_asset_contract_v2(admin.clone());
    (
        TokenClient::new(env, &contract_address.address()),
        StellarAssetClient::new(env, &contract_address.address()),
    )
}

#[test]
fn test_create_job_transfers_funds_and_returns_id() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MarketContract, ());
    let client = MarketContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);

    // Mint tokens to finder
    token_admin_client.mint(&finder, &1000);
    assert_eq!(token_client.balance(&finder), 1000);

    // Create job — funds move, id starts at 1
    let job_id = client.create_job(&finder, &token_client.address, &500);
    assert_eq!(job_id, 1);
    assert_eq!(token_client.balance(&finder), 500);
    assert_eq!(token_client.balance(&contract_id), 500);

    // Second job increments counter
    let job_id_2 = client.create_job(&finder, &token_client.address, &200);
    assert_eq!(job_id_2, 2);
    assert_eq!(token_client.balance(&finder), 300);
    assert_eq!(token_client.balance(&contract_id), 700);
}

#[test]
fn test_create_job_emits_event() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MarketContract, ());
    let client = MarketContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &1000);

    client.create_job(&finder, &token_client.address, &500);

    // events().all() includes the token transfer event + our JobCreated event
    let all_events = env.events().all();
    assert_eq!(all_events.len(), 2);

    // The last event is our JobCreated event from the market contract
    // Wrap both sides in a soroban Vec so comparison uses host-level equality
    let expected_topics: soroban_sdk::Vec<Val> =
        vec![&env, Symbol::new(&env, "job_created").into_val(&env)];
    let expected_data: Val = Map::<Symbol, Val>::from_array(
        &env,
        [
            (Symbol::new(&env, "amount"), 500i128.into_val(&env)),
            (Symbol::new(&env, "id"), 1u64.into_val(&env)),
        ],
    )
    .into_val(&env);

    let expected: soroban_sdk::Vec<(Address, soroban_sdk::Vec<Val>, Val)> = vec![
        &env,
        (contract_id.clone(), expected_topics, expected_data),
    ];
    assert_eq!(vec![&env, all_events.get(1).unwrap()], expected);
}

#[test]
fn test_create_job_incremental_ids() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MarketContract, ());
    let client = MarketContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let finder = Address::generate(&env);
    let (token_client, token_admin_client) = create_token(&env, &admin);
    token_admin_client.mint(&finder, &3000);

    assert_eq!(client.create_job(&finder, &token_client.address, &100), 1);
    assert_eq!(client.create_job(&finder, &token_client.address, &100), 2);
    assert_eq!(client.create_job(&finder, &token_client.address, &100), 3);
}
