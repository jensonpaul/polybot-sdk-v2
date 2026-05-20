
#![allow(
    clippy::exhaustive_structs,
    clippy::exhaustive_enums,
    reason = "Alloy sol! macro generates code that triggers these lints"
)]

use alloy::primitives::ChainId;
use alloy::providers::Provider;
use alloy::sol;

use super::error::CtfError;
use super::types::{
    CollectionIdRequest, CollectionIdResponse, ConditionIdRequest, ConditionIdResponse,
    MergePositionsRequest, MergePositionsResponse, PositionIdRequest, PositionIdResponse,
    RedeemNegRiskRequest, RedeemNegRiskResponse, RedeemPositionsRequest, RedeemPositionsResponse,
    SplitPositionRequest, SplitPositionResponse,
};
use crate::{Result, contract_config};

sol! {
    #[sol(rpc)]
    interface IConditionalTokens {
        
        function prepareCondition(
            address oracle,
            bytes32 questionId,
            uint256 outcomeSlotCount
        ) external;

        function getConditionId(
            address oracle,
            bytes32 questionId,
            uint256 outcomeSlotCount
        ) external pure returns (bytes32);

        function getCollectionId(
            bytes32 parentCollectionId,
            bytes32 conditionId,
            uint256 indexSet
        ) external view returns (bytes32);

        function getPositionId(
            address collateralToken,
            bytes32 collectionId
        ) external pure returns (uint256);

        function splitPosition(
            address collateralToken,
            bytes32 parentCollectionId,
            bytes32 conditionId,
            uint256[] calldata partition,
            uint256 amount
        ) external;

        function mergePositions(
            address collateralToken,
            bytes32 parentCollectionId,
            bytes32 conditionId,
            uint256[] calldata partition,
            uint256 amount
        ) external;

        function redeemPositions(
            address collateralToken,
            bytes32 parentCollectionId,
            bytes32 conditionId,
            uint256[] calldata indexSets
        ) external;
    }

    #[sol(rpc)]
    interface INegRiskAdapter {
        
        function redeemPositions(
            bytes32 conditionId,
            uint256[] calldata amounts
        ) external;
    }
}

#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct Client<P: Provider> {
    contract: IConditionalTokens::IConditionalTokensInstance<P>,
    neg_risk_adapter: Option<INegRiskAdapter::INegRiskAdapterInstance<P>>,
    provider: P,
}

impl<P: Provider + Clone> Client<P> {
    
    pub fn new(provider: P, chain_id: ChainId) -> Result<Self> {
        let config = contract_config(chain_id, false).ok_or_else(|| {
            CtfError::ContractCall(format!(
                "CTF contract configuration not found for chain ID {chain_id}"
            ))
        })?;

        let contract = IConditionalTokens::new(config.conditional_tokens, provider.clone());

        Ok(Self {
            contract,
            neg_risk_adapter: None,
            provider,
        })
    }

    pub fn with_neg_risk(provider: P, chain_id: ChainId) -> Result<Self> {
        let config = contract_config(chain_id, true).ok_or_else(|| {
            CtfError::ContractCall(format!(
                "NegRisk contract configuration not found for chain ID {chain_id}"
            ))
        })?;

        let contract = IConditionalTokens::new(config.conditional_tokens, provider.clone());

        let neg_risk_adapter = config
            .neg_risk_adapter
            .map(|addr| INegRiskAdapter::new(addr, provider.clone()));

        Ok(Self {
            contract,
            neg_risk_adapter,
            provider,
        })
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "debug", skip(self), fields(
            oracle = %request.oracle,
            question_id = %request.question_id,
            outcome_slot_count = %request.outcome_slot_count
        ))
    )]
    pub async fn condition_id(&self, request: &ConditionIdRequest) -> Result<ConditionIdResponse> {
        let condition_id = self
            .contract
            .getConditionId(
                request.oracle,
                request.question_id,
                request.outcome_slot_count,
            )
            .call()
            .await
            .map_err(|e| CtfError::ContractCall(format!("Failed to get condition ID: {e}")))?;

        Ok(ConditionIdResponse { condition_id })
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "debug", skip(self), fields(
            parent_collection_id = %request.parent_collection_id,
            condition_id = %request.condition_id,
            index_set = %request.index_set
        ))
    )]
    pub async fn collection_id(
        &self,
        request: &CollectionIdRequest,
    ) -> Result<CollectionIdResponse> {
        let collection_id = self
            .contract
            .getCollectionId(
                request.parent_collection_id,
                request.condition_id,
                request.index_set,
            )
            .call()
            .await
            .map_err(|e| CtfError::ContractCall(format!("Failed to get collection ID: {e}")))?;

        Ok(CollectionIdResponse { collection_id })
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "debug", skip(self), fields(
            collateral_token = %request.collateral_token,
            collection_id = %request.collection_id
        ))
    )]
    pub async fn position_id(&self, request: &PositionIdRequest) -> Result<PositionIdResponse> {
        let position_id = self
            .contract
            .getPositionId(request.collateral_token, request.collection_id)
            .call()
            .await
            .map_err(|e| CtfError::ContractCall(format!("Failed to get position ID: {e}")))?;

        Ok(PositionIdResponse { position_id })
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "debug", skip(self), fields(
            collateral_token = %request.collateral_token,
            condition_id = %request.condition_id,
            amount = %request.amount
        ))
    )]
    pub async fn split_position(
        &self,
        request: &SplitPositionRequest,
    ) -> Result<SplitPositionResponse> {
        let pending_tx = self
            .contract
            .splitPosition(
                request.collateral_token,
                request.parent_collection_id,
                request.condition_id,
                request.partition.clone(),
                request.amount,
            )
            .send()
            .await
            .map_err(|e| {
                CtfError::ContractCall(format!("Failed to send split transaction: {e}"))
            })?;

        let transaction_hash = *pending_tx.tx_hash();

        let receipt = pending_tx
            .get_receipt()
            .await
            .map_err(|e| CtfError::ContractCall(format!("Failed to get split receipt: {e}")))?;

        Ok(SplitPositionResponse {
            transaction_hash,
            block_number: receipt.block_number.ok_or_else(|| {
                CtfError::ContractCall("Block number not available in receipt".to_owned())
            })?,
        })
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "debug", skip(self), fields(
            collateral_token = %request.collateral_token,
            condition_id = %request.condition_id,
            amount = %request.amount
        ))
    )]
    pub async fn merge_positions(
        &self,
        request: &MergePositionsRequest,
    ) -> Result<MergePositionsResponse> {
        let pending_tx = self
            .contract
            .mergePositions(
                request.collateral_token,
                request.parent_collection_id,
                request.condition_id,
                request.partition.clone(),
                request.amount,
            )
            .send()
            .await
            .map_err(|e| {
                CtfError::ContractCall(format!("Failed to send merge transaction: {e}"))
            })?;

        let transaction_hash = *pending_tx.tx_hash();

        let receipt = pending_tx
            .get_receipt()
            .await
            .map_err(|e| CtfError::ContractCall(format!("Failed to get merge receipt: {e}")))?;

        Ok(MergePositionsResponse {
            transaction_hash,
            block_number: receipt.block_number.ok_or_else(|| {
                CtfError::ContractCall("Block number not available in receipt".to_owned())
            })?,
        })
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "debug", skip(self), fields(
            collateral_token = %request.collateral_token,
            condition_id = %request.condition_id
        ))
    )]
    pub async fn redeem_positions(
        &self,
        request: &RedeemPositionsRequest,
    ) -> Result<RedeemPositionsResponse> {
        let pending_tx = self
            .contract
            .redeemPositions(
                request.collateral_token,
                request.parent_collection_id,
                request.condition_id,
                request.index_sets.clone(),
            )
            .send()
            .await
            .map_err(|e| {
                CtfError::ContractCall(format!("Failed to send redeem transaction: {e}"))
            })?;

        let transaction_hash = *pending_tx.tx_hash();

        let receipt = pending_tx
            .get_receipt()
            .await
            .map_err(|e| CtfError::ContractCall(format!("Failed to get redeem receipt: {e}")))?;

        Ok(RedeemPositionsResponse {
            transaction_hash,
            block_number: receipt.block_number.ok_or_else(|| {
                CtfError::ContractCall("Block number not available in receipt".to_owned())
            })?,
        })
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "debug", skip(self), fields(
            condition_id = %request.condition_id,
            amounts_len = request.amounts.len()
        ))
    )]
    pub async fn redeem_neg_risk(
        &self,
        request: &RedeemNegRiskRequest,
    ) -> Result<RedeemNegRiskResponse> {
        let adapter = self.neg_risk_adapter.as_ref().ok_or_else(|| {
            CtfError::ContractCall(
                "NegRisk adapter not available. Use Client::with_neg_risk() to enable NegRisk support".to_owned()
            )
        })?;

        let pending_tx = adapter
            .redeemPositions(request.condition_id, request.amounts.clone())
            .send()
            .await
            .map_err(|e| {
                CtfError::ContractCall(format!("Failed to send NegRisk redeem transaction: {e}"))
            })?;

        let transaction_hash = *pending_tx.tx_hash();

        let receipt = pending_tx.get_receipt().await.map_err(|e| {
            CtfError::ContractCall(format!("Failed to get NegRisk redeem receipt: {e}"))
        })?;

        Ok(RedeemNegRiskResponse {
            transaction_hash,
            block_number: receipt.block_number.ok_or_else(|| {
                CtfError::ContractCall("Block number not available in receipt".to_owned())
            })?,
        })
    }

    #[must_use]
    pub const fn provider(&self) -> &P {
        &self.provider
    }
}
