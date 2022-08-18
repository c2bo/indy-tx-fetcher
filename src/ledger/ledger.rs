use crate::ledger::transaction::TransactionReply;
use indy_vdr::pool::helpers::{perform_ledger_request, perform_refresh};
use indy_vdr::pool::RequestResult::Reply;
use indy_vdr::pool::{Pool, PoolBuilder, PoolTransactions, SharedPool};
use log::{debug, error, info};
use reqwest::blocking;
use rocksdb::{IteratorMode, Options, DB};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;
use serde_json::Value;
use tokio::runtime::Runtime;
use crate::ledger::python::run_python;

pub struct Ledger {
    pub name: String,

    rt: Runtime,
    pool: SharedPool,
    db: DB,
}

impl Ledger {
    pub fn new(
        name: String,
        genesis_url: String,
        db_folder: String,
    ) -> Result<Ledger, Box<dyn Error>> {
        // initialize tokio runtime
        let rt = Runtime::new()?;

        // check genesis File and init indy-vdr pool
        let resp = blocking::get(genesis_url).unwrap().text().unwrap();
        let genesis_txns = PoolTransactions::from_json(resp.as_str()).unwrap();
        let pool_builder = PoolBuilder::default()
            .transactions(genesis_txns.to_owned())
            .unwrap();
        let pool = pool_builder.into_shared().unwrap();
        let (txns, _timing) = rt.block_on(perform_refresh(&pool)).unwrap();
        let pool = if let Some(txns) = txns {
            let builder = {
                let mut pool_txns = genesis_txns;
                pool_txns.extend_from_json(&txns).unwrap();
                PoolBuilder::default()
                    .transactions(pool_txns.clone())
                    .unwrap()
            };
            builder.into_shared().unwrap()
        } else {
            pool
        };

        // initialize rocksdb
        let mut db_options = Options::default();
        db_options.create_if_missing(true);
        let db_path = Path::new(&db_folder).join(&name);

        let db = DB::open(&db_options, db_path)?;

        Ok(Ledger { name, rt, pool, db })
    }

    pub fn get_size(&self) -> Result<u64, Box<dyn Error>> {
        let iter = self.db.iterator(IteratorMode::Start);
        let last = iter.last();
        if last.is_none() {
            return Err("Unknown".into());
        }
        let key: [u8; 8] = last.unwrap().unwrap().0.as_ref().try_into().unwrap();
        Ok(u64::from_be_bytes(key))
    }

    fn get(&self, key: u64) -> Result<String, Box<dyn Error>> {
        let res = self.db.get(key.to_be_bytes())?;
        return match res {
            Some(data) => Ok(String::from_utf8_lossy(&data).to_string()),
            _ => Err("Not found".into()),
        };
    }

    fn put(&self, key: u64, value: &str) -> Result<(), Box<dyn Error>> {
        let res = self.db.put(key.to_be_bytes(), value.as_bytes());
        return match res {
            Ok(x) => Ok(x),
            Err(err) => Err(err.into()),
        };
    }

    pub fn sync(&self) -> Result<u64, Box<dyn Error>> {
        let req_builder = self.pool.get_request_builder();

        let mut seq_no: u64 = 1;
        let cur_size = self.get_size();
        match cur_size {
            Ok(val) => {
                seq_no = val;
            }
            _ => {}
        }

        let mut ledger_size: u64 = seq_no;

        info!("Starting sync at seqNo={}", seq_no);
        while seq_no <= ledger_size {
            let request = req_builder
                .build_get_txn_request(None, 1, seq_no as i32)
                .unwrap();
            let (res, _) = self
                .rt
                .block_on(perform_ledger_request(&self.pool, &request))
                .unwrap();
            let tx_raw = match res {
                Reply(data) => data,
                _ => {
                    continue;
                }
            };
            debug!("Got transaction [{}]: {}", seq_no, tx_raw);
            // Try to parse as a form of validation
            let tx = TransactionReply::from_string(&tx_raw);
            let tx = match tx {
                Err(err) => return Err(err),
                Ok(reply) => reply,
            };

            if ledger_size != tx.result.data.ledger_size {
                ledger_size = tx.result.data.ledger_size;
            }
            self.put(seq_no, tx_raw.as_str())?;

            if seq_no % 100 == 0 {
                info!("Reached seqNo={}", seq_no)
            }
            seq_no = seq_no + 1;
        }
        Ok(ledger_size)
    }

    pub fn test_ordering(&self) -> Result<Vec<OrderingProblem>, Box<dyn Error>> {
        let cur_size = self.get_size()?;
        let mut problems: Vec<OrderingProblem> = Vec::<OrderingProblem>::new();
        let mut revocation_state: HashMap<String, RevRegState> = HashMap::new();

        for seq_no in 1..cur_size {
            let tx_raw = self.get(seq_no)?;
            let tx = TransactionReply::from_string(tx_raw.as_str())?;
            match tx.result.data.txn.tx_type.as_str() {
                REV_REG_DEF => {
                    let key = tx.result.data.txn.data["id"].to_owned();
                    let tx_data = tx.result.data.txn.data;
                    let strategy: String = tx_data["value"]["issuanceType"].as_str().unwrap().to_string();
                    debug!(
                        "[{}] Found a REV_REG_DEF transaction: {}",
                        tx.result.seq_no, key
                    );
                    revocation_state.insert(
                        key.as_str().unwrap().to_string(),
                        RevRegState {
                            strategy,
                            revoked: vec![],
                            issued: vec![],
                        },
                    );
                }
                REV_REG_ENTRY => {
                    let key = tx.result.data.txn.data["revocRegDefId"]
                        .as_str()
                        .unwrap()
                        .to_string();
                    let tx_data = tx.result.data.txn.data;
                    let rev_reg_def = revocation_state.get_mut(&key);
                    match rev_reg_def {
                        None => {
                            error!("Could not find REV_REG_DEF for REV_REG_ENTRY: {}", key);
                            // continue;
                        }
                        Some(rev_reg_state) => {
                            let res = run_python(rev_reg_state, tx_data.to_owned()).unwrap();
                            if !res.is_empty() {
                                debug!("[{}] Found REV_REG, checked order: {:?}", seq_no, res);
                            }
                            let mut sorted: Vec<u64> = res.to_owned();
                            sorted.sort();
                            let matching = sorted.eq(&res);
                            if !matching {
                                let value = tx_data.get("value").unwrap();
                                let issued= convert_vec(value.get("issued").map_or(Vec::<Value>::new(), |x| x.as_array().unwrap().to_owned()));
                                let revoked= convert_vec(value.get("revoked").map_or(Vec::<Value>::new(), |x| x.as_array().unwrap().to_owned()));
                                problems.push(OrderingProblem {
                                    start_state: rev_reg_state.to_owned(),
                                    issued: issued.to_owned(),
                                    revoked: revoked.to_owned(),
                                    result: res.to_owned(),
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(problems)
    }
}

fn convert_vec(vec: Vec<Value>) -> Vec<u64> {
    let mut output: Vec<u64> = Vec::<u64>::new();
    for number in vec {
        let x = number.as_u64().unwrap();
        output.push(x);
    }
    output
}

pub const REV_REG_DEF: &str = "113";
pub const REV_REG_ENTRY: &str = "114";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RevRegState {
    pub strategy: String,
    pub revoked: Vec<u64>,
    pub issued: Vec<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OrderingProblem {
    start_state: RevRegState,
    issued: Vec<u64>,
    revoked: Vec<u64>,
    result: Vec<u64>,
}
