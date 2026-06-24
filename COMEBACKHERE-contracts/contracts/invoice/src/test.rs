#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env};

use crate::{InvoiceContract, InvoiceContractClient, InvoiceStatus};

fn setup_test() -> (Env, Address, Address, InvoiceContractClient) {
    let env = Env::default();
    let admin = Address::generate(&env);
    let merchant = Address::generate(&env);

    let contract_id = env.register_contract(None, InvoiceContract);
    let client = InvoiceContractClient::new(&env, &contract_id);

    client.initialize(&admin);
    env.mock_all_auths();

    (env, admin, merchant, client)
}

fn create_expired_invoice(
    env: &Env,
    client: &InvoiceContractClient,
    merchant: &Address,
) -> u32 {
    let now = env.ledger().timestamp();
    let expires_at = now - 100;
    client.create_invoice(merchant, merchant, 100_i128, 120_i128, expires_at)
}

fn create_active_invoice(
    env: &Env,
    client: &InvoiceContractClient,
    merchant: &Address,
) -> u32 {
    let now = env.ledger().timestamp();
    let expires_at = now + 100000;
    client.create_invoice(merchant, merchant, 100_i128, 120_i128, expires_at)
}

#[test]
fn test_batch_expire_empty_range() {
    let (env, _admin, merchant, client) = setup_test();
    create_expired_invoice(&env, &client, &merchant);

    let expired = client.batch_expire(&0, &0);
    assert_eq!(
        expired, 0,
        "empty range should expire 0 invoices"
    );
}

#[test]
fn test_batch_expire_partial_batch() {
    let (env, _admin, merchant, client) = setup_test();

    for _ in 0..10 {
        create_expired_invoice(&env, &client, &merchant);
    }

    let expired = client.batch_expire(&0, &3);
    assert_eq!(
        expired, 3,
        "should expire 3 of 10 invoices in the first batch"
    );

    let remaining = client.batch_expire(&3, &3);
    assert_eq!(
        remaining, 3,
        "should expire 3 more invoices in the second batch"
    );
}

#[test]
fn test_batch_expire_full_batch() {
    let (env, _admin, merchant, client) = setup_test();

    for _ in 0..5 {
        create_expired_invoice(&env, &client, &merchant);
    }

    let expired = client.batch_expire(&0, &5);
    assert_eq!(
        expired, 5,
        "should expire all 5 invoices in a single batch"
    );

    let more = client.batch_expire(&0, &5);
    assert_eq!(
        more, 0,
        "no more invoices to expire after full batch"
    );
}

#[test]
fn test_batch_expire_offset_beyond_range() {
    let (env, _admin, merchant, client) = setup_test();

    for _ in 0..3 {
        create_expired_invoice(&env, &client, &merchant);
    }

    let expired = client.batch_expire(&10, &5);
    assert_eq!(
        expired, 0,
        "offset beyond total count should expire 0"
    );
}

#[test]
fn test_batch_expire_mixed_status() {
    let (env, _admin, merchant, client) = setup_test();

    let id1 = create_expired_invoice(&env, &client, &merchant);
    let _id2 = create_active_invoice(&env, &client, &merchant);
    let id3 = create_expired_invoice(&env, &client, &merchant);

    let expired = client.batch_expire(&0, &10);
    assert_eq!(expired, 2, "should expire only expired invoices");

    let status1 = client.get_invoice_status(&id1);
    assert_eq!(status1, InvoiceStatus::Expired);

    let status3 = client.get_invoice_status(&id3);
    assert_eq!(status3, InvoiceStatus::Expired);
}

#[test]
fn test_batch_expire_limit_ceiling() {
    let (env, _admin, merchant, client) = setup_test();

    for _ in 0..60 {
        create_expired_invoice(&env, &client, &merchant);
    }

    let expired = client.batch_expire(&0, &60);
    assert_eq!(
        expired, 50,
        "should cap at DEFAULT_BATCH_LIMIT of 50"
    );
}
