use radix_common::prelude::*;
use radix_engine_interface::prelude::*;

/// Anthic configuration
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnthicConfig {
    /// This access rule is used to ensure that Anthic processes a subintent before being committed on-ledger.
    pub verify_parent_access_rule: AccessRule,
    /// The mapping from Anthic token symbols to on-ledger resource addresses
    pub symbol_to_resource: HashMap<String, ResourceAddress>,
    /// Each subintent submitted requires a flat solver fee, a portion of which will be rebated.
    pub settlement_fee_per_resource: HashMap<String, Decimal>,
    /// The taker fee in percentage for a given level
    pub anthic_fee_per_level: Vec<AnthicLevelFee>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnthicLevelFee {
    pub taker_fee: Decimal,
    pub maker_fee: Decimal,
}

/// Instamint configuration
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstamintConfig {
    /// The resource address which is used for customer badges
    pub customer_badge_resource: ResourceAddress,
    /// The address of the instamint component which is called to instamint resources
    pub instamint_component: ComponentAddress,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnthicAccount {
    pub address: ComponentAddress,
    pub instamint_customer_badge_local_id: Option<NonFungibleLocalId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnthicAddressInfo {
    pub level: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OnLedgerAccount {
    pub address: ComponentAddress,
    pub balances: HashMap<ResourceAddress, Decimal>,
}
