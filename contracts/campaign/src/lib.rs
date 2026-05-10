#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, Address, Env, String, Symbol, Vec,
};

// ── Storage keys ────────────────────────────────────────────────────────────

const ADMIN: Symbol = symbol_short!("ADMIN");
const FEE_BPS: Symbol = symbol_short!("FEE_BPS");
const CAMP_CNT: Symbol = symbol_short!("CAMP_CNT");

fn campaign_key(id: u64) -> (Symbol, u64) {
    (symbol_short!("CAMP"), id)
}

fn donation_key(campaign_id: u64, donor: &Address) -> (Symbol, u64, Address) {
    (symbol_short!("DON"), campaign_id, donor.clone())
}

// ── Types ────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, PartialEq)]
pub enum CampaignStatus {
    Pending,
    Active,
    Successful,
    Failed,
    Cancelled,
}

#[contracttype]
#[derive(Clone, PartialEq)]
pub enum CampaignType {
    Open,
    Curated,
}

#[contracttype]
#[derive(Clone)]
pub struct Campaign {
    pub id: u64,
    pub creator: Address,
    pub token: Address,
    pub goal: i128,
    pub raised: i128,
    pub title: String,
    pub description: String,
    pub category: String,
    pub campaign_type: CampaignType,
    pub deadline_ledger: u32,
    pub status: CampaignStatus,
}

#[contracttype]
#[derive(Clone)]
pub struct DonationRecord {
    pub campaign_id: u64,
    pub donor: Address,
    pub amount: i128,
    pub refunded: bool,
    pub ledger: u32,
}

// ── Contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct CampaignContract;

#[contractimpl]
impl CampaignContract {
    /// One-time initialisation. Sets admin and platform fee in basis points.
    pub fn initialize(env: Env, admin: Address, fee_bps: u32) {
        if env.storage().instance().has(&ADMIN) {
            panic!("already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&FEE_BPS, &fee_bps);
        env.storage().instance().set(&CAMP_CNT, &0u64);
    }

    /// Create a new campaign. Open campaigns become Active immediately;
    /// Curated campaigns start as Pending until admin approves.
    pub fn create_campaign(
        env: Env,
        creator: Address,
        token: Address,
        goal: i128,
        title: String,
        description: String,
        category: String,
        campaign_type: CampaignType,
        deadline_ledger: u32,
    ) -> u64 {
        creator.require_auth();
        assert!(goal > 0, "goal must be positive");
        assert!(
            deadline_ledger > env.ledger().sequence(),
            "deadline must be in the future"
        );

        let id: u64 = env.storage().instance().get(&CAMP_CNT).unwrap_or(0);
        let status = match campaign_type {
            CampaignType::Open => CampaignStatus::Active,
            CampaignType::Curated => CampaignStatus::Pending,
        };

        let campaign = Campaign {
            id,
            creator: creator.clone(),
            token,
            goal,
            raised: 0,
            title: title.clone(),
            description,
            category,
            campaign_type,
            deadline_ledger,
            status: status.clone(),
        };

        env.storage().persistent().set(&campaign_key(id), &campaign);
        env.storage().instance().set(&CAMP_CNT, &(id + 1));

        env.events().publish(
            (symbol_short!("campaign"), symbol_short!("created")),
            (id, creator, status),
        );
        id
    }

    /// Admin approves a Curated campaign → Active.
    pub fn approve_campaign(env: Env, campaign_id: u64) {
        let admin: Address = env.storage().instance().get(&ADMIN).unwrap();
        admin.require_auth();

        let mut c: Campaign = env
            .storage()
            .persistent()
            .get(&campaign_key(campaign_id))
            .expect("campaign not found");
        assert!(c.status == CampaignStatus::Pending, "not pending");

        c.status = CampaignStatus::Active;
        env.storage().persistent().set(&campaign_key(campaign_id), &c);

        env.events().publish(
            (symbol_short!("campaign"), symbol_short!("approved")),
            campaign_id,
        );
    }

    /// Creator or admin cancels a campaign that hasn't received donations.
    pub fn cancel_campaign(env: Env, caller: Address, campaign_id: u64) {
        caller.require_auth();
        let admin: Address = env.storage().instance().get(&ADMIN).unwrap();
        let mut c: Campaign = env
            .storage()
            .persistent()
            .get(&campaign_key(campaign_id))
            .expect("campaign not found");

        assert!(
            caller == c.creator || caller == admin,
            "unauthorized"
        );
        assert!(
            c.status == CampaignStatus::Pending || c.status == CampaignStatus::Active,
            "cannot cancel"
        );
        assert!(c.raised == 0, "has donations; use refund flow");

        c.status = CampaignStatus::Cancelled;
        env.storage().persistent().set(&campaign_key(campaign_id), &c);

        env.events().publish(
            (symbol_short!("campaign"), symbol_short!("cancelled")),
            campaign_id,
        );
    }

    /// Donate to an Active campaign. Locks tokens in the contract.
    /// Auto-marks Successful when raised >= goal.
    pub fn donate(env: Env, donor: Address, campaign_id: u64, amount: i128) {
        donor.require_auth();
        assert!(amount > 0, "amount must be positive");

        let mut c: Campaign = env
            .storage()
            .persistent()
            .get(&campaign_key(campaign_id))
            .expect("campaign not found");
        assert!(c.status == CampaignStatus::Active, "campaign not active");
        assert!(
            env.ledger().sequence() <= c.deadline_ledger,
            "campaign expired"
        );

        token::Client::new(&env, &c.token).transfer(
            &donor,
            &env.current_contract_address(),
            &amount,
        );

        // Update or create donation record
        let key = donation_key(campaign_id, &donor);
        let mut rec: DonationRecord = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(DonationRecord {
                campaign_id,
                donor: donor.clone(),
                amount: 0,
                refunded: false,
                ledger: env.ledger().sequence(),
            });
        rec.amount += amount;
        rec.ledger = env.ledger().sequence();
        env.storage().persistent().set(&key, &rec);

        c.raised += amount;
        if c.raised >= c.goal {
            c.status = CampaignStatus::Successful;
            env.events().publish(
                (symbol_short!("campaign"), symbol_short!("success")),
                campaign_id,
            );
        }
        env.storage().persistent().set(&campaign_key(campaign_id), &c);

        env.events().publish(
            (symbol_short!("campaign"), symbol_short!("donated")),
            (campaign_id, donor, amount),
        );
    }

    /// Creator withdraws from a Successful campaign. Platform fee goes to admin.
    pub fn withdraw(env: Env, campaign_id: u64) {
        let mut c: Campaign = env
            .storage()
            .persistent()
            .get(&campaign_key(campaign_id))
            .expect("campaign not found");
        c.creator.require_auth();
        assert!(c.status == CampaignStatus::Successful, "not successful");

        let fee_bps: u32 = env.storage().instance().get(&FEE_BPS).unwrap_or(0);
        let admin: Address = env.storage().instance().get(&ADMIN).unwrap();
        let fee = c.raised * fee_bps as i128 / 10_000;
        let payout = c.raised - fee;

        let tok = token::Client::new(&env, &c.token);
        if fee > 0 {
            tok.transfer(&env.current_contract_address(), &admin, &fee);
        }
        tok.transfer(&env.current_contract_address(), &c.creator, &payout);

        c.raised = 0;
        env.storage().persistent().set(&campaign_key(campaign_id), &c);

        env.events().publish(
            (symbol_short!("campaign"), symbol_short!("withdraw")),
            (campaign_id, payout, fee),
        );
    }

    /// Mark a campaign Failed after deadline if goal not reached.
    pub fn mark_failed(env: Env, campaign_id: u64) {
        let mut c: Campaign = env
            .storage()
            .persistent()
            .get(&campaign_key(campaign_id))
            .expect("campaign not found");
        assert!(c.status == CampaignStatus::Active, "not active");
        assert!(
            env.ledger().sequence() > c.deadline_ledger,
            "deadline not passed"
        );
        assert!(c.raised < c.goal, "goal was reached");

        c.status = CampaignStatus::Failed;
        env.storage().persistent().set(&campaign_key(campaign_id), &c);

        env.events().publish(
            (symbol_short!("campaign"), symbol_short!("failed")),
            campaign_id,
        );
    }

    /// Donor reclaims their contribution from a Failed or Cancelled campaign.
    pub fn refund(env: Env, donor: Address, campaign_id: u64) {
        donor.require_auth();
        let c: Campaign = env
            .storage()
            .persistent()
            .get(&campaign_key(campaign_id))
            .expect("campaign not found");
        assert!(
            c.status == CampaignStatus::Failed || c.status == CampaignStatus::Cancelled,
            "not refundable"
        );

        let key = donation_key(campaign_id, &donor);
        let mut rec: DonationRecord = env
            .storage()
            .persistent()
            .get(&key)
            .expect("no donation found");
        assert!(!rec.refunded, "already refunded");
        assert!(rec.amount > 0, "nothing to refund");

        let amount = rec.amount;
        rec.amount = 0;
        rec.refunded = true;
        // Zero out before transfer to prevent re-entrancy / double-claim
        env.storage().persistent().set(&key, &rec);

        token::Client::new(&env, &c.token).transfer(
            &env.current_contract_address(),
            &donor,
            &amount,
        );

        env.events().publish(
            (symbol_short!("campaign"), symbol_short!("refunded")),
            (campaign_id, donor, amount),
        );
    }

    // ── Views ────────────────────────────────────────────────────────────────

    pub fn get_campaign(env: Env, campaign_id: u64) -> Campaign {
        env.storage()
            .persistent()
            .get(&campaign_key(campaign_id))
            .expect("campaign not found")
    }

    pub fn get_donation(env: Env, campaign_id: u64, donor: Address) -> DonationRecord {
        env.storage()
            .persistent()
            .get(&donation_key(campaign_id, &donor))
            .expect("donation not found")
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token::{Client as TokenClient, StellarAssetClient},
        Env, String,
    };

    fn setup() -> (Env, CampaignContractClient<'static>, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, CampaignContract);
        let client = CampaignContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let donor = Address::generate(&env);
        client.initialize(&admin, &250u32); // 2.5% fee
        (env, client, admin, creator, donor)
    }

    fn make_token(env: &Env, admin: &Address) -> Address {
        let token_id = env.register_stellar_asset_contract_v2(admin.clone());
        token_id.address()
    }

    fn mint(env: &Env, token: &Address, admin: &Address, to: &Address, amount: i128) {
        StellarAssetClient::new(env, token).mint(to, &amount);
        let _ = admin;
    }

    #[test]
    fn open_campaign_full_lifecycle() {
        let (env, client, admin, creator, donor) = setup();
        let token = make_token(&env, &admin);
        mint(&env, &token, &admin, &donor, 1_000);

        let deadline = env.ledger().sequence() + 100;
        let id = client.create_campaign(
            &creator,
            &token,
            &500i128,
            &String::from_str(&env, "Test"),
            &String::from_str(&env, "Desc"),
            &String::from_str(&env, "Tech"),
            &CampaignType::Open,
            &deadline,
        );

        let c = client.get_campaign(&id);
        assert_eq!(c.status, CampaignStatus::Active);

        client.donate(&donor, &id, &500i128);
        let c = client.get_campaign(&id);
        assert_eq!(c.status, CampaignStatus::Successful);

        client.withdraw(&id);
        let tok = TokenClient::new(&env, &token);
        // creator receives 500 - 2.5% fee = 487 (rounded down)
        assert!(tok.balance(&creator) >= 487);
    }

    #[test]
    fn curated_campaign_requires_approval() {
        let (env, client, admin, creator, _donor) = setup();
        let token = make_token(&env, &admin);
        let deadline = env.ledger().sequence() + 100;
        let id = client.create_campaign(
            &creator,
            &token,
            &100i128,
            &String::from_str(&env, "Curated"),
            &String::from_str(&env, "Desc"),
            &String::from_str(&env, "Art"),
            &CampaignType::Curated,
            &deadline,
        );
        assert_eq!(client.get_campaign(&id).status, CampaignStatus::Pending);
        client.approve_campaign(&id);
        assert_eq!(client.get_campaign(&id).status, CampaignStatus::Active);
    }

    #[test]
    fn refund_after_failed() {
        let (env, client, admin, creator, donor) = setup();
        let token = make_token(&env, &admin);
        mint(&env, &token, &admin, &donor, 200);
        let deadline = env.ledger().sequence() + 5;
        let id = client.create_campaign(
            &creator,
            &token,
            &1_000i128,
            &String::from_str(&env, "Fail"),
            &String::from_str(&env, "Desc"),
            &String::from_str(&env, "Tech"),
            &CampaignType::Open,
            &deadline,
        );
        client.donate(&donor, &id, &200i128);
        env.ledger().with_mut(|l| l.sequence_number += 10);
        client.mark_failed(&id);
        client.refund(&donor, &id);
        assert_eq!(TokenClient::new(&env, &token).balance(&donor), 200);
    }
}
