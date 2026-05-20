
use alloy::primitives::{B256, U256};
use bon::Builder;

#[non_exhaustive]
#[derive(Debug, Clone, Builder)]
pub struct ConditionIdResponse {
    
    pub condition_id: B256,
}

#[non_exhaustive]
#[derive(Debug, Clone, Builder)]
pub struct CollectionIdResponse {
    
    pub collection_id: B256,
}

#[non_exhaustive]
#[derive(Debug, Clone, Builder)]
pub struct PositionIdResponse {
    
    pub position_id: U256,
}

#[non_exhaustive]
#[derive(Debug, Clone, Builder)]
pub struct SplitPositionResponse {
    
    pub transaction_hash: B256,
    
    pub block_number: u64,
}

#[non_exhaustive]
#[derive(Debug, Clone, Builder)]
pub struct MergePositionsResponse {
    
    pub transaction_hash: B256,
    
    pub block_number: u64,
}

#[non_exhaustive]
#[derive(Debug, Clone, Builder)]
pub struct RedeemPositionsResponse {
    
    pub transaction_hash: B256,
    
    pub block_number: u64,
}

#[non_exhaustive]
#[derive(Debug, Clone, Builder)]
pub struct RedeemNegRiskResponse {
    
    pub transaction_hash: B256,
    
    pub block_number: u64,
}
