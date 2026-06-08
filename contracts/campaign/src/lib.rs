#![no_std]
#![allow(clippy::too_many_arguments)]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, Address, Env, String, Symbol, Vec,
};

// ── Storage keys ────────────────────────────────────────────────────────────

const ADMIN: Symbol = symbol_short!("ADMIN");
const FEE_BPS: Symbol = symbol_short!("FEE_BPS");
const CAMP_CNT: Symbol = symbol_short!("CAMP_CNT");
const CATEGORIES: Symbol = symbol_short!("CATS");

fn campaign_key(id: u64) -> (Symbol, u64) {
    (symbol_short!("CAMP"), id)
}

fn donation_key(campaign_id: u64, donor: &Address) -> (Symbol, u64, Address) {
    (symbol_short!("DON"), campaign_id, donor.clone())
}

// ── Types ────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, PartialEq, Debug)]
/// Lifecycle state for a crowdfunding campaign.
pub enum CampaignStatus {
    /// Created but waiting for curated approval.
    Pending,
    /// Open for donations.
    Active,
    /// Funding goal has been reached.
    Successful,
    /// Deadline passed before the funding goal was reached.
    Failed,
    /// Cancelled before completion.
    Cancelled,
}

#[contracttype]
#[derive(Clone, PartialEq, Debug)]
/// Approval mode for a campaign.
pub enum CampaignType {
    /// Campaign is active immediately after creation.
    Open,
    /// Campaign requires admin approval before donations are accepted.
    Curated,
}

#[contracttype]
#[derive(Clone)]
/// Stored metadata and funding state for a campaign.
pub struct Campaign {
    /// Unique campaign identifier.
    pub id: u64,
    /// Address that created the campaign and can withdraw successful funds.
    pub creator: Address,
    /// Token contract accepted by this campaign.
    pub token: Address,
    /// Funding goal denominated in the campaign token.
    pub goal: i128,
    /// Total amount raised and not yet withdrawn.
    pub raised: i128,
    /// Human-readable campaign title.
    pub title: String,
    /// Longer campaign description.
    pub description: String,
    /// Category name selected from the approved category list.
    pub category: String,
    /// Campaign approval mode.
    pub campaign_type: CampaignType,
    /// Ledger sequence after which the campaign expires.
    pub deadline_ledger: u32,
    /// Current lifecycle state.
    pub status: CampaignStatus,
}

#[contracttype]
#[derive(Clone)]
/// Per-donor contribution record for a campaign.
pub struct DonationRecord {
    /// Campaign that received the donation.
    pub campaign_id: u64,
    /// Donor address.
    pub donor: Address,
    /// Current refundable donation amount.
    pub amount: i128,
    /// Whether this donation has already been refunded.
    pub refunded: bool,
    /// Ledger sequence of the most recent donation update.
    pub ledger: u32,
}

// ── Contract ─────────────────────────────────────────────────────────────────

#[contract]
/// Crowdfunding campaign management contract.
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
        env.storage()
            .instance()
            .set(&CATEGORIES, &Vec::<String>::new(&env));
    }

    /// Add an allowed campaign category.
    pub fn add_category(env: Env, admin: Address, name: String) {
        Self::require_admin(&env, &admin);
        let mut categories = Self::load_categories(&env);
        assert!(!Self::has_category(&categories, &name), "category exists");
        categories.push_back(name.clone());
        env.storage().instance().set(&CATEGORIES, &categories);
        env.events().publish(
            (
                symbol_short!("campaign"),
                Symbol::new(&env, "category_added"),
            ),
            name,
        );
    }

    /// Remove an allowed campaign category.
    pub fn remove_category(env: Env, admin: Address, name: String) {
        Self::require_admin(&env, &admin);
        let categories = Self::load_categories(&env);
        let mut next = Vec::<String>::new(&env);
        let mut found = false;
        for category in categories.iter() {
            if category == name {
                found = true;
            } else {
                next.push_back(category);
            }
        }
        assert!(found, "category not found");
        env.storage().instance().set(&CATEGORIES, &next);
        env.events().publish(
            (
                symbol_short!("campaign"),
                Symbol::new(&env, "category_removed"),
            ),
            name,
        );
    }

    /// Return the currently allowed campaign categories.
    pub fn get_categories(env: Env) -> Vec<String> {
        Self::load_categories(&env)
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
        assert!(
            Self::has_category(&Self::load_categories(&env), &category),
            "unknown category"
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
        env.storage()
            .persistent()
            .set(&campaign_key(campaign_id), &c);

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

        assert!(caller == c.creator || caller == admin, "unauthorized");
        assert!(
            c.status == CampaignStatus::Pending || c.status == CampaignStatus::Active,
            "cannot cancel"
        );
        assert!(c.raised == 0, "has donations; use refund flow");

        c.status = CampaignStatus::Cancelled;
        env.storage()
            .persistent()
            .set(&campaign_key(campaign_id), &c);

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
        let mut rec: DonationRecord =
            env.storage()
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
        env.storage()
            .persistent()
            .set(&campaign_key(campaign_id), &c);

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
        env.storage()
            .persistent()
            .set(&campaign_key(campaign_id), &c);

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
        env.storage()
            .persistent()
            .set(&campaign_key(campaign_id), &c);

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

    /// Return a stored campaign by id.
    pub fn get_campaign(env: Env, campaign_id: u64) -> Campaign {
        env.storage()
            .persistent()
            .get(&campaign_key(campaign_id))
            .expect("campaign not found")
    }

    /// Return a donor's contribution record for a campaign.
    pub fn get_donation(env: Env, campaign_id: u64, donor: Address) -> DonationRecord {
        env.storage()
            .persistent()
            .get(&donation_key(campaign_id, &donor))
            .expect("donation not found")
    }

    fn require_admin(env: &Env, admin: &Address) {
        let stored_admin: Address = env.storage().instance().get(&ADMIN).unwrap();
        assert!(&stored_admin == admin, "unauthorized");
        admin.require_auth();
    }

    fn load_categories(env: &Env) -> Vec<String> {
        env.storage()
            .instance()
            .get(&CATEGORIES)
            .unwrap_or(Vec::new(env))
    }

    fn has_category(categories: &Vec<String>, name: &String) -> bool {
        for category in categories.iter() {
            if &category == name {
                return true;
            }
        }
        false
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

    fn setup() -> (
        Env,
        CampaignContractClient<'static>,
        Address,
        Address,
        Address,
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, CampaignContract);
        let client = CampaignContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let creator = Address::generate(&env);
        let donor = Address::generate(&env);
        client.initialize(&admin, &250u32); // 2.5% fee
        client.add_category(&admin, &String::from_str(&env, "Tech"));
        client.add_category(&admin, &String::from_str(&env, "Art"));
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

    #[test]
    fn admin_can_manage_categories() {
        let (env, client, admin, _, _) = setup();
        let category = String::from_str(&env, "Education");
        client.add_category(&admin, &category);
        assert_eq!(client.get_categories().len(), 3);
        client.remove_category(&admin, &category);
        assert_eq!(client.get_categories().len(), 2);
    }

    #[test]
    #[should_panic(expected = "unauthorized")]
    fn non_admin_cannot_manage_categories() {
        let (env, client, _, creator, _) = setup();
        client.add_category(&creator, &String::from_str(&env, "Education"));
    }

    #[test]
    #[should_panic(expected = "unknown category")]
    fn create_campaign_rejects_unknown_category() {
        let (env, client, admin, creator, _) = setup();
        let token = make_token(&env, &admin);
        let deadline = env.ledger().sequence() + 100;
        client.create_campaign(
            &creator,
            &token,
            &100i128,
            &String::from_str(&env, "Unknown"),
            &String::from_str(&env, "Desc"),
            &String::from_str(&env, "Unknown"),
            &CampaignType::Open,
            &deadline,
        );
    }
}
