#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, panic_with_error, Address, Env, Symbol};

const DEFAULT_BATCH_LIMIT: u32 = 50;

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum InvoiceStatus {
    Pending,
    Paid,
    Expired,
    Cancelled,
    RefundRequested,
    Released,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct Invoice {
    pub id: u32,
    pub merchant: Address,
    pub payer: Address,
    pub amount_usdc: i128,
    pub gross_usdc: i128,
    pub expires_at: u64,
    pub status: InvoiceStatus,
    pub paid_at: u64,
}

#[derive(Clone, Debug)]
#[contracttype]
pub enum InvoiceError {
    Unauthorized,
    ContractPaused,
    InvalidAmount,
    NotPending,
    Expired,
    NotFound,
    AlreadyInitialized,
    ZeroDuration,
    ExpiryOverflow,
    NotPaid,
}

const ADMIN_KEY: Symbol = Symbol::short("admin");
const PAUSED_KEY: Symbol = Symbol::short("pause");
const INIT_KEY: Symbol = Symbol::short("init");
const NEXT_ID_KEY: Symbol = Symbol::short("nid");

#[contract]
pub struct InvoiceContract;

#[contractimpl]
impl InvoiceContract {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&INIT_KEY) {
            panic_with_error!(&env, InvoiceError::AlreadyInitialized);
        }
        env.storage().instance().set(&ADMIN_KEY, &admin);
        env.storage().instance().set(&PAUSED_KEY, &false);
        env.storage().instance().set(&INIT_KEY, &true);
    }

    pub fn is_paused(env: Env) -> bool {
        env.storage().instance().get(&PAUSED_KEY).unwrap_or(false)
    }

    fn require_not_paused(env: &Env) {
        if env.storage().instance().get(&PAUSED_KEY).unwrap_or(false) {
            panic_with_error!(env, InvoiceError::ContractPaused);
        }
    }

    fn require_admin(env: &Env, caller: &Address) {
        let admin: Address = env.storage().instance().get(&ADMIN_KEY).unwrap();
        if caller != &admin {
            panic_with_error!(env, InvoiceError::Unauthorized);
        }
    }

    pub fn create_invoice(
        env: Env,
        caller: Address,
        merchant: Address,
        amount_usdc: i128,
        gross_usdc: i128,
        expires_at: u64,
    ) -> u32 {
        Self::require_not_paused(&env);
        caller.require_auth();

        if amount_usdc <= 0 || gross_usdc < amount_usdc {
            panic_with_error!(&env, InvoiceError::InvalidAmount);
        }

        let id: u32 = env.storage().instance().get(&NEXT_ID_KEY).unwrap_or(1);
        env.storage().instance().set(&NEXT_ID_KEY, &(id + 1));

        let zero_addr = Address::from_string(&soroban_sdk::String::from_str(
            &env,
            "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
        ));

        let invoice = Invoice {
            id,
            merchant,
            payer: zero_addr,
            amount_usdc,
            gross_usdc,
            expires_at,
            status: InvoiceStatus::Pending,
            paid_at: 0,
        };

        env.storage().persistent().set(&id, &invoice);
        env.storage().persistent().extend_ttl(&id, 5000, 5000);

        env.events().publish(("invoice_created", id), &invoice);
        id
    }

    pub fn get_invoice(env: Env, id: u32) -> Invoice {
        env.storage()
            .persistent()
            .get(&id)
            .unwrap_or_else(|| panic_with_error!(&env, InvoiceError::NotFound))
    }

    pub fn get_invoice_status(env: Env, id: u32) -> InvoiceStatus {
        let invoice = Self::get_invoice(env, id);
        invoice.status
    }

    pub fn mark_paid(env: Env, caller: Address, id: u32) {
        Self::require_not_paused(&env);
        caller.require_auth();

        let mut invoice: Invoice = env
            .storage()
            .persistent()
            .get(&id)
            .unwrap_or_else(|| panic_with_error!(&env, InvoiceError::NotFound));

        if invoice.status != InvoiceStatus::Pending {
            panic_with_error!(&env, InvoiceError::NotPending);
        }

        if env.ledger().timestamp() >= invoice.expires_at {
            panic_with_error!(&env, InvoiceError::Expired);
        }

        invoice.status = InvoiceStatus::Paid;
        invoice.payer = caller.clone();
        invoice.paid_at = env.ledger().timestamp();

        env.storage().persistent().set(&id, &invoice);
        env.events().publish(("invoice_paid", id, caller), ());
    }

    pub fn batch_expire(env: Env, offset: u32, limit: u32) -> u32 {
        if limit > DEFAULT_BATCH_LIMIT {
            panic_with_error!(&env, InvoiceError::InvalidAmount);
        }

        let total: u32 = env.storage().instance().get(&NEXT_ID_KEY).unwrap_or(1);
        let end = (offset + limit).min(total);
        let mut expired_count: u32 = 0;

        for id in offset..end {
            if let Some(mut invoice) = env.storage().persistent().get(&id) {
                if invoice.status == InvoiceStatus::Pending
                    && env.ledger().timestamp() >= invoice.expires_at
                {
                    invoice.status = InvoiceStatus::Expired;
                    env.storage().persistent().set(&id, &invoice);
                    expired_count += 1;
                    env.events().publish(("invoice_expired", id), ());
                }
            }
        }

        expired_count
    }

    pub fn request_refund(env: Env, caller: Address, id: u32) {
        Self::require_not_paused(&env);
        caller.require_auth();

        let mut invoice: Invoice = env
            .storage()
            .persistent()
            .get(&id)
            .unwrap_or_else(|| panic_with_error!(&env, InvoiceError::NotFound));

        if invoice.status != InvoiceStatus::Paid {
            panic_with_error!(&env, InvoiceError::NotPaid);
        }

        if caller != invoice.payer {
            panic_with_error!(&env, InvoiceError::Unauthorized);
        }

        invoice.status = InvoiceStatus::RefundRequested;
        env.storage().persistent().set(&id, &invoice);
        env.events().publish(("invoice_refund_req", id, caller), ());
    }

    pub fn cancel_invoice(env: Env, caller: Address, id: u32) {
        Self::require_not_paused(&env);
        caller.require_auth();

        let mut invoice: Invoice = env
            .storage()
            .persistent()
            .get(&id)
            .unwrap_or_else(|| panic_with_error!(&env, InvoiceError::NotFound));

        if invoice.merchant != caller {
            panic_with_error!(&env, InvoiceError::Unauthorized);
        }

        if invoice.status != InvoiceStatus::Pending {
            panic_with_error!(&env, InvoiceError::NotPending);
        }

        invoice.status = InvoiceStatus::Cancelled;
        env.storage().persistent().set(&id, &invoice);
        env.events().publish(("invoice_cancelled", id), ());
    }

    pub fn release_escrow(env: Env, caller: Address, id: u32) {
        Self::require_not_paused(&env);
        caller.require_auth();

        let mut invoice: Invoice = env
            .storage()
            .persistent()
            .get(&id)
            .unwrap_or_else(|| panic_with_error!(&env, InvoiceError::NotFound));

        if invoice.status != InvoiceStatus::Paid
            && invoice.status != InvoiceStatus::RefundRequested
        {
            panic_with_error!(&env, InvoiceError::NotPaid);
        }

        invoice.status = InvoiceStatus::Released;
        env.storage().persistent().set(&id, &invoice);
        env.events().publish(("escrow_released", id), ());
    }

    pub fn pause(env: Env, caller: Address) {
        caller.require_auth();
        Self::require_admin(&env, &caller);
        env.storage().instance().set(&PAUSED_KEY, &true);
        env.events().publish(("contract_paused",), ());
    }

    pub fn unpause(env: Env, caller: Address) {
        caller.require_auth();
        Self::require_admin(&env, &caller);
        env.storage().instance().set(&PAUSED_KEY, &false);
        env.events().publish(("contract_unpaused",), ());
    }
}

#[cfg(test)]
mod test;
