use alloy::primitives::{ChainId, U256};
use bon::Builder;
use serde::Serialize;
use serde_with::{DisplayFromStr, serde_as};

use crate::types::Address;

#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Builder)]
pub struct DepositRequest {
    
    pub address: Address,
}

#[non_exhaustive]
#[derive(Debug, Clone, Builder)]
#[builder(on(String, into))]
pub struct StatusRequest {
    pub address: String,
}

#[non_exhaustive]
#[serde_as]
#[derive(Debug, Clone, Serialize, Builder)]
#[builder(on(String, into))]
#[serde(rename_all = "camelCase")]
pub struct QuoteRequest {
    
    #[serde_as(as = "DisplayFromStr")]
    pub from_amount_base_unit: U256,
    
    #[serde_as(as = "DisplayFromStr")]
    pub from_chain_id: ChainId,
    
    pub from_token_address: String,
    
    pub recipient_address: String,
    
    #[serde_as(as = "DisplayFromStr")]
    pub to_chain_id: ChainId,
    
    pub to_token_address: String,
}

#[non_exhaustive]
#[serde_as]
#[derive(Debug, Clone, Serialize, Builder)]
#[builder(on(String, into))]
#[serde(rename_all = "camelCase")]
pub struct WithdrawRequest {
    
    pub address: Address,
    
    #[serde_as(as = "DisplayFromStr")]
    pub to_chain_id: ChainId,
    
    pub to_token_address: String,
    
    pub recipient_addr: String,
}
