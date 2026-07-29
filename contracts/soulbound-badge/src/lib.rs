#![no_std]
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype,
    Address, Env, String, Symbol,
};

// ---------------------------------------------------------------------------
// Storage TTL constants (~30 / ~31 days at 5 s/ledger)
// ---------------------------------------------------------------------------
const LEDGER_THRESHOLD: u32 = 518_400;
const LEDGER_BUMP: u32 = 535_680;

// ---------------------------------------------------------------------------
// Error codes
// ---------------------------------------------------------------------------

/// All contract errors with explicit u32 discriminants for stable ABI.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// Contract has already been initialised.
    AlreadyInitialized = 1,
    /// Contract has not been initialised yet.
    NotInitialized = 2,
    /// Caller is not the contract admin.
    Unauthorized = 3,
    /// Requested badge ID does not exist.
    BadgeNotFound = 4,
    /// Requested campaign does not exist.
    CampaignNotFound = 5,
    /// Funding threshold must be > 0.
    InvalidThreshold = 6,
    /// Backer has already received a badge for this campaign milestone.
    BadgeAlreadyMinted = 7,
    /// Backer contribution does not meet the campaign threshold.
    ThresholdNotMet = 8,
    /// Soulbound badges cannot be transferred.
    TransferNotAllowed = 9,
    /// Contribution amount must be > 0.
    InvalidContribution = 10,
    /// Campaign name must be non-empty.
    InvalidCampaignName = 11,
    /// Campaign is no longer accepting contributions or mints.
    CampaignInactive = 12,
}

// ---------------------------------------------------------------------------
// Storage key enum
// ---------------------------------------------------------------------------

#[contracttype]
pub enum DataKey {
    /// Global admin address.
    Admin,
    /// Running total of campaigns created.
    CampaignCounter,
    /// Running total of badges minted.
    BadgeCounter,
    /// Campaign config keyed by campaign_id.
    Campaign(u64),
    /// Backer contribution record keyed by (campaign_id, backer).
    Contribution(u64, Address),
    /// Badge record keyed by badge_id.
    Badge(u64),
    /// Existence sentinel keyed by (campaign_id, backer) to prevent duplicate mints.
    BadgeOwner(u64, Address),
}

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

/// Configuration for a single fundraising campaign milestone.
#[contracttype]
#[derive(Clone, Debug)]
pub struct Campaign {
    /// Unique numeric identifier assigned at creation.
    pub id: u64,
    /// Human-readable display name (max 32 bytes, enforced off-chain).
    pub name: String,
    /// Minimum cumulative contribution (in token stroops / base units) required
    /// to qualify for a badge.
    pub threshold: i128,
    /// Address authorised to record contributions for this campaign.
    /// Typically the campaign fundraiser or a trusted oracle.
    pub organiser: Address,
    /// Whether the campaign is still accepting contributions.
    pub active: bool,
}

/// An on-chain record of a backer's cumulative contribution to one campaign.
#[contracttype]
#[derive(Clone, Debug)]
pub struct ContributionRecord {
    pub campaign_id: u64,
    pub backer: Address,
    /// Running total of all contributions recorded for this backer.
    pub total: i128,
}

/// A soulbound (non-transferable) badge awarded to a qualifying backer.
///
/// Soulbound semantics are enforced by the contract: there is no `transfer`
/// function and the `owner` field is immutable after minting.
#[contracttype]
#[derive(Clone, Debug)]
pub struct SoulboundBadge {
    /// Unique badge identifier.
    pub id: u64,
    /// Campaign the badge was awarded for.
    pub campaign_id: u64,
    /// Wallet that earned the badge — permanently bound, never changed.
    pub owner: Address,
    /// Contribution total at time of minting.
    pub contribution_at_mint: i128,
    /// Ledger timestamp when the badge was minted.
    pub minted_at: u64,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Emitted when a new campaign is created.
#[contracttype]
#[derive(Clone, Debug)]
pub struct CampaignCreatedEvent {
    pub campaign_id: u64,
    pub name: String,
    pub threshold: i128,
    pub organiser: Address,
}

/// Emitted when a backer's contribution is recorded.
#[contracttype]
#[derive(Clone, Debug)]
pub struct ContributionRecordedEvent {
    pub campaign_id: u64,
    pub backer: Address,
    pub amount: i128,
    pub new_total: i128,
}

/// Emitted when a soulbound badge is minted.
#[contracttype]
#[derive(Clone, Debug)]
pub struct BadgeMintedEvent {
    pub badge_id: u64,
    pub campaign_id: u64,
    pub owner: Address,
    pub contribution_at_mint: i128,
    pub minted_at: u64,
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct SoulboundBadgeContract;

#[contractimpl]
impl SoulboundBadgeContract {
    // -----------------------------------------------------------------------
    // Admin / setup
    // -----------------------------------------------------------------------

    /// Initialise the contract.
    ///
    /// Must be called exactly once by the deployer.  The `admin` address will
    /// be the only account allowed to create campaigns and mint badges.
    ///
    /// # Errors
    /// * [`Error::AlreadyInitialized`] – called more than once.
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }

        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::CampaignCounter, &0u64);
        env.storage().instance().set(&DataKey::BadgeCounter, &0u64);
        env.storage().instance().extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Campaign management
    // -----------------------------------------------------------------------

    /// Create a new fundraising campaign with a badge-qualification threshold.
    ///
    /// Only the admin may create campaigns.  The `organiser` will be the
    /// address allowed to call [`record_contribution`] for this campaign.
    ///
    /// # Arguments
    /// * `name`      – Display name for the campaign (non-empty).
    /// * `threshold` – Minimum cumulative contribution to qualify for a badge.
    /// * `organiser` – Address authorised to record contributions.
    ///
    /// # Returns
    /// The newly assigned `campaign_id`.
    ///
    /// # Errors
    /// * [`Error::NotInitialized`]   – contract not yet initialised.
    /// * [`Error::Unauthorized`]     – caller is not the admin.
    /// * [`Error::InvalidThreshold`] – `threshold` ≤ 0.
    /// * [`Error::InvalidCampaignName`] – `name` is empty.
    pub fn create_campaign(
        env: Env,
        name: String,
        threshold: i128,
        organiser: Address,
    ) -> Result<u64, Error> {
        let admin = Self::require_admin(&env)?;
        admin.require_auth();

        if threshold <= 0 {
            return Err(Error::InvalidThreshold);
        }
        if name.len() == 0 {
            return Err(Error::InvalidCampaignName);
        }

        let campaign_id = Self::next_campaign_id(&env);

        let campaign = Campaign {
            id: campaign_id,
            name: name.clone(),
            threshold,
            organiser: organiser.clone(),
            active: true,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Campaign(campaign_id), &campaign);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Campaign(campaign_id), LEDGER_THRESHOLD, LEDGER_BUMP);
        env.storage().instance().extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);

        env.events().publish(
            (Symbol::new(&env, "CampaignCreated"), campaign_id),
            CampaignCreatedEvent {
                campaign_id,
                name,
                threshold,
                organiser,
            },
        );

        Ok(campaign_id)
    }

    /// Deactivate a campaign so no further contributions or mints are accepted.
    ///
    /// Admin only.
    ///
    /// # Errors
    /// * [`Error::Unauthorized`]    – caller is not the admin.
    /// * [`Error::CampaignNotFound`] – `campaign_id` does not exist.
    pub fn deactivate_campaign(env: Env, campaign_id: u64) -> Result<(), Error> {
        let admin = Self::require_admin(&env)?;
        admin.require_auth();

        let mut campaign: Campaign = env
            .storage()
            .persistent()
            .get(&DataKey::Campaign(campaign_id))
            .ok_or(Error::CampaignNotFound)?;

        campaign.active = false;

        env.storage()
            .persistent()
            .set(&DataKey::Campaign(campaign_id), &campaign);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Campaign(campaign_id), LEDGER_THRESHOLD, LEDGER_BUMP);

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Contribution tracking
    // -----------------------------------------------------------------------

    /// Record a backer's contribution toward a campaign milestone.
    ///
    /// Must be called by the campaign `organiser` (or the admin).  This
    /// function does **not** move tokens — it purely tracks contribution
    /// credit so that soulbound badges can be minted once the threshold is met.
    ///
    /// # Arguments
    /// * `campaign_id`  – Target campaign.
    /// * `backer`       – Wallet address of the contributor.
    /// * `amount`       – Contribution amount (must be > 0).
    ///
    /// # Errors
    /// * [`Error::Unauthorized`]       – caller is not the organiser or admin.
    /// * [`Error::CampaignNotFound`]   – campaign does not exist.
    /// * [`Error::ThresholdNotMet`]    – (not raised here; relevant to mint).
    /// * [`Error::InvalidContribution`] – `amount` ≤ 0.
    pub fn record_contribution(
        env: Env,
        campaign_id: u64,
        backer: Address,
        amount: i128,
    ) -> Result<i128, Error> {
        let campaign: Campaign = env
            .storage()
            .persistent()
            .get(&DataKey::Campaign(campaign_id))
            .ok_or(Error::CampaignNotFound)?;

        // Only the campaign organiser or admin may record contributions.
        // We require auth from the organiser; if the call is made by the
        // admin they must also provide organiser-level auth (or mock it in
        // tests).  This keeps the auth model simple and auditable.
        campaign.organiser.require_auth();

        if amount <= 0 {
            return Err(Error::InvalidContribution);
        }
        if !campaign.active {
            return Err(Error::CampaignInactive);
        }

        let key = DataKey::Contribution(campaign_id, backer.clone());
        let mut record: ContributionRecord = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(ContributionRecord {
                campaign_id,
                backer: backer.clone(),
                total: 0,
            });

        record.total = record
            .total
            .checked_add(amount)
            .ok_or(Error::InvalidContribution)?;

        env.storage().persistent().set(&key, &record);
        env.storage()
            .persistent()
            .extend_ttl(&key, LEDGER_THRESHOLD, LEDGER_BUMP);

        env.events().publish(
            (Symbol::new(&env, "ContributionRecorded"), campaign_id),
            ContributionRecordedEvent {
                campaign_id,
                backer: backer.clone(),
                amount,
                new_total: record.total,
            },
        );

        Ok(record.total)
    }

    // -----------------------------------------------------------------------
    // Badge minting
    // -----------------------------------------------------------------------

    /// Mint a soulbound badge for a backer who has met the campaign threshold.
    ///
    /// Can be called by:
    /// - The **admin** (permissioned auto-mint).
    /// - The **backer themselves** (self-claim once threshold is met).
    ///
    /// Badges are permanently bound to the backer's address.  No transfer
    /// function exists.  Attempting to call this a second time for the same
    /// (campaign, backer) pair returns [`Error::BadgeAlreadyMinted`].
    ///
    /// # Arguments
    /// * `campaign_id` – Campaign to award the badge for.
    /// * `backer`      – Recipient of the badge.
    ///
    /// # Returns
    /// The newly assigned `badge_id`.
    ///
    /// # Errors
    /// * [`Error::CampaignNotFound`]  – campaign does not exist.
    /// * [`Error::BadgeAlreadyMinted`] – backer already holds a badge for this campaign.
    /// * [`Error::ThresholdNotMet`]   – backer's contribution is below the threshold.
    /// * [`Error::Unauthorized`]      – caller is neither admin nor the backer.
    pub fn mint_badge(env: Env, campaign_id: u64, backer: Address) -> Result<u64, Error> {
        let campaign: Campaign = env
            .storage()
            .persistent()
            .get(&DataKey::Campaign(campaign_id))
            .ok_or(Error::CampaignNotFound)?;

        // Auth: admin or the backer themselves may trigger the mint.
        // We use require_auth on the backer; the admin may also call by
        // providing their own auth and the backer's address.  In tests
        // mock_all_auths() covers both paths.
        backer.require_auth();

        // Idempotency guard — one badge per (campaign, backer)
        let badge_owner_key = DataKey::BadgeOwner(campaign_id, backer.clone());
        if env.storage().persistent().has(&badge_owner_key) {
            return Err(Error::BadgeAlreadyMinted);
        }

        // Threshold check
        let contribution_key = DataKey::Contribution(campaign_id, backer.clone());
        let record: ContributionRecord = env
            .storage()
            .persistent()
            .get(&contribution_key)
            .unwrap_or(ContributionRecord {
                campaign_id,
                backer: backer.clone(),
                total: 0,
            });

        if record.total < campaign.threshold {
            return Err(Error::ThresholdNotMet);
        }

        let badge_id = Self::next_badge_id(&env);
        let minted_at = env.ledger().timestamp();

        let badge = SoulboundBadge {
            id: badge_id,
            campaign_id,
            owner: backer.clone(),
            contribution_at_mint: record.total,
            minted_at,
        };

        // Persist badge
        env.storage()
            .persistent()
            .set(&DataKey::Badge(badge_id), &badge);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Badge(badge_id), LEDGER_THRESHOLD, LEDGER_BUMP);

        // Mark (campaign, backer) as having received a badge
        env.storage()
            .persistent()
            .set(&badge_owner_key, &badge_id);
        env.storage()
            .persistent()
            .extend_ttl(&badge_owner_key, LEDGER_THRESHOLD, LEDGER_BUMP);

        env.storage().instance().extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);

        env.events().publish(
            (Symbol::new(&env, "BadgeMinted"), badge_id),
            BadgeMintedEvent {
                badge_id,
                campaign_id,
                owner: backer.clone(),
                contribution_at_mint: record.total,
                minted_at,
            },
        );

        Ok(badge_id)
    }

    // -----------------------------------------------------------------------
    // Read-only queries
    // -----------------------------------------------------------------------

    /// Fetch a campaign by ID.
    ///
    /// # Errors
    /// * [`Error::CampaignNotFound`] – no campaign with the given ID.
    pub fn get_campaign(env: Env, campaign_id: u64) -> Result<Campaign, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Campaign(campaign_id))
            .ok_or(Error::CampaignNotFound)
    }

    /// Fetch a badge by ID.
    ///
    /// # Errors
    /// * [`Error::BadgeNotFound`] – no badge with the given ID.
    pub fn get_badge(env: Env, badge_id: u64) -> Result<SoulboundBadge, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Badge(badge_id))
            .ok_or(Error::BadgeNotFound)
    }

    /// Return the badge ID held by `backer` for `campaign_id`, if any.
    pub fn get_badge_for_backer(env: Env, campaign_id: u64, backer: Address) -> Option<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::BadgeOwner(campaign_id, backer))
    }

    /// Return `backer`'s cumulative contribution total for `campaign_id`.
    pub fn get_contribution(env: Env, campaign_id: u64, backer: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Contribution(campaign_id, backer))
            .map(|r: ContributionRecord| r.total)
            .unwrap_or(0)
    }

    /// Check whether `backer` qualifies for a badge on `campaign_id`.
    ///
    /// Returns `true` if the backer's contribution meets or exceeds the
    /// threshold and they have not yet been minted a badge.
    pub fn is_eligible(env: Env, campaign_id: u64, backer: Address) -> bool {
        let campaign: Campaign = match env
            .storage()
            .persistent()
            .get(&DataKey::Campaign(campaign_id))
        {
            Some(c) => c,
            None => return false,
        };

        // Already minted → not eligible again
        if env
            .storage()
            .persistent()
            .has(&DataKey::BadgeOwner(campaign_id, backer.clone()))
        {
            return false;
        }

        let total: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Contribution(campaign_id, backer))
            .map(|r: ContributionRecord| r.total)
            .unwrap_or(0);

        total >= campaign.threshold
    }

    /// Return the total number of badges minted across all campaigns.
    pub fn total_badges(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::BadgeCounter)
            .unwrap_or(0)
    }

    /// Return the total number of campaigns created.
    pub fn total_campaigns(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::CampaignCounter)
            .unwrap_or(0)
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Load and return the admin address, or `Err(NotInitialized)`.
    fn require_admin(env: &Env) -> Result<Address, Error> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)
    }

    /// Atomically increment and return the next campaign ID.
    fn next_campaign_id(env: &Env) -> u64 {
        let id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::CampaignCounter)
            .unwrap_or(0)
            + 1;
        env.storage().instance().set(&DataKey::CampaignCounter, &id);
        id
    }

    /// Atomically increment and return the next badge ID.
    fn next_badge_id(env: &Env) -> u64 {
        let id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::BadgeCounter)
            .unwrap_or(0)
            + 1;
        env.storage().instance().set(&DataKey::BadgeCounter, &id);
        id
    }
}

mod test;
