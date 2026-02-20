#![no_std]
use soroban_sdk::{contract, contractevent, contractimpl, contracttype, token, Address, Env};

#[contracttype]
pub enum DataKey {
    JobCounter,
    Job(u64),
}

#[contracttype]
#[derive(Clone, PartialEq)]
pub enum JobStatus {
    Open,
}

#[contracttype]
#[derive(Clone)]
pub struct Job {
    pub id: u64,
    pub finder: Address,
    pub token: Address,
    pub amount: i128,
    pub status: JobStatus,
}

#[contractevent]
pub struct JobCreated {
    pub id: u64,
    pub amount: i128,
}

#[contract]
pub struct MarketContract;

#[contractimpl]
impl MarketContract {
    pub fn create_job(env: Env, finder: Address, token: Address, amount: i128) -> u64 {
        // 1. Require auth from the finder
        finder.require_auth();

        // 2. Transfer token from finder to this contract
        let token_client = token::TokenClient::new(&env, &token);
        token_client.transfer(&finder, &env.current_contract_address(), &amount);

        // 3. Get and increment job counter
        let counter: u64 = env
            .storage()
            .instance()
            .get(&DataKey::JobCounter)
            .unwrap_or(0);
        let id = counter + 1;
        env.storage().instance().set(&DataKey::JobCounter, &id);

        // 4. Store job in persistent storage
        let job = Job {
            id,
            finder,
            token,
            amount,
            status: JobStatus::Open,
        };
        env.storage().persistent().set(&DataKey::Job(id), &job);

        // 5. Emit JobCreated event
        JobCreated { id, amount }.publish(&env);

        // 6. Return job id
        id
    }
}

mod test;
