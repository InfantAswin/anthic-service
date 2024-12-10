use std::env;
use std::ops::Add;
use std::str::FromStr;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use chrono::{TimeDelta, Utc};
use radix_common::math::{decimal, Decimal};
use radix_common::network::NetworkDefinition;
use radix_common::prelude::{Epoch, Instant, Secp256k1PrivateKey};
use radix_transactions::model::{IntentCoreV2, IntentHeaderV2, IntentSignatureV1, IntentSignaturesV2, NonRootSubintentSignaturesV2, NonRootSubintentsV2, PartialTransactionV2, PreparationSettingsV1, SignatureWithPublicKeyV1, SignedPartialTransactionV2, SubintentManifestV2, SubintentV2};
use radix_transactions::prelude::{HasSubintentHash, TransactionPayload};
use rand::RngCore;
use rand::rngs::ThreadRng;
use anthic_client::AnthicClient;
use anthic_model::{AnthicAccount, AnthicConfig, InstamintConfig};
use anthic_subintents::*;
use serde::{Deserialize, Serialize};

struct NewUserOrder {
    buy: TokenAmount,
    sell: TokenAmount,
}

#[derive(Debug, Deserialize, Serialize)]
struct ReturnStruct {
    data: String
}


#[derive(Debug, Deserialize, Serialize)]
pub struct InputStruct {
    buy_symbol: String,
    buy_amount: String,
    sell_symbol: String,
    sell_amount: String
}


pub async fn execute(_req: HttpRequest, input_data: web::Json<InputStruct>) -> impl Responder {
    
    println!("API called...");

    let network = NetworkDefinition::from_str("stokenet").unwrap();
    let trade_api_url = "https://trade-api.staging.anthic.io";
    let anthic_api_key = env::var("ANTHIC_API_KEY").expect("$ANTHIC_API_KEY is not set");

    // A high level Anthic client which wraps calls to the Anthic API
    let client = AnthicClient::new(
        network.clone(),
        trade_api_url.to_string(),
        anthic_api_key.to_string()
    );

    // Anthic configuration
    let anthic_config = client.load_anthic_config().await.unwrap();

    // Instamint configuration
    let instamint_config = client.load_instamint_config().await.unwrap();

    // Your account
    let anthic_account = client.load_anthic_account().await.unwrap();

    // The private key associated with your account
    let private_key = env::var("PRIVATE_KEY").expect("$PRIVATE_KEY is not set");

    let private_key = Secp256k1PrivateKey::from_hex(&private_key).unwrap();

    // A user order to receive 0.001 Test-xwBTC in exchange for 95.85 Test-xUSDC
    let user_order_to_fill = {
        let buy = TokenAmount {
            symbol: input_data.buy_symbol.to_string(),
            amount: decimal::Decimal::from_str(&input_data.buy_amount).unwrap(),
        };
        let sell = TokenAmount {
            symbol: input_data.sell_symbol.to_string(),
            amount: decimal::Decimal::from_str(&input_data.sell_amount).unwrap(),
        };

        NewUserOrder {
            buy,
            sell,
        }
    };

    // Create the manifest for the fill, in this case we will use instamint to mint the required Test-xwBTC
    let manifest = create_fill_manifest(&anthic_config, &instamint_config, &anthic_account, user_order_to_fill, true).unwrap();

    // Compose the subintent which includes the manifest just created as well as additional metadata info
    let subintent = {
        // The current epoch is required to create a valid subintent
        let cur_epoch = client.trade_api_client.network_status().await.unwrap().cur_epoch;

        // Anthic requires a minimum of 10 seconds expiry
        let expire_after_secs = 15;

        // random nonce
        let nonce = {
            let mut rng = ThreadRng::default();
            rng.next_u64()
        };

        create_fill_subintent(&network, manifest, expire_after_secs, cur_epoch, nonce)
    };

    // Sign the subintent hash
    let signature = {
        let hash = subintent
            .prepare(PreparationSettingsV1::latest_ref())
            .unwrap()
            .subintent_hash();
        let signature = private_key.sign(&hash);
        SignatureWithPublicKeyV1::Secp256k1 {
            signature,
        }
    };

    // Create the signed partial transaction which may be submitted to Anthic
    let signed_partial_transaction = create_signed_partial_transaction(subintent, signature);

    let data = signed_partial_transaction.to_raw().unwrap().to_hex();

    println!("Return Hash:{:?}", data);
    HttpResponse::Ok().json(
        ReturnStruct {
            data
        }
    )
}

fn create_fill_manifest(
    anthic_config: &AnthicConfig,
    instamint_config: &InstamintConfig,
    account: &AnthicAccount,
    new_user_order: NewUserOrder,
    use_instamint: bool,
) -> Result<SubintentManifestV2, String> {
    // Filling the user order requires a subintent which inverses the user order
    let buy = new_user_order.sell;
    let sell = new_user_order.buy;

    let mut builder = AnthicSubintentManifestBuilder::new(anthic_config.clone());

    // There is a flat solver fee which includes transaction execution fee, a portion of which will be rebated
    // in the transaction
    let settlement_fee_amount = anthic_config.settlement_fee_per_resource.get(&sell.symbol).unwrap().clone();
    // Maker fees are zero initially
    let anthic_fee_amount = Decimal::zero();

    if use_instamint {
        if let Some(local_id) = &account.instamint_customer_badge_local_id {
            let to_mint = TokenAmount {
                symbol: sell.symbol.clone(),
                amount: sell.amount + settlement_fee_amount + anthic_fee_amount
            };
            builder = builder.instamint_into_account(instamint_config, account.address, local_id.clone(), to_mint);
        } else {
            return Err("Cannot instamint without badge".to_string());
        }
    }

    let manifest = builder.add_anthic_limit_order(account.address, sell, buy, settlement_fee_amount, anthic_fee_amount).build();
    Ok(manifest)
}

fn create_fill_subintent(
    network_definition: &NetworkDefinition,
    manifest: SubintentManifestV2,
    expire_after_secs: i64,
    cur_epoch: u64,
    nonce: u64,
) -> SubintentV2 {
    let (instructions, blobs, children) = manifest.for_intent();
    let expiry_timestamp_secs = Utc::now().add(TimeDelta::seconds(expire_after_secs)).timestamp();

    SubintentV2 {
        intent_core: IntentCoreV2 {
            header: IntentHeaderV2 {
                network_id: network_definition.id,
                start_epoch_inclusive: Epoch::of(cur_epoch),
                end_epoch_exclusive: Epoch::of(cur_epoch + 2),
                min_proposer_timestamp_inclusive: None,
                max_proposer_timestamp_exclusive: Some(Instant::new(expiry_timestamp_secs)),
                intent_discriminator: nonce,
            },
            blobs,
            message: Default::default(),
            children,
            instructions,
        },
    }
}

fn create_signed_partial_transaction(subintent: SubintentV2, signature: SignatureWithPublicKeyV1) -> SignedPartialTransactionV2 {
    SignedPartialTransactionV2 {
        partial_transaction: PartialTransactionV2 {
            root_subintent: subintent,
            non_root_subintents: NonRootSubintentsV2(Default::default()),
        },
        root_subintent_signatures: IntentSignaturesV2 {
            signatures: vec![IntentSignatureV1(signature)],
        },
        non_root_subintent_signatures: NonRootSubintentSignaturesV2 {
            by_subintent: Default::default(),
        },
    }
}