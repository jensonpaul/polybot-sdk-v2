use alloy::primitives::U256;
use bon::Builder;
use serde::Deserialize;
use serde_with::{DisplayFromStr, serde_as};

use crate::types::{Address, ChainId, Decimal};

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize, PartialEq, Builder)]
pub struct DepositResponse {
    
    pub address: DepositAddresses,
    
    pub note: Option<String>,
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize, PartialEq, Builder)]
#[builder(on(String, into))]
pub struct DepositAddresses {
    
    pub evm: Address,
    
    pub svm: String,
    
    pub btc: String,
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize, PartialEq, Builder)]
#[serde(rename_all = "camelCase")]
pub struct SupportedAssetsResponse {
    
    pub supported_assets: Vec<SupportedAsset>,
    
    pub note: Option<String>,
}

#[non_exhaustive]
#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Builder)]
#[builder(on(String, into))]
#[serde(rename_all = "camelCase")]
pub struct SupportedAsset {
    
    #[serde_as(as = "DisplayFromStr")]
    pub chain_id: ChainId,
    
    pub chain_name: String,
    
    pub token: Token,
    
    pub min_checkout_usd: Decimal,
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize, PartialEq, Builder)]
#[builder(on(String, into))]
pub struct Token {
    
    pub name: String,
    
    pub symbol: String,
    
    pub address: String,
    
    pub decimals: u8,
}

#[non_exhaustive]
#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Builder)]
#[builder(on(String, into))]
#[serde(rename_all = "camelCase")]
pub struct StatusResponse {
    
    pub transactions: Vec<DepositTransaction>,
}

#[non_exhaustive]
#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Builder)]
#[builder(on(String, into))]
#[serde(rename_all = "camelCase")]
pub struct DepositTransaction {
    
    #[serde_as(as = "DisplayFromStr")]
    pub from_chain_id: ChainId,
    
    pub from_token_address: String,
    
    #[serde_as(as = "DisplayFromStr")]
    pub from_amount_base_unit: U256,
    
    #[serde_as(as = "DisplayFromStr")]
    pub to_chain_id: ChainId,
    
    pub to_token_address: Address,
    
    pub status: DepositTransactionStatus,
    
    pub tx_hash: Option<String>,
    
    pub created_time_ms: Option<u64>,
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DepositTransactionStatus {
    DepositDetected,
    Processing,
    OriginTxConfirmed,
    Submitted,
    Completed,
    Failed,
}

#[non_exhaustive]
#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Builder)]
#[builder(on(String, into))]
#[serde(rename_all = "camelCase")]
pub struct QuoteResponse {
    
    pub est_checkout_time_ms: u64,
    
    pub est_fee_breakdown: EstimatedFeeBreakdown,
    
    pub est_input_usd: f64,
    
    pub est_output_usd: f64,
    
    #[serde_as(as = "DisplayFromStr")]
    pub est_to_token_base_unit: U256,
    
    pub quote_id: String,
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize, PartialEq, Builder)]
#[builder(on(String, into))]
#[serde(rename_all = "camelCase")]
pub struct EstimatedFeeBreakdown {
    
    pub app_fee_label: String,
    
    pub app_fee_percent: f64,
    
    pub app_fee_usd: f64,
    
    pub fill_cost_percent: f64,
    
    pub fill_cost_usd: f64,
    
    pub gas_usd: f64,
    
    pub max_slippage: f64,
    
    pub min_received: f64,
    
    pub swap_impact: f64,
    
    pub swap_impact_usd: f64,
    
    pub total_impact: f64,
    
    pub total_impact_usd: f64,
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize, PartialEq, Builder)]
#[builder(on(String, into))]
pub struct WithdrawResponse {
    
    pub address: WithdrawalAddresses,
    
    pub note: String,
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize, PartialEq, Builder)]
#[builder(on(String, into))]
pub struct WithdrawalAddresses {
    
    pub evm: Address,
    
    pub svm: String,
    
    pub btc: String,
}
