pub mod validate;

use radix_common::prelude::*;
use radix_transactions::prelude::*;
use anthic_model::{AnthicConfig, InstamintConfig};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct TokenAmount {
    pub symbol: String,
    pub amount: Decimal,
}

pub struct AnthicSubintentManifestBuilder {
    config: AnthicConfig,
    builder: SubintentManifestV2Builder,
}

impl AnthicSubintentManifestBuilder {
    pub fn new(config: AnthicConfig) -> Self {
        Self {
            config,
            builder: SubintentManifestV2Builder::new_subintent_v2(),
        }
    }

    /// Add instructions to instamint a token into an instamint account
    pub fn instamint_into_account(
        mut self,
        instamint_config: &InstamintConfig,
        account: ComponentAddress,
        local_id: NonFungibleLocalId,
        to_mint: TokenAmount,
    ) -> Self {
        let resource = self.config.symbol_to_resource.get(&to_mint.symbol).unwrap().clone();

        self.builder = self.builder.create_proof_from_account_of_non_fungibles(account, instamint_config.customer_badge_resource, [local_id.clone()])
            .create_proof_from_auth_zone_of_non_fungibles(instamint_config.customer_badge_resource, [local_id], "instamint-proof")
            .with_name_lookup(|builder, lookup| {
                builder.call_method(
                    instamint_config.instamint_component,
                    "mint_to_account",
                    (resource, to_mint.amount, lookup.proof("instamint-proof"),),
                )
            });

        self
    }

    /// Add instructions for an anthic order
    pub fn add_anthic_limit_order(
        mut self,
        account: ComponentAddress,
        sell: TokenAmount,
        buy: TokenAmount,
        settlement_fee_amount: Decimal,
        anthic_fee_amount: Decimal,
    ) -> Self {
        let sell_resource = self.config.symbol_to_resource.get(&sell.symbol).unwrap().clone();
        let buy_resource = self.config.symbol_to_resource.get(&buy.symbol).unwrap().clone();
        let fee_resource = sell_resource;
        let withdraw_amount = sell.amount + settlement_fee_amount + anthic_fee_amount;

        self.builder = self.builder
            // This instruction ensures that the subintent is processed by Anthic before being committed
            .verify_parent(self.config.verify_parent_access_rule.clone())
            // Withdraw enough to cover fees and the swap
            .withdraw_from_account(account, sell_resource, withdraw_amount)
            // The following instructions perform the swap
            .take_from_worktop(sell_resource, sell.amount, "sell")
            .assert_next_call_returns_only(ManifestResourceConstraints::new().with(
                buy_resource,
                ManifestResourceConstraint::AtLeastAmount(buy.amount),
            ))
            .with_bucket("sell", |builder, bucket| builder.yield_to_parent((bucket,)))
            // The following instructions retrieve the fees
            .take_from_worktop(fee_resource, anthic_fee_amount, "anthic-fee")
            .take_from_worktop(fee_resource, settlement_fee_amount, "settlement-fee")
            .with_name_lookup(|builder, lookup| {
                builder.yield_to_parent((lookup.bucket("anthic-fee"), lookup.bucket("settlement-fee")))
            })
            // Everything is settled, deposit all resources into the account and yield to parent
            .deposit_entire_worktop(account)
            .yield_to_parent(());

        self
    }

    pub fn build(self) -> SubintentManifestV2 {
        self.builder.build()
    }
}
