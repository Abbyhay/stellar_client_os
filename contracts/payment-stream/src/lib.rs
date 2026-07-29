#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, panic_with_error, token, Address, Env, Symbol};

/// Stream status enum
#[contracttype]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StreamStatus {
    Active,
    Paused,
    Canceled,
    Completed,
    Disputed,
}

/// Stream data structure
#[contracttype]
#[derive(Clone)]
pub struct Stream {
    pub id: u64,
    pub sender: Address,
    pub recipient: Address,
    pub token: Address,
    pub total_amount: i128,
    pub balance: i128,
    pub withdrawn_amount: i128,
    pub start_time: u64,
    pub end_time: u64,
    pub status: StreamStatus,
    pub paused_at: Option<u64>,  
    pub total_paused_duration: u64,
}

/// Per-stream metrics tracking
#[contracttype]
#[derive(Clone)]
pub struct StreamMetrics {
    pub last_activity: u64,           // Timestamp of last stream activity
    pub total_withdrawn: i128,        // Total amount withdrawn from stream
    pub withdrawal_count: u32,        // Number of withdrawal operations
    pub pause_count: u32,             // Number of times stream was paused
    pub total_delegations: u32,       // Total number of delegation changes
    pub current_delegate: Option<Address>, // Current delegate (if any)
    pub last_delegation_time: u64,    // Timestamp of last delegation change
}

/// Protocol-wide metrics tracking
#[contracttype]
#[derive(Clone)]
pub struct ProtocolMetrics {
    pub total_active_streams: u64,    // Count of currently active streams
    pub total_tokens_streamed: i128,  // Total tokens ever streamed
    pub total_streams_created: u64,   // Total number of streams created
    pub total_delegations: u64,       // Total number of delegations across all streams
}

/// A dispute resolution that has been decided but is queued behind a
/// mandatory timelock before its payout can be executed.
#[contracttype]
#[derive(Clone)]
pub struct QueuedResolution {
    pub dispute_id: u64,
    pub stream_id: u64,
    pub recipient_amount: i128,
    pub sender_amount: i128,
    pub execute_after: u64,
    pub executed: bool,
    pub previous_status: StreamStatus,
}

/// Dispute queued event data
#[contracttype]
#[derive(Clone)]
pub struct DisputeQueuedEvent {
    pub dispute_id: u64,
    pub stream_id: u64,
    pub recipient_amount: i128,
    pub sender_amount: i128,
    pub execute_after: u64,
}

/// Dispute executed event data
#[contracttype]
#[derive(Clone)]
pub struct DisputeExecutedEvent {
    pub dispute_id: u64,
    pub stream_id: u64,
    pub recipient_amount: i128,
    pub sender_amount: i128,
}

/// Dispute canceled event data
#[contracttype]
#[derive(Clone)]
pub struct DisputeCanceledEvent {
    pub dispute_id: u64,
    pub stream_id: u64,
}

/// Fee collected event data
#[contracttype]
#[derive(Clone)]
pub struct FeeCollectedEvent {
    pub stream_id: u64,
    pub amount: i128,
}

/// Stream deposit event data
#[contracttype]
#[derive(Clone)]
pub struct StreamDepositEvent {
    pub stream_id: u64,
    pub amount: i128,
}

/// Delegation granted event data
#[contracttype]
#[derive(Clone)]
pub struct DelegationGrantedEvent {
    pub stream_id: u64,
    pub recipient: Address,
    pub delegate: Address,
}

/// Delegation revoked event data
#[contracttype]
#[derive(Clone)]
pub struct DelegationRevokedEvent {
    pub stream_id: u64,
    pub recipient: Address,
}

// Stream paused event
#[contracttype]
#[derive(Clone)]
pub struct StreamPausedEvent {
    pub stream_id: u64,
    pub paused_at: u64,
}

// Stream resumed event
#[contracttype]
#[derive(Clone)]
pub struct StreamResumedEvent {
    pub stream_id: u64,
    pub resumed_at: u64,
    pub paused_duration: u64,
}

/// Custom errors for the contract
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    InvalidAmount = 4,
    InvalidTimeRange = 5,
    StreamNotFound = 6,
    StreamNotActive = 7,
    StreamNotPaused = 8,
    StreamCannotBeCanceled = 9,
    InsufficientWithdrawable = 10,
    TransferFailed = 11,
    FeeTooHigh = 12,
    InvalidRecipient = 13,
    DepositExceedsTotal = 14,
    ArithmeticOverflow = 15,
    InvalidDelegate = 16,
    DisputeInProgress = 17,
    DisputeAlreadyQueued = 18,
    DisputeNotFound = 19,
    DisputeAlreadyExecuted = 20,
    TimelockNotElapsed = 21,
    InvalidResolutionAmounts = 22,
}

// Constants
const MAX_FEE: u32 = 500; // 5% in basis points
const LEDGER_THRESHOLD: u32 = 518400; // ~30 days at 5s/ledger
const LEDGER_BUMP: u32 = 535680; // ~31 days
const DISPUTE_TIMELOCK_DELAY: u64 = 172800; // 48 hours in seconds

#[contract]
pub struct PaymentStreamContract;

#[contractimpl]
impl PaymentStreamContract {
    /// Initialize the contract
    pub fn initialize(env: Env, admin: Address, fee_collector: Address, general_fee_rate: u32) {
        if env.storage().instance().has(&Symbol::new(&env, "admin")) {
            panic_with_error!(&env, Error::AlreadyInitialized);
        }
        if general_fee_rate > MAX_FEE {
            panic_with_error!(&env, Error::FeeTooHigh);
        }
        admin.require_auth();
        
        env.storage().instance().set(&Symbol::new(&env, "admin"), &admin);
        env.storage().instance().set(&Symbol::new(&env, "stream_count"), &0u64);
        env.storage().instance().set(&Symbol::new(&env, "fee_collector"), &fee_collector);
        env.storage().instance().set(&Symbol::new(&env, "general_protocol_fee_rate"), &general_fee_rate);
        
        // Initialize protocol metrics
        let initial_metrics = ProtocolMetrics {
            total_active_streams: 0,
            total_tokens_streamed: 0,
            total_streams_created: 0,
            total_delegations: 0,
        };
        env.storage().instance().set(&Symbol::new(&env, "protocol_metrics"), &initial_metrics);
        
        env.storage().instance().extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);
    }

    /// Create a new payment stream
    pub fn create_stream(
        env: Env,
        sender: Address,
        recipient: Address,
        token: Address,
        total_amount: i128,
        initial_amount: i128,
        start_time: u64,
        end_time: u64,
    ) -> u64 {
        sender.require_auth();

        // Validate inputs
        if total_amount <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        if initial_amount < 0 || initial_amount > total_amount {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        if end_time <= start_time {
            panic_with_error!(&env, Error::InvalidTimeRange);
        }

        // Get and increment stream count
        let mut stream_count: u64 = env.storage().instance().get(&Symbol::new(&env, "stream_count")).unwrap_or(0);
        let stream_id = stream_count + 1;
        stream_count += 1;
        env.storage().instance().set(&Symbol::new(&env, "stream_count"), &stream_count);

        let current_time = env.ledger().timestamp();

        // Create stream
        let stream = Stream {
            id: stream_id,
            sender: sender.clone(),
            recipient: recipient.clone(),
            token: token.clone(),
            total_amount,
            balance: initial_amount,
            withdrawn_amount: 0,
            start_time,
            end_time,
            status: StreamStatus::Active,
            paused_at: None,
            total_paused_duration: 0,
        };

        // Initialize stream metrics
        let stream_metrics = StreamMetrics {
            last_activity: current_time,
            total_withdrawn: 0,
            withdrawal_count: 0,
            pause_count: 0,
            total_delegations: 0,
            current_delegate: None,
            last_delegation_time: 0,
        };

        // Store stream and metrics
        env.storage().persistent().set(&stream_id, &stream);
        env.storage().persistent().set(&(stream_id, Symbol::new(&env, "metrics")), &stream_metrics);
        env.storage().persistent().extend_ttl(&stream_id, LEDGER_THRESHOLD, LEDGER_BUMP);
        env.storage().persistent().extend_ttl(&(stream_id, Symbol::new(&env, "metrics")), LEDGER_THRESHOLD, LEDGER_BUMP);

        // Update protocol metrics
        let mut protocol_metrics: ProtocolMetrics = env.storage().instance()
            .get(&Symbol::new(&env, "protocol_metrics"))
            .unwrap_or(ProtocolMetrics {
                total_active_streams: 0,
                total_tokens_streamed: 0,
                total_streams_created: 0,
                total_delegations: 0,
            });

        protocol_metrics.total_active_streams += 1;
        protocol_metrics.total_tokens_streamed += total_amount;
        protocol_metrics.total_streams_created += 1;

        env.storage().instance().set(&Symbol::new(&env, "protocol_metrics"), &protocol_metrics);
        env.storage().instance().extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);

        // Transfer tokens from sender to contract (escrow)
        if initial_amount > 0 {
            let token_client = token::Client::new(&env, &token);
            token_client.transfer(&sender, &env.current_contract_address(), &initial_amount);
        }

        stream_id
    }

    /// Deposit tokens to an existing stream
    pub fn deposit(env: Env, stream_id: u64, amount: i128) {
        let mut stream: Stream = Self::get_stream(env.clone(), stream_id);

        if matches!(stream.status, StreamStatus::Canceled | StreamStatus::Completed) {
            panic_with_error!(&env, Error::StreamNotActive);
        }
        if stream.status == StreamStatus::Disputed {
            panic_with_error!(&env, Error::DisputeInProgress);
        }

        stream.sender.require_auth();

        if amount <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }

        let new_balance = stream.balance.checked_add(amount)
            .unwrap_or_else(|| panic_with_error!(&env, Error::ArithmeticOverflow));

        if new_balance > stream.total_amount {
            panic_with_error!(&env, Error::DepositExceedsTotal);
        }

        // Transfer tokens from sender to contract
        let token_client = token::Client::new(&env, &stream.token);
        token_client.transfer(&stream.sender, &env.current_contract_address(), &amount);

        // Update balance
        stream.balance = new_balance;
        env.storage().persistent().set(&stream_id, &stream);
        env.storage().persistent().extend_ttl(&stream_id, LEDGER_THRESHOLD, LEDGER_BUMP);

        // Update stream metrics
        let mut metrics: StreamMetrics = env.storage().persistent()
            .get(&(stream_id, Symbol::new(&env, "metrics")))
            .unwrap_or_else(|| Self::default_stream_metrics(&env));

        metrics.last_activity = env.ledger().timestamp();

        env.storage().persistent().set(&(stream_id, Symbol::new(&env, "metrics")), &metrics);
        env.storage().persistent().extend_ttl(&(stream_id, Symbol::new(&env, "metrics")), LEDGER_THRESHOLD, LEDGER_BUMP);

        // Emit StreamDeposit event
        env.events().publish(("StreamDeposit", stream_id), StreamDepositEvent { stream_id, amount });
    }

    /// Get stream details
    pub fn get_stream(env: Env, stream_id: u64) -> Stream {
        match env.storage().persistent().get(&stream_id) {
            Some(stream) => {
                env.storage().persistent().extend_ttl(&stream_id, LEDGER_THRESHOLD, LEDGER_BUMP);
                stream
            },
            None => panic_with_error!(&env, Error::StreamNotFound),
        }
    }

    /// Helper function to create default stream metrics
    fn default_stream_metrics(env: &Env) -> StreamMetrics {
        StreamMetrics {
            last_activity: env.ledger().timestamp(),
            total_withdrawn: 0,
            withdrawal_count: 0,
            pause_count: 0,
            total_delegations: 0,
            current_delegate: None,
            last_delegation_time: 0,
        }
    }

    /// Assert that the caller is authorized to withdraw (recipient or delegate).
    fn assert_is_recipient_or_delegate(env: &Env, stream_id: u64) {
        let stream: Stream = Self::get_stream(env.clone(), stream_id);
        
        // First, check if a delegate is set and try to require auth from them
        let delegate_opt: Option<Address> = env.storage().persistent().get(&(stream_id, Symbol::new(env, "delegate")));
        
        if let Some(delegate) = delegate_opt {
            // If delegate exists, require auth from delegate (they're the one calling)
            delegate.require_auth();
        } else {
            // No delegate, require auth from recipient
            stream.recipient.require_auth();
        }
    }

    /// Set a delegate for withdrawal rights on a stream
    pub fn set_delegate(env: Env, stream_id: u64, delegate: Address) {
        let stream: Stream = Self::get_stream(env.clone(), stream_id);
        stream.recipient.require_auth();
    
        // Prevent self-delegation
        if delegate == stream.recipient {
            panic_with_error!(&env, Error::InvalidDelegate);
        }

        // Check if there's an existing delegate and emit revocation event
        let delegate_key = (stream_id, Symbol::new(&env, "delegate"));
        if let Some(old_delegate) = env.storage().persistent().get::<_, Address>(&delegate_key) {
            if old_delegate != delegate {
                let revoke_event = DelegationRevokedEvent {
                    stream_id,
                    recipient: stream.recipient.clone(),
                };
                env.events().publish(("DelegationRevoked", stream_id), revoke_event);
            }
        }

        let current_time = env.ledger().timestamp();

        // Store delegate
        env.storage().persistent().set(&(stream_id, Symbol::new(&env, "delegate")), &delegate);
        env.storage().persistent().extend_ttl(&(stream_id, Symbol::new(&env, "delegate")), LEDGER_THRESHOLD, LEDGER_BUMP);

        // Update stream metrics
        let mut metrics: StreamMetrics = env.storage().persistent()
            .get(&(stream_id, Symbol::new(&env, "metrics")))
            .unwrap_or_else(|| Self::default_stream_metrics(&env));

        metrics.total_delegations += 1;
        metrics.current_delegate = Some(delegate.clone());
        metrics.last_delegation_time = current_time;
        metrics.last_activity = current_time;

        env.storage().persistent().set(&(stream_id, Symbol::new(&env, "metrics")), &metrics);
        env.storage().persistent().extend_ttl(&(stream_id, Symbol::new(&env, "metrics")), LEDGER_THRESHOLD, LEDGER_BUMP);

        // Update protocol metrics
        let mut protocol_metrics: ProtocolMetrics = env.storage().instance()
            .get(&Symbol::new(&env, "protocol_metrics"))
            .unwrap();
        protocol_metrics.total_delegations += 1;
        env.storage().instance().set(&Symbol::new(&env, "protocol_metrics"), &protocol_metrics);
        env.storage().instance().extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);

        // Emit event
        let event = DelegationGrantedEvent {
            stream_id,
            recipient: stream.recipient,
            delegate: delegate.clone(),
        };
        env.events().publish(("DelegationGranted", stream_id), event);
    }

    /// Revoke the delegate for a stream
    pub fn revoke_delegate(env: Env, stream_id: u64) {
        let stream: Stream = Self::get_stream(env.clone(), stream_id);
        stream.recipient.require_auth();

        let delegate_key = (stream_id, Symbol::new(&env, "delegate"));
        let had_delegate = env.storage().persistent().has(&delegate_key);

        // Remove delegate
        env.storage().persistent().remove(&delegate_key);

        // Update stream metrics
        if had_delegate {
            let mut metrics: StreamMetrics = env.storage().persistent()
                .get(&(stream_id, Symbol::new(&env, "metrics")))
                .unwrap_or_else(|| Self::default_stream_metrics(&env));

            metrics.current_delegate = None;
            metrics.last_activity = env.ledger().timestamp();

            env.storage().persistent().set(&(stream_id, Symbol::new(&env, "metrics")), &metrics);
            env.storage().persistent().extend_ttl(&(stream_id, Symbol::new(&env, "metrics")), LEDGER_THRESHOLD, LEDGER_BUMP);

            // Emit event
            let event = DelegationRevokedEvent {
                stream_id,
                recipient: stream.recipient,
            };
            env.events().publish(("DelegationRevoked", stream_id), event);
        }
    }

    /// Get the delegate for a stream
    pub fn get_delegate(env: Env, stream_id: u64) -> Option<Address> {
        // Ensure stream exists
        Self::get_stream(env.clone(), stream_id);
        env.storage().persistent().get(&(stream_id, Symbol::new(&env, "delegate")))
    }

    /// Calculate the protocol fee for a given amount
    fn calculate_protocol_fee(env: &Env, amount: i128) -> i128 {
        let fee_rate: u32 = env.storage().instance().get(&Symbol::new(env, "general_protocol_fee_rate")).unwrap_or(0);

        if fee_rate == 0 || amount <= 0 {
            return 0;
        }

        // fee = (amount * fee_rate) / 10000
        // Split calculation to avoid overflow while preserving precision
        let rate = fee_rate as i128;
        let fee = (amount / 10000) * rate + ((amount % 10000) * rate) / 10000;
        fee.max(0)
    }

    /// Calculate withdrawable amount for a stream
    pub fn withdrawable_amount(env: Env, stream_id: u64) -> i128 {
        let stream: Stream = Self::get_stream(env.clone(), stream_id);

        // Paused streams have no withdrawable amount
        if stream.status == StreamStatus::Paused {
            return 0;
        }

        // Only active streams can have withdrawable amounts
        if stream.status != StreamStatus::Active {
            return 0;
        }

        let current_time = env.ledger().timestamp();

        if current_time <= stream.start_time {
            return 0;
        }

        // Calculate effective elapsed time (excluding paused duration)
        let raw_elapsed = if current_time >= stream.end_time {
            stream.end_time - stream.start_time
        } else {
            current_time - stream.start_time
        };

        // Subtract the total paused duration from elapsed time
        let elapsed = raw_elapsed.saturating_sub(stream.total_paused_duration);

        let duration = (stream.end_time - stream.start_time).saturating_sub(stream.total_paused_duration);
        if duration == 0 {
            return 0;
        }

        let vested = (stream.total_amount * elapsed as i128) / duration as i128;

        vested - stream.withdrawn_amount
    }

    /// Withdraw from a stream
    pub fn withdraw(env: Env, stream_id: u64, amount: i128) {
        let mut stream: Stream = Self::get_stream(env.clone(), stream_id);

        Self::assert_is_recipient_or_delegate(&env, stream_id);

        let available = Self::withdrawable_amount(env.clone(), stream_id);
        if amount > available || amount <= 0 {
            panic_with_error!(&env, Error::InsufficientWithdrawable);
        }

        // Calculate protocol fee
        let fee = Self::calculate_protocol_fee(&env, amount);
        let net_amount = amount - fee;

        stream.withdrawn_amount += amount;

        // Check if stream is completed
        if stream.withdrawn_amount >= stream.total_amount {
            stream.status = StreamStatus::Completed;
            
            // Update protocol metrics - decrease active streams
            let mut protocol_metrics: ProtocolMetrics = env.storage().instance()
                .get(&Symbol::new(&env, "protocol_metrics"))
                .unwrap();
            protocol_metrics.total_active_streams = protocol_metrics.total_active_streams.saturating_sub(1);
            env.storage().instance().set(&Symbol::new(&env, "protocol_metrics"), &protocol_metrics);
        }

        env.storage().persistent().set(&stream_id, &stream);
        env.storage().persistent().extend_ttl(&stream_id, LEDGER_THRESHOLD, LEDGER_BUMP);

        // Update stream metrics
        let mut metrics: StreamMetrics = env.storage().persistent()
            .get(&(stream_id, Symbol::new(&env, "metrics")))
            .unwrap_or_else(|| Self::default_stream_metrics(&env));

        metrics.total_withdrawn += amount;
        metrics.withdrawal_count += 1;
        metrics.last_activity = env.ledger().timestamp();

        env.storage().persistent().set(&(stream_id, Symbol::new(&env, "metrics")), &metrics);
        env.storage().persistent().extend_ttl(&(stream_id, Symbol::new(&env, "metrics")), LEDGER_THRESHOLD, LEDGER_BUMP);

        // Transfer net amount to recipient
        let token_client = token::Client::new(&env, &stream.token);
        token_client.transfer(&env.current_contract_address(), &stream.recipient, &net_amount);

        // Transfer fee to collector if fee > 0
        if fee > 0 {
            let fee_collector: Address = env.storage().instance().get(&Symbol::new(&env, "fee_collector")).unwrap();
            token_client.transfer(&env.current_contract_address(), &fee_collector, &fee);
            env.events().publish(("FeeCollected", stream_id), fee);
        }
    }

    /// Withdraw the maximum available amount from a stream
    pub fn withdraw_max(env: Env, stream_id: u64) {
        let available = Self::withdrawable_amount(env.clone(), stream_id);
        if available <= 0 {
            panic_with_error!(&env, Error::InsufficientWithdrawable);
        }
        Self::withdraw(env, stream_id, available);
    }

    /// Pause a stream (sender only)
    pub fn pause_stream(env: Env, stream_id: u64) {
        let mut stream: Stream = Self::get_stream(env.clone(), stream_id);

        stream.sender.require_auth();

        if stream.status != StreamStatus::Active {
            panic_with_error!(&env, Error::StreamNotActive);
        }

        let current_time = env.ledger().timestamp();
        
        stream.status = StreamStatus::Paused;
        stream.paused_at = Some(current_time);

        env.storage().persistent().set(&stream_id, &stream);
        env.storage().persistent().extend_ttl(&stream_id, LEDGER_THRESHOLD, LEDGER_BUMP);

        // Update stream metrics
        let mut metrics: StreamMetrics = env.storage().persistent()
            .get(&(stream_id, Symbol::new(&env, "metrics")))
            .unwrap_or_else(|| Self::default_stream_metrics(&env));

        metrics.pause_count += 1;
        metrics.last_activity = current_time;

        env.storage().persistent().set(&(stream_id, Symbol::new(&env, "metrics")), &metrics);
        env.storage().persistent().extend_ttl(&(stream_id, Symbol::new(&env, "metrics")), LEDGER_THRESHOLD, LEDGER_BUMP);

        // Update protocol metrics - decrease active streams
        let mut protocol_metrics: ProtocolMetrics = env.storage().instance()
            .get(&Symbol::new(&env, "protocol_metrics"))
            .unwrap();
        protocol_metrics.total_active_streams = protocol_metrics.total_active_streams.saturating_sub(1);
        env.storage().instance().set(&Symbol::new(&env, "protocol_metrics"), &protocol_metrics);
        env.storage().instance().extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);

        // Emit StreamPaused event
        env.events().publish(
            ("StreamPaused", stream_id),
            StreamPausedEvent {
                stream_id,
                paused_at: current_time,
            },
        );
    }

    /// Resume a paused stream (sender only)
    pub fn resume_stream(env: Env, stream_id: u64) {
        let mut stream: Stream = Self::get_stream(env.clone(), stream_id);

        stream.sender.require_auth();

        if stream.status != StreamStatus::Paused {
            panic_with_error!(&env, Error::StreamNotPaused);
        }

        let current_time = env.ledger().timestamp();
        
        // Calculate pause duration
        let paused_duration = if let Some(paused_at) = stream.paused_at {
            current_time.saturating_sub(paused_at)
        } else {
            0
        };

        // Update total paused duration
        stream.total_paused_duration += paused_duration;
        
        // Extend end_time by the paused duration
        stream.end_time += paused_duration;
        
        stream.status = StreamStatus::Active;
        stream.paused_at = None;

        env.storage().persistent().set(&stream_id, &stream);
        env.storage().persistent().extend_ttl(&stream_id, LEDGER_THRESHOLD, LEDGER_BUMP);

        // Update stream metrics
        let mut metrics: StreamMetrics = env.storage().persistent()
            .get(&(stream_id, Symbol::new(&env, "metrics")))
            .unwrap_or_else(|| Self::default_stream_metrics(&env));

        metrics.last_activity = current_time;

        env.storage().persistent().set(&(stream_id, Symbol::new(&env, "metrics")), &metrics);
        env.storage().persistent().extend_ttl(&(stream_id, Symbol::new(&env, "metrics")), LEDGER_THRESHOLD, LEDGER_BUMP);

        // Update protocol metrics - increase active streams
        let mut protocol_metrics: ProtocolMetrics = env.storage().instance()
            .get(&Symbol::new(&env, "protocol_metrics"))
            .unwrap();
        protocol_metrics.total_active_streams += 1;
        env.storage().instance().set(&Symbol::new(&env, "protocol_metrics"), &protocol_metrics);
        env.storage().instance().extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);

        // Emit StreamResumed event
        env.events().publish(
            ("StreamResumed", stream_id),
            StreamResumedEvent {
                stream_id,
                resumed_at: current_time,
                paused_duration,
            },
        );
    }

    /// Cancel a stream
    pub fn cancel_stream(env: Env, stream_id: u64) {
        let mut stream: Stream = Self::get_stream(env.clone(), stream_id);

        stream.sender.require_auth();

        if stream.status != StreamStatus::Active && stream.status != StreamStatus::Paused {
            panic_with_error!(&env, Error::StreamCannotBeCanceled);
        }
        
        let was_active = stream.status == StreamStatus::Active;
        stream.status = StreamStatus::Canceled;

        env.storage().persistent().set(&stream_id, &stream);
        env.storage().persistent().extend_ttl(&stream_id, LEDGER_THRESHOLD, LEDGER_BUMP);

        // Update stream metrics
        let mut metrics: StreamMetrics = env.storage().persistent()
            .get(&(stream_id, Symbol::new(&env, "metrics")))
            .unwrap_or_else(|| Self::default_stream_metrics(&env));

        metrics.last_activity = env.ledger().timestamp();

        env.storage().persistent().set(&(stream_id, Symbol::new(&env, "metrics")), &metrics);
        env.storage().persistent().extend_ttl(&(stream_id, Symbol::new(&env, "metrics")), LEDGER_THRESHOLD, LEDGER_BUMP);

        // Update protocol metrics - decrease active streams if it was active
        if was_active {
            let mut protocol_metrics: ProtocolMetrics = env.storage().instance()
                .get(&Symbol::new(&env, "protocol_metrics"))
                .unwrap();
            protocol_metrics.total_active_streams = protocol_metrics.total_active_streams.saturating_sub(1);
            env.storage().instance().set(&Symbol::new(&env, "protocol_metrics"), &protocol_metrics);
            env.storage().instance().extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);
        }

        // Refund remaining tokens to sender
        let remaining = (stream.balance - stream.withdrawn_amount).max(0);
        if remaining > 0 {
            let token_client = token::Client::new(&env, &stream.token);
            token_client.transfer(&env.current_contract_address(), &stream.sender, &remaining);
        }
    }

    /// Set the protocol fee rate
    pub fn set_protocol_fee_rate(env: Env, new_fee_rate: u32) {
        let admin: Address = env.storage().instance().get(&Symbol::new(&env, "admin")).unwrap();
        admin.require_auth();

        if new_fee_rate > MAX_FEE {
            panic_with_error!(&env, Error::FeeTooHigh);
        }

        env.storage().instance().set(&Symbol::new(&env, "general_protocol_fee_rate"), &new_fee_rate);
        env.storage().instance().extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);
    }

    /// Set the fee collector address
    pub fn set_fee_collector(env: Env, new_fee_collector: Address) {
        let admin: Address = env.storage().instance().get(&Symbol::new(&env, "admin")).unwrap();
        admin.require_auth();

        env.storage().instance().set(&Symbol::new(&env, "fee_collector"), &new_fee_collector);
        env.storage().instance().extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);
    }

    /// Get the current protocol fee rate
    pub fn get_protocol_fee_rate(env: Env) -> u32 {
        env.storage().instance().get(&Symbol::new(&env, "general_protocol_fee_rate")).unwrap_or(0)
    }

    /// Get the current fee collector
    pub fn get_fee_collector(env: Env) -> Address {
        env.storage().instance().get(&Symbol::new(&env, "fee_collector")).unwrap()
    }

    /// Get stream-specific metrics
    pub fn get_stream_metrics(env: Env, stream_id: u64) -> StreamMetrics {
        // Ensure stream exists
        Self::get_stream(env.clone(), stream_id);
        
        // Return metrics or default if not found
        env.storage().persistent()
            .get(&(stream_id, Symbol::new(&env, "metrics")))
            .unwrap_or_else(|| Self::default_stream_metrics(&env))
    }

    /// Get protocol-wide metrics
    pub fn get_protocol_metrics(env: Env) -> ProtocolMetrics {
        env.storage().instance()
            .get(&Symbol::new(&env, "protocol_metrics"))
            .unwrap_or(ProtocolMetrics {
                total_active_streams: 0,
                total_tokens_streamed: 0,
                total_streams_created: 0,
                total_delegations: 0,
            })
    }

    /// Record a decided dispute resolution outcome for a stream and queue
    /// its payout behind a mandatory 48-hour timelock (admin/arbiter only).
    ///
    /// The stream is moved into `Disputed` status for the duration of the
    /// timelock, blocking deposits, withdrawals, pausing, resuming, and
    /// cancellation until the resolution is either executed via
    /// [`Self::execute_resolution`] or reversed via
    /// [`Self::cancel_queued_resolution`]. `recipient_amount` and
    /// `sender_amount` are drawn from the stream's currently escrowed,
    /// unwithdrawn balance and must not exceed it.
    ///
    /// Returns the id of the queued dispute.
    pub fn resolve_dispute(
        env: Env,
        stream_id: u64,
        recipient_amount: i128,
        sender_amount: i128,
    ) -> u64 {
        let admin: Address = env.storage().instance().get(&Symbol::new(&env, "admin")).unwrap();
        admin.require_auth();

        let mut stream: Stream = Self::get_stream(env.clone(), stream_id);

        if stream.status == StreamStatus::Disputed {
            panic_with_error!(&env, Error::DisputeAlreadyQueued);
        }
        if stream.status != StreamStatus::Active && stream.status != StreamStatus::Paused {
            panic_with_error!(&env, Error::StreamNotActive);
        }

        if recipient_amount < 0 || sender_amount < 0 {
            panic_with_error!(&env, Error::InvalidResolutionAmounts);
        }

        let total_resolution = recipient_amount.checked_add(sender_amount)
            .unwrap_or_else(|| panic_with_error!(&env, Error::ArithmeticOverflow));

        let escrowed_balance = stream.balance - stream.withdrawn_amount;
        if total_resolution <= 0 || total_resolution > escrowed_balance {
            panic_with_error!(&env, Error::InvalidResolutionAmounts);
        }

        let previous_status = stream.status;
        let was_active = stream.status == StreamStatus::Active;
        stream.status = StreamStatus::Disputed;

        env.storage().persistent().set(&stream_id, &stream);
        env.storage().persistent().extend_ttl(&stream_id, LEDGER_THRESHOLD, LEDGER_BUMP);

        if was_active {
            let mut protocol_metrics: ProtocolMetrics = env.storage().instance()
                .get(&Symbol::new(&env, "protocol_metrics"))
                .unwrap();
            protocol_metrics.total_active_streams = protocol_metrics.total_active_streams.saturating_sub(1);
            env.storage().instance().set(&Symbol::new(&env, "protocol_metrics"), &protocol_metrics);
            env.storage().instance().extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);
        }

        // Allocate a new dispute id
        let mut dispute_count: u64 = env.storage().instance().get(&Symbol::new(&env, "dispute_count")).unwrap_or(0);
        dispute_count += 1;
        let dispute_id = dispute_count;
        env.storage().instance().set(&Symbol::new(&env, "dispute_count"), &dispute_count);
        env.storage().instance().extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);

        let execute_after = env.ledger().timestamp() + DISPUTE_TIMELOCK_DELAY;

        let queued = QueuedResolution {
            dispute_id,
            stream_id,
            recipient_amount,
            sender_amount,
            execute_after,
            executed: false,
            previous_status,
        };

        let dispute_key = (Symbol::new(&env, "dispute"), dispute_id);
        env.storage().persistent().set(&dispute_key, &queued);
        env.storage().persistent().extend_ttl(&dispute_key, LEDGER_THRESHOLD, LEDGER_BUMP);

        let active_dispute_key = (stream_id, Symbol::new(&env, "active_dispute"));
        env.storage().persistent().set(&active_dispute_key, &dispute_id);
        env.storage().persistent().extend_ttl(&active_dispute_key, LEDGER_THRESHOLD, LEDGER_BUMP);

        env.events().publish(
            ("DisputeQueued", stream_id),
            DisputeQueuedEvent {
                dispute_id,
                stream_id,
                recipient_amount,
                sender_amount,
                execute_after,
            },
        );

        dispute_id
    }

    /// Execute a queued dispute resolution once its 48-hour timelock has
    /// elapsed. Callable by anyone, since the outcome and amounts were
    /// already fixed and authorized when the dispute was queued; only the
    /// passage of time gates execution.
    pub fn execute_resolution(env: Env, dispute_id: u64) {
        let dispute_key = (Symbol::new(&env, "dispute"), dispute_id);
        let mut queued: QueuedResolution = env.storage().persistent()
            .get(&dispute_key)
            .unwrap_or_else(|| panic_with_error!(&env, Error::DisputeNotFound));

        if queued.executed {
            panic_with_error!(&env, Error::DisputeAlreadyExecuted);
        }

        if env.ledger().timestamp() < queued.execute_after {
            panic_with_error!(&env, Error::TimelockNotElapsed);
        }

        let mut stream: Stream = Self::get_stream(env.clone(), queued.stream_id);

        let token_client = token::Client::new(&env, &stream.token);
        if queued.recipient_amount > 0 {
            token_client.transfer(&env.current_contract_address(), &stream.recipient, &queued.recipient_amount);
        }
        if queued.sender_amount > 0 {
            token_client.transfer(&env.current_contract_address(), &stream.sender, &queued.sender_amount);
        }

        stream.withdrawn_amount += queued.recipient_amount;
        stream.status = StreamStatus::Completed;

        env.storage().persistent().set(&queued.stream_id, &stream);
        env.storage().persistent().extend_ttl(&queued.stream_id, LEDGER_THRESHOLD, LEDGER_BUMP);

        queued.executed = true;
        env.storage().persistent().set(&dispute_key, &queued);
        env.storage().persistent().extend_ttl(&dispute_key, LEDGER_THRESHOLD, LEDGER_BUMP);

        env.storage().persistent().remove(&(queued.stream_id, Symbol::new(&env, "active_dispute")));

        env.events().publish(
            ("DisputeExecuted", queued.stream_id),
            DisputeExecutedEvent {
                dispute_id,
                stream_id: queued.stream_id,
                recipient_amount: queued.recipient_amount,
                sender_amount: queued.sender_amount,
            },
        );
    }

    /// Cancel a queued dispute resolution before its timelock elapses
    /// (admin/arbiter only), restoring the stream to its pre-dispute
    /// status, e.g. if new evidence emerges during the delay window.
    pub fn cancel_queued_resolution(env: Env, dispute_id: u64) {
        let admin: Address = env.storage().instance().get(&Symbol::new(&env, "admin")).unwrap();
        admin.require_auth();

        let dispute_key = (Symbol::new(&env, "dispute"), dispute_id);
        let queued: QueuedResolution = env.storage().persistent()
            .get(&dispute_key)
            .unwrap_or_else(|| panic_with_error!(&env, Error::DisputeNotFound));

        if queued.executed {
            panic_with_error!(&env, Error::DisputeAlreadyExecuted);
        }

        let mut stream: Stream = Self::get_stream(env.clone(), queued.stream_id);
        stream.status = queued.previous_status;

        env.storage().persistent().set(&queued.stream_id, &stream);
        env.storage().persistent().extend_ttl(&queued.stream_id, LEDGER_THRESHOLD, LEDGER_BUMP);

        if queued.previous_status == StreamStatus::Active {
            let mut protocol_metrics: ProtocolMetrics = env.storage().instance()
                .get(&Symbol::new(&env, "protocol_metrics"))
                .unwrap();
            protocol_metrics.total_active_streams += 1;
            env.storage().instance().set(&Symbol::new(&env, "protocol_metrics"), &protocol_metrics);
            env.storage().instance().extend_ttl(LEDGER_THRESHOLD, LEDGER_BUMP);
        }

        env.storage().persistent().remove(&dispute_key);
        env.storage().persistent().remove(&(queued.stream_id, Symbol::new(&env, "active_dispute")));

        env.events().publish(
            ("DisputeCanceled", queued.stream_id),
            DisputeCanceledEvent {
                dispute_id,
                stream_id: queued.stream_id,
            },
        );
    }

    /// Get a queued dispute resolution by id, if it exists
    pub fn get_queued_resolution(env: Env, dispute_id: u64) -> Option<QueuedResolution> {
        env.storage().persistent().get(&(Symbol::new(&env, "dispute"), dispute_id))
    }

    /// Get the id of the currently active (unresolved) dispute for a stream, if any
    pub fn get_active_dispute(env: Env, stream_id: u64) -> Option<u64> {
        env.storage().persistent().get(&(stream_id, Symbol::new(&env, "active_dispute")))
    }
}

mod test;