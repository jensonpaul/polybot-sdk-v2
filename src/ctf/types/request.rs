
use alloy::primitives::{B256, U256};
use bon::Builder;

use crate::types::Address;

pub const BINARY_PARTITION: [u64; 2] = [1, 2];

#[non_exhaustive]
#[derive(Debug, Clone, Builder)]
pub struct ConditionIdRequest {
    
    pub oracle: Address,
    
    pub question_id: B256,
    
    pub outcome_slot_count: U256,
}

#[non_exhaustive]
#[derive(Debug, Clone, Builder)]
pub struct CollectionIdRequest {
    
    pub parent_collection_id: B256,
    
    pub condition_id: B256,
    
    pub index_set: U256,
}

#[non_exhaustive]
#[derive(Debug, Clone, Builder)]
pub struct PositionIdRequest {
    
    pub collateral_token: Address,
    
    pub collection_id: B256,
}

#[non_exhaustive]
#[derive(Debug, Clone, Builder)]
pub struct SplitPositionRequest {
    
    pub collateral_token: Address,
    
    #[builder(default)]
    pub parent_collection_id: B256,
    
    pub condition_id: B256,
    
    pub partition: Vec<U256>,
    
    pub amount: U256,
}

#[non_exhaustive]
#[derive(Debug, Clone, Builder)]
pub struct MergePositionsRequest {
    
    pub collateral_token: Address,
    
    #[builder(default)]
    pub parent_collection_id: B256,
    
    pub condition_id: B256,
    
    pub partition: Vec<U256>,
    
    pub amount: U256,
}

#[non_exhaustive]
#[derive(Debug, Clone, Builder)]
pub struct RedeemPositionsRequest {
    
    pub collateral_token: Address,
    
    #[builder(default)]
    pub parent_collection_id: B256,
    
    pub condition_id: B256,
    
    pub index_sets: Vec<U256>,
}

#[non_exhaustive]
#[derive(Debug, Clone, Builder)]
pub struct RedeemNegRiskRequest {
    
    pub condition_id: B256,
    
    pub amounts: Vec<U256>,
}

impl SplitPositionRequest {
    
    #[must_use]
    pub fn for_binary_market(collateral_token: Address, condition_id: B256, amount: U256) -> Self {
        Self {
            collateral_token,
            parent_collection_id: B256::default(),
            condition_id,
            partition: BINARY_PARTITION.iter().map(|&i| U256::from(i)).collect(),
            amount,
        }
    }
}

impl MergePositionsRequest {
    
    #[must_use]
    pub fn for_binary_market(collateral_token: Address, condition_id: B256, amount: U256) -> Self {
        Self {
            collateral_token,
            parent_collection_id: B256::default(),
            condition_id,
            partition: BINARY_PARTITION.iter().map(|&i| U256::from(i)).collect(),
            amount,
        }
    }
}

impl RedeemPositionsRequest {
    
    #[must_use]
    pub fn for_binary_market(collateral_token: Address, condition_id: B256) -> Self {
        Self {
            collateral_token,
            parent_collection_id: B256::default(),
            condition_id,
            index_sets: BINARY_PARTITION.iter().map(|&i| U256::from(i)).collect(),
        }
    }
}
