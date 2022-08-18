use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::error::Error;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TransactionReply {
    pub op: String,
    pub result: TransactionResult,
}

impl TransactionReply {
    pub fn from_string(raw: &str) -> Result<TransactionReply, Box<dyn Error>> {
        let res: Result<TransactionReply, serde_json::Error> = serde_json::from_str(raw);
        return match res {
            Ok(reply) => Ok(reply),
            Err(err) => Err(err.into()),
        };
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TransactionResult {
    pub identifier: String,
    pub req_id: u64,
    pub seq_no: u64,
    #[serde(rename = "type")]
    pub result_type: String,
    #[serde(rename = "state_proof")]
    pub state_proof: Value,
    pub data: ResultData,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResultData {
    pub ledger_size: u64,
    pub req_signature: Value,
    pub audit_path: Value,
    pub ver: String,
    pub txn: Transaction,
    pub root_hash: String,
    pub txn_metadata: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    #[serde(rename = "type")]
    pub tx_type: String,
    pub metadata: Value,
    pub data: Map<String, Value>,
}
