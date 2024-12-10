use radix_common::math::Decimal;
use radix_common::prelude::*;
use radix_engine_interface::blueprints::account::{AccountWithdrawManifestInput, ACCOUNT_DEPOSIT_BATCH_IDENT, ACCOUNT_WITHDRAW_IDENT};
use radix_engine_interface::prelude::*;
use radix_transactions::manifest::*;
use radix_transactions::model::InstructionV2;
use radix_transactions::prelude::*;

pub fn anthic_validate_manifest(manifest: &SubintentManifestV2) -> Result<AnthicLimitOrderDefinition, String> {
    anthic_validate_instructions(&manifest.instructions)
}

pub fn anthic_validate_subintent(subintent: &SubintentV2) -> Result<AnthicLimitOrderDefinition, String> {
    anthic_validate_instructions(&subintent.intent_core.instructions.0)
}

pub fn anthic_validate_instructions(instructions: &Vec<InstructionV2>) -> Result<AnthicLimitOrderDefinition, String> {
    let mut state = SubintentValidatorState::new();

    for instruction in instructions {
        match SubintentValidator::process(&mut state, instruction) {
            Ok(_) => {}
            Err(error) => {
                return Err(error);
            }
        }
    }

    match state.qualified_state {
        SubintentValidatorQualifiedState::Complete { order } => Ok(order),
        _ => Err("Incomplete limit order manifest".to_string()),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct ResourceAmount {
    pub resource: ResourceAddress,
    pub amount: Decimal,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct FeeDefinition {
    pub resource: ResourceAddress,
    pub anthic_amount: Decimal,
    pub settlement_amount: Decimal,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct LimitOrderMeta {
    pub access_rule: AccessRule,
    pub account: ComponentAddress,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct LimitOrder {
    pub sell: ResourceAmount,
    pub buy: ResourceAmount,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct AnthicLimitOrderDefinition {
    pub meta: LimitOrderMeta,
    pub trade: LimitOrder,
    pub fee: FeeDefinition,
}

struct SubintentValidator;

impl SubintentValidator {
    fn process(
        state: &mut SubintentValidatorState,
        instruction: &InstructionV2,
    ) -> Result<(), String> {
        let cur_bucket = state.bucket_id;

        match instruction {
            InstructionV2::TakeFromWorktop(..) => state.bucket_id += 1,
            InstructionV2::TakeAllFromWorktop(..) => state.bucket_id += 1,
            InstructionV2::TakeNonFungiblesFromWorktop(..) => state.bucket_id += 1,
            _ => {}
        }

        match &state.qualified_state {
            SubintentValidatorQualifiedState::Initial => {
                match instruction {
                    InstructionV2::VerifyParent(VerifyParent { access_rule }) => {
                        state.qualified_state = SubintentValidatorQualifiedState::AccessRuleVerified {
                            access_rule: access_rule.clone(),
                        };
                    }
                    InstructionV2::YieldToParent(..) => {
                        return Err("Cannot yield to parent before VerifyParent".to_string());
                    }
                    _ => {}
                }

                Ok(())
            }
            SubintentValidatorQualifiedState::AccessRuleVerified { access_rule } => {
                if let InstructionV2::CallMethod(CallMethod {
                                                     address: DynamicGlobalAddress::Static(address),
                                                     method_name,
                                                     args,
                                                 }) = &instruction
                {
                    if address.as_node_id().is_global_account()
                        && method_name.eq(ACCOUNT_WITHDRAW_IDENT)
                    {
                        let withdraw: AccountWithdrawManifestInput =
                            manifest_decode(&manifest_encode(&args).unwrap())
                                .map_err(|err| format!("Decode error: {:?}", err))?;
                        if let ManifestResourceAddress::Static(resource) = withdraw.resource_address {
                            state.qualified_state =
                                SubintentValidatorQualifiedState::WithdrewFromAccount {
                                    meta: LimitOrderMeta {
                                        access_rule: access_rule.clone(),
                                        account: address.clone().try_into().unwrap(),
                                    },
                                    withdraw: ResourceAmount {
                                        resource,
                                        amount: withdraw.amount,
                                    },
                                };
                            return Ok(());
                        }
                    }
                }

                Err(format!(
                    "Expected Account Withdraw method but was: {:?}",
                    instruction
                ))
            }

            SubintentValidatorQualifiedState::WithdrewFromAccount { meta, withdraw } => {
                if let InstructionV2::TakeFromWorktop(TakeFromWorktop {
                                                          resource_address,
                                                          amount,
                                                      }) = &instruction
                {
                    if withdraw.amount >= *amount && withdraw.resource.eq(resource_address) {
                        state.qualified_state = SubintentValidatorQualifiedState::CreatedSellBucket {
                            meta: meta.clone(),
                            sell: ResourceAmount {
                                resource: *resource_address,
                                amount: *amount,
                            },
                            sell_bucket: ManifestBucket(cur_bucket),
                            leftover: withdraw.amount - *amount,
                        };
                        return Ok(());
                    }
                }

                Err(format!(
                    "Expected Equivalent Take From Worktop method but was: {:?}",
                    instruction
                ))
            }
            SubintentValidatorQualifiedState::CreatedSellBucket {
                meta,
                sell,
                sell_bucket,
                leftover,
            } => {
                match &instruction {
                    InstructionV2::AssertNextCallReturnsOnly(AssertNextCallReturnsOnly {
                                                                 constraints,
                                                             }) => {
                        if constraints.len() == 1 {
                            let (resource, constraint) = constraints.iter().next().unwrap();
                            match constraint {
                                ManifestResourceConstraint::AtLeastAmount(amount) => {
                                    state.qualified_state =
                                        SubintentValidatorQualifiedState::AssertedNextCallReturns {
                                            meta: meta.clone(),
                                            trade: LimitOrder {
                                                sell: sell.clone(),
                                                buy: ResourceAmount {
                                                    resource: *resource,
                                                    amount: *amount,
                                                },
                                            },
                                            sell_bucket: sell_bucket.clone(),
                                        };
                                    return Ok(());
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }

                Err(format!(
                    "Expected Take Bucket From Worktop method but was: {:?}",
                    instruction
                ))
            }

            SubintentValidatorQualifiedState::AssertedNextCallReturns {
                meta,
                trade,
                sell_bucket,
            } => {
                if let InstructionV2::YieldToParent(YieldToParent { args }) = &instruction {
                    if let Some(passed_to_parent) = try_manifest_args_to_bucket(args) {
                        if sell_bucket.eq(passed_to_parent) {
                            state.qualified_state =
                                SubintentValidatorQualifiedState::YieldedSellBucketToParent {
                                    meta: meta.clone(),
                                    trade: trade.clone(),
                                };
                            return Ok(());
                        }
                    }
                }

                Err(format!(
                    "Expected YieldToParent method but was: {:?}",
                    instruction
                ))
            }

            SubintentValidatorQualifiedState::YieldedSellBucketToParent { meta, trade } => {
                if let InstructionV2::TakeFromWorktop(TakeFromWorktop {
                                                          resource_address,
                                                          amount,
                                                      }) = &instruction
                {
                    state.qualified_state = SubintentValidatorQualifiedState::CreatedAnthicFeeBucket {
                        meta: meta.clone(),
                        trade: trade.clone(),
                        anthic_fee: ResourceAmount {
                            resource: *resource_address,
                            amount: *amount,
                        },
                        anthic_fee_bucket: ManifestBucket(cur_bucket),
                    };
                    return Ok(());
                }

                Err(format!(
                    "Expected Take From Worktop method but was: {:?}",
                    instruction
                ))
            }

            SubintentValidatorQualifiedState::CreatedAnthicFeeBucket {
                meta,
                trade,
                anthic_fee,
                anthic_fee_bucket,
            } => {
                if let InstructionV2::TakeFromWorktop(TakeFromWorktop {
                                                          resource_address,
                                                          amount,
                                                      }) = &instruction
                {
                    if resource_address.eq(&anthic_fee.resource) {
                        state.qualified_state =
                            SubintentValidatorQualifiedState::CreatedSettlementFeeBucket {
                                order: AnthicLimitOrderDefinition {
                                    meta: meta.clone(),
                                    trade: trade.clone(),
                                    fee: FeeDefinition {
                                        resource: *resource_address,
                                        anthic_amount: anthic_fee.amount,
                                        settlement_amount: *amount,
                                    },
                                },
                                anthic_fee_bucket: anthic_fee_bucket.clone(),
                                solver_fee_bucket: ManifestBucket(cur_bucket),
                            };
                        return Ok(());
                    }
                }

                Err(format!(
                    "Expected Take From Worktop but was: {:?}",
                    instruction
                ))
            }

            SubintentValidatorQualifiedState::CreatedSettlementFeeBucket {
                order,
                anthic_fee_bucket,
                solver_fee_bucket,
            } => {
                if let InstructionV2::YieldToParent(YieldToParent { args }) = &instruction {
                    if let Some((b0, b1)) = try_manifest_args_to_two_buckets(args) {
                        if b0.eq(anthic_fee_bucket) && b1.eq(solver_fee_bucket) {
                            state.qualified_state =
                                SubintentValidatorQualifiedState::YieldedFeesToParent {
                                    order: order.clone(),
                                };
                            return Ok(());
                        }
                    }
                }
                Err(format!(
                    "Expected Yield to parent but was: {:?}",
                    instruction
                ))
            }
            SubintentValidatorQualifiedState::YieldedFeesToParent { order } => {
                if let InstructionV2::CallMethod(CallMethod {
                                                     address: DynamicGlobalAddress::Static(address),
                                                     method_name,
                                                     args,
                                                 }) = &instruction
                {
                    if address.as_node_id().eq(order.meta.account.as_node_id())
                        && method_name.eq(ACCOUNT_DEPOSIT_BATCH_IDENT)
                    {
                        if args.eq(&ManifestValue::Tuple {
                            fields: vec![ManifestValue::Custom {
                                value: ManifestCustomValue::Expression(
                                    ManifestExpression::EntireWorktop,
                                ),
                            }],
                        }) {
                            state.qualified_state =
                                SubintentValidatorQualifiedState::DepositedWorktopToAccount {
                                    order: order.clone(),
                                };
                            return Ok(());
                        }
                    }
                }

                Err(format!(
                    "Expected Deposit Batch method but was: {:?}",
                    instruction
                ))
            }

            SubintentValidatorQualifiedState::DepositedWorktopToAccount { order } => {
                if let InstructionV2::YieldToParent(YieldToParent { .. }) = &instruction {
                    state.qualified_state = SubintentValidatorQualifiedState::Complete {
                        order: order.clone(),
                    };
                    return Ok(());
                }

                Err(format!(
                    "Expected Yield to Parent method but was: {:?}",
                    instruction
                ))
            }

            SubintentValidatorQualifiedState::Complete { .. } => {
                if let InstructionV2::YieldToParent(YieldToParent { .. }) = &instruction {
                    Err("Cannot yield to parent after order completion".to_string())
                } else {
                    Ok(())
                }
            }
        }
    }
}



#[derive(Debug)]
enum SubintentValidatorQualifiedState {
    Initial,
    AccessRuleVerified {
        access_rule: AccessRule,
    },
    WithdrewFromAccount {
        meta: LimitOrderMeta,
        withdraw: ResourceAmount,
    },
    CreatedSellBucket {
        meta: LimitOrderMeta,
        sell: ResourceAmount,
        sell_bucket: ManifestBucket,
        leftover: Decimal,
    },
    AssertedNextCallReturns {
        meta: LimitOrderMeta,
        trade: LimitOrder,
        sell_bucket: ManifestBucket,
    },
    YieldedSellBucketToParent {
        meta: LimitOrderMeta,
        trade: LimitOrder,
    },
    CreatedAnthicFeeBucket {
        meta: LimitOrderMeta,
        trade: LimitOrder,
        anthic_fee: ResourceAmount,
        anthic_fee_bucket: ManifestBucket,
    },
    CreatedSettlementFeeBucket {
        order: AnthicLimitOrderDefinition,
        anthic_fee_bucket: ManifestBucket,
        solver_fee_bucket: ManifestBucket,
    },
    YieldedFeesToParent {
        order: AnthicLimitOrderDefinition,
    },
    DepositedWorktopToAccount {
        order: AnthicLimitOrderDefinition,
    },
    Complete {
        order: AnthicLimitOrderDefinition,
    },
}

#[derive(Debug)]
struct SubintentValidatorState {
    bucket_id: u32,
    qualified_state: SubintentValidatorQualifiedState,
}

impl SubintentValidatorState {
    pub fn new() -> Self {
        Self {
            bucket_id: 0u32,
            qualified_state: SubintentValidatorQualifiedState::Initial,
        }
    }
}

fn try_manifest_args_to_bucket(args: &ManifestValue) -> Option<&ManifestBucket> {
    match args {
        ManifestValue::Tuple { fields } => {
            if fields.len() == 1 {
                let field = fields.iter().next().unwrap();
                match field {
                    Value::Custom {
                        value: ManifestCustomValue::Bucket(bucket),
                    } => {
                        return Some(bucket);
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }

    None
}

fn try_manifest_args_to_two_buckets(
    args: &ManifestValue,
) -> Option<(&ManifestBucket, &ManifestBucket)> {
    match args {
        ManifestValue::Tuple { fields } => {
            if fields.len() == 2 {
                let field0 = &fields[0];
                let field1 = &fields[1];
                match (field0, field1) {
                    (
                        Value::Custom {
                            value: ManifestCustomValue::Bucket(bucket0),
                        },
                        Value::Custom {
                            value: ManifestCustomValue::Bucket(bucket1),
                        },
                    ) => {
                        return Some((bucket0, bucket1));
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }

    None
}