use crate::ledger::transaction::TransactionReply;
use indy_vdr::pool::helpers::{perform_ledger_request, perform_refresh};
use indy_vdr::pool::RequestResult::Reply;
use indy_vdr::pool::{Pool, PoolBuilder, PoolTransactions, SharedPool};
use log::{debug, error, info, trace};
use reqwest;
use rocksdb::{IteratorMode, Options, DB};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::fmt::Formatter;
use std::path::Path;
use indy_vdr::ledger::identifiers::RevocationRegistryId;
use serde_json::Value;
use tokio::runtime::Runtime;
use crate::ledger::python::{REV_REG_STRATEGY_DEFAULT, REV_REG_STRATEGY_DEMAND, run_python};

pub struct Ledger {
    pub name: String,

    rt: Runtime,
    pool: SharedPool,
    db: DB,
}

const PYTHON_BIN_OLD: &str = "/usr/bin/python3.5";
const PYTHON_BIN_NEW: &str = "/usr/bin/python3.8";

const PYTHON_PATH_OLD: &str = "python-tests/revoked.py";
const PYTHON_PATH_NEW: &str = "python-tests/revoked_new.py";

impl Ledger {
    pub fn new(
        name: String,
        genesis_url: String,
        db_folder: String,
    ) -> Result<Ledger, Box<dyn Error>> {
        // initialize tokio runtime
        let rt = Runtime::new()?;

        // check genesis File and init indy-vdr pool
        let resp = rt.block_on(reqwest::get(genesis_url)).unwrap();
        let resp = rt.block_on(resp.text()).unwrap();

        let genesis_txns = PoolTransactions::from_json(resp.as_str()).unwrap();
        let pool_builder = PoolBuilder::default()
            .transactions(genesis_txns.to_owned())
            .unwrap();
        let pool = pool_builder.into_shared().unwrap();

        // refresh pool
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
                    trace!(
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
                            let res_old = run_python(rev_reg_state, tx_data.to_owned(), PYTHON_BIN_OLD, PYTHON_PATH_OLD).unwrap();
                            if !res_old.is_empty() {
                                debug!("[{}] Found REV_REG, checked order: {:?}", seq_no, res_old);
                            }
                            // check for unsorted
                            let mut sorted: Vec<u64> = res_old.to_owned();
                            sorted.sort();
                            let matching = sorted.eq(&res_old);
                            if !matching {
                                let value = tx_data.get("value").unwrap();
                                let issued= convert_vec(value.get("issued").map_or(Vec::<Value>::new(), |x| x.as_array().unwrap().to_owned()));
                                let revoked= convert_vec(value.get("revoked").map_or(Vec::<Value>::new(), |x| x.as_array().unwrap().to_owned()));
                                let problem = OrderingProblem {
                                    start_state: rev_reg_state.to_owned(),
                                    issued: issued.to_owned(),
                                    revoked: revoked.to_owned(),
                                    result: res_old.to_owned(),
                                    tx: seq_no,
                                };
                                problems.push(problem.to_owned());
                                // test against python3.8 with new python implementation
                                let res_new = run_python(rev_reg_state, tx_data.to_owned(), PYTHON_BIN_NEW, PYTHON_PATH_NEW).unwrap();
                                if res_old != res_new {
                                    error!("Results did not match - seqNo={}: {:?} !=  {:?}", seq_no, res_old, res_new);
                                    error!("{}", problem);

                                    // In this case, make sure we are doing things correctly -> get revregdelta from ledger for before/after this tx
                                    let request_builder = self.pool.get_request_builder();
                                    let id = tx_data.get("revocRegDefId").unwrap().as_str().unwrap();
                                    let timestamp = tx.result.data.txn_metadata.get("txnTime").unwrap().as_i64().unwrap();

                                    let request_before = request_builder.build_get_revoc_reg_delta_request(None, &RevocationRegistryId(id.to_string()), None, timestamp-1).unwrap();
                                    let (res_before, _) =  self.rt.block_on(perform_ledger_request(&self.pool, &request_before)).unwrap();
                                    let res_before = match res_before {
                                        Reply(data) => data,
                                        _ => "".to_string(),
                                    };
                                    info!("State on ledger before: {}", res_before);

                                    let request_after = request_builder.build_get_revoc_reg_delta_request(None, &RevocationRegistryId(id.to_string()), None, timestamp).unwrap();
                                    let (res_after,_) =  self.rt.block_on(perform_ledger_request(&self.pool, &request_after)).unwrap();
                                    let res_after = match res_after {
                                        Reply(data) => data,
                                        _ => "".to_string(),
                                    };
                                    info!("State on ledger after: {}", res_after);
                                }
                            }

                            // TODO: Should we sort here? otherwise we will keep on getting repeat results for a state that was already not sorted
                            let mut res = res_old.to_owned();
                            //res.sort();
                            match rev_reg_state.strategy.as_str() {
                                REV_REG_STRATEGY_DEFAULT => {
                                    rev_reg_state.revoked = res
                                },
                                REV_REG_STRATEGY_DEMAND => {
                                    rev_reg_state.issued = res
                                },
                                _ => {},
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
    pub tx: u64,
    pub start_state: RevRegState,
    pub issued: Vec<u64>,
    pub revoked: Vec<u64>,
    pub result: Vec<u64>,
}

impl OrderingProblem {
    pub fn issuance_by_default(&self) -> bool {
        return self.start_state.strategy.as_str() == REV_REG_STRATEGY_DEFAULT;
    }
}

impl fmt::Display for OrderingProblem {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.issuance_by_default() {
            write!(f, "[{}] revoked={:?}, issued_new={:?}, revoked_new={:?}, result={:?}", self.start_state.strategy, self.start_state.revoked, self.issued, self.revoked, self.result)
        } else {
            write!(f, "[{}] issued={:?}, issued_new={:?}, revoked_new={:?}, result={:?}", self.start_state.strategy, self.start_state.issued, self.issued, self.revoked, self.result)
        }
    }
}