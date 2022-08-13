use futures::executor::block_on;
use indy_vdr::pool::helpers::{perform_ledger_request, perform_refresh};
use indy_vdr::pool::RequestResult::Reply;
use indy_vdr::pool::{Pool, PoolBuilder, PoolTransactions};
use log::{debug, error, info};
use reqwest::blocking;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::error::Error;
use std::process::Command;
use indy_vdr::ledger::identifiers::RevocationRegistryId;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TransactionReply {
    op: String,
    result: TransactionResult,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TransactionResult {
    identifier: String,
    req_id: u64,
    seq_no: u64,
    #[serde(rename = "type")]
    result_type: String,
    #[serde(rename = "state_proof")]
    state_proof: Value,
    data: ResultData,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ResultData {
    ledger_size: u64,
    req_signature: Value,
    audit_path: Value,
    ver: String,
    txn: Transaction,
    root_hash: String,
    txn_metadata: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Transaction {
    #[serde(rename = "type")]
    tx_type: String,
    metadata: Value,
    data: Map<String, Value>,
}

fn parse(raw: &str) -> serde_json::Result<TransactionReply> {
    let res = serde_json::from_str(raw);
    return res;
}

const REV_REG_DEF: &str = "113";
const REV_REG_ENTRY: &str = "114";

const REV_REG_STRATEGY_DEFAULT: &str = "ISSUANCE_BY_DEFAULT";
const REV_REG_STRATEGY_DEMAND: &str = "ISSUANCE_ON_DEMAND";

struct RevRegState {
    strategy: String,
    revoked: Vec<u64>,
    issued: Vec<u64>,
}

fn build_transaction_history() {

}

fn run_python(
    rev_reg_state: &mut RevRegState,
    tx_data: Map<String, Value>,
) -> Result<Vec<u64>, Box<dyn Error>> {
    let value = tx_data.get("value").unwrap();

    if value.get("revoked").is_none() && value.get("issued").is_none() {
        return Ok(vec![]);
    }

    let mut cmd = Command::new("/usr/bin/python3.5");
    let mut output = cmd.arg("python-tests/revoked.py");
    match rev_reg_state.strategy.as_str() {
        REV_REG_STRATEGY_DEFAULT => {
            output = output.args(["--strat_default", "True"]);
            output = output.arg("--revoked_old");
            for number in rev_reg_state.revoked.to_owned() {
                output = output.arg(format!("{}", number));
            }
            output = output.arg("--issued_new");
            for number in value.get("issued").map_or(&Vec::<Value>::new(), |x| x.as_array().unwrap()) {
                output = output.arg(format!("{}", number.as_u64().unwrap()));
            }
            output = output.arg("--revoked_new");
            for number in value.get("revoked").map_or(&Vec::<Value>::new(), |x| x.as_array().unwrap()) {
                output = output.arg(format!("{}", number.as_u64().unwrap()));
            }
        }
        REV_REG_STRATEGY_DEMAND => {
            output = output.args(["--strat_default", "False"]);
            output = output.arg("--issued_old");
            for number in rev_reg_state.issued.to_owned() {
                output = output.arg(format!("{}", number));
            }
            output = output.arg("--issued_new");
            for number in value.get("issued").map_or(&Vec::<Value>::new(), |x| x.as_array().unwrap()) {
                output = output.arg(format!("{}", number.as_u64().unwrap()));
            }
            output = output.arg("--revoked_new");
            for number in value.get("revoked").map_or(&Vec::<Value>::new(), |x| x.as_array().unwrap()) {
                output = output.arg(format!("{}", number.as_u64().unwrap()));
            }
        }
        _ => {
            error!("Unknown strategy: {}", rev_reg_state.strategy);
        }
    }

    let out_raw = output.output().unwrap();
    let out = String::from_utf8_lossy(&out_raw.stdout).to_string();
    let res: Vec<u64> = out.split_whitespace().map(|x| x.parse().unwrap()).collect();

    match rev_reg_state.strategy.as_str() {
        REV_REG_STRATEGY_DEFAULT => {
            rev_reg_state.revoked = res.to_owned();
        }
        REV_REG_STRATEGY_DEMAND => {
            rev_reg_state.issued = res.to_owned();
        }
        _ => {}
    }
    Ok(res)
}

fn main() {
    env_logger::init();
    // idunion test
     let genesis_url = "https://raw.githubusercontent.com/IDunion/IDunion_TestNet_Genesis/master/pool_transactions_genesis".to_string();
    // sovrin main
    // let genesis_url = "https://sovrin-mainnet-browser.vonx.io/genesis".to_string();
    let resp = blocking::get(genesis_url).unwrap().text().unwrap();

    let genesis_txns = PoolTransactions::from_json(resp.as_str()).unwrap();
    // Initialize pool
    let pool_builder = PoolBuilder::default()
        .transactions(genesis_txns.to_owned())
        .unwrap();
    let pool = pool_builder.into_shared().unwrap();

    // Refresh pool
    let (txns, _timing) = block_on(perform_refresh(&pool)).unwrap();

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
    info!("Updated pool, ready to fetch txns");
    let req_builder = pool.get_request_builder();

    // Some manual tests to verify this issue exists on the ledgers
    // sovrin mainnet
    //let sovreq = req_builder.build_get_revoc_reg_delta_request(None,&RevocationRegistryId("NW4mH3ybqwTnUZ8xjtoCR8:4:NW4mH3ybqwTnUZ8xjtoCR8:3:CL:54767:dea:CL_ACCUM:TAG1".to_string()), None, 1607024454).unwrap();
    //let (sovres, _) = block_on(perform_ledger_request(&pool, &sovreq)).unwrap();
    //error!("{:?}", sovres);
    // idunion
    let sovreq = req_builder.build_get_revoc_reg_delta_request(None,&RevocationRegistryId("3QowxFtwciWceMFr7WbwnM:4:3QowxFtwciWceMFr7WbwnM:3:CL:2016:BankAccount:CL_ACCUM:6f045d8c-2621-47d2-83b8-289339320eae".to_string()), None, 1628490657).unwrap();
    let (sovres, _) = block_on(perform_ledger_request(&pool, &sovreq)).unwrap();
    error!("{:?}", sovres);

    let mut seq_no: u64 = 1;
    let mut ledger_size: Option<u64> = None;
    let mut revocation_state: HashMap<String, RevRegState> = HashMap::new();
    let mut problematic_txs: Vec<u64> = vec![];
    while ledger_size.is_none() || seq_no < ledger_size.unwrap() {
        let request = req_builder.build_get_txn_request(None, 1, seq_no as i32).unwrap();
        let (res, _) = block_on(perform_ledger_request(&pool, &request)).unwrap();
        let tx_raw = match res {
            Reply(data) => data,
            _ => {
                continue;
            }
        };
        debug!("TX {}: {}", seq_no, tx_raw);
        let tx = parse(&tx_raw).unwrap();
        if ledger_size.is_none() {
            ledger_size = Some(tx.result.data.ledger_size);
            info!("Updated Ledger size to: {}", ledger_size.unwrap())
        }

        // Only do something if revocation registries are concerned
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
                        strategy: strategy,
                        revoked: vec![],
                        issued: vec![],
                    },
                );
            }
            REV_REG_ENTRY => {
                let key = tx.result.data.txn.data["revocRegDefId"].as_str().unwrap().to_string();
                let tx_data = tx.result.data.txn.data;
                debug!(
                    "[{}] Found a REV_REG_ENTRY transaction: {}",
                    tx.result.seq_no, key
                );
                let rev_reg_def = revocation_state.get_mut(&key);
                match rev_reg_def {
                    None => {
                        error!("Could not find REV_REG_DEF for REV_REG_ENTRY: {}", key);
                        continue;
                    }
                    Some(rev_reg_state) => {
                        let res = run_python(rev_reg_state, tx_data).unwrap();
                        if !res.is_empty() {
                            info!("[{}] Found REV_REG, checked order: {:?}", seq_no, res);
                        }
                        let mut sorted: Vec<u64> = res.to_owned();
                        sorted.sort();
                        let matching = sorted.eq(&res);
                        if !matching {
                            error!("Found transaction with seqNo={}: {}", seq_no, tx_raw);
                            problematic_txs.push(seq_no.to_owned());
                        }
                    }
                }
            }
            _ => {}
        }
        if seq_no % 100 == 0 {
            info!("Reached seqNo {}", seq_no)
        }
        seq_no = seq_no + 1;
    }
    if !problematic_txs.is_empty() {
        error!("Found {} transactions that might have problems with ordering:", problematic_txs.len());
        error!("{:?}", problematic_txs);
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Map, Number, Value};
    use crate::{parse, RevRegState, run_python};

    #[test]
    fn test_cmd() {
        let state = &mut RevRegState{
            issued: vec![],
            revoked: vec![1,5,6,7],
            strategy: "ISSUANCE_BY_DEFAULT".to_string(),
        };
        let mut tx_data =  Map::new();
        let mut value = Map::new();
        value.insert("revoked".to_string(), Value::Array(vec![Value::Number(Number::from(8))]));
        tx_data.insert("value".to_string(), Value::from(value));
        let res = run_python(state, tx_data);
        assert_eq!(res.unwrap(), vec![8,1,5,6,7]);
    }
    #[test]
    fn deserialize() -> Result<(), String> {
        let raw_tx = r#"
{
    "op": "REPLY",
    "result": {
        "reqId": 1660338622948940257,
        "seqNo": 1147,
        "type": "3",
        "data": {
            "txn": {
                "protocolVersion": 2,
                "metadata": {
                    "digest": "6d00208044bcfa39cb53b16e698d5ad770925c97730395d160041c81f4c6315a",
                    "from": "ELMkCtYoz86qnJKeQqrL1M",
                    "reqId": 1616591033032699553,
                    "payloadDigest": "6ad5d27090f5927960b7c2165fae3032e1a55d70865c802cd80872023fead74d"
                },
                "data": {
                    "value": {
                        "prevAccum": "21 143F811DD2BA598568D8FD9E633017B31D3783CF913BC70B707202D127F1A40D0 21 1143E6DFE8BE888FC58FCC4561FEDC53A71F3B8B068C4C8FA22B8C35CD4603272 6 5848065DA0998DB1CD028D41ABE40B901CBA58F655D06C12171C17CF74EC3CD9 4 18CA39FC13C1CA4D0748186235CC77828283C8394A5CE2DFF5A1985B6315F652 6 85F3FD2953868D0CD4FD817A9A374CD4D8BA68C2EA4D95F780501E37F2BE0B60 4 20B46EEF4A93946768F209D2D125B2565A132485C54571D1CAD93A4A5E023BAD",
                        "accum": "21 11A5DC259C657E09EE151930B37C6B1FF81D1777D8A69C06E8212E2516F167FC5 21 13547ABE5A0861B995F041804C9DA3377CEFCC728454A710D0311D2AAE2589E3B 6 62648CD79153392C113FC3AC64284300631F5852A0D7CE42FAED7BD257BD16CA 4 21135E0030AC34A879742C3098C89A09FF7056C68D55B9247F6D6466BD493073 6 618C8E09C4A767DF67D42D3D3A752B93D734B8078450EFB4D460EAF67D30F13F 4 46AB17D0CDAC5803B1357CB2B7880EB1D39C2DF0E8699ADCD1BE294E4F13C43A",
                        "revoked": [
                            462
                        ]
                    },
                    "revocRegDefId": "ELMkCtYoz86qnJKeQqrL1M:4:ELMkCtYoz86qnJKeQqrL1M:3:CL:165:masterID Dev Rev:CL_ACCUM:4cd46f58-10da-4a8c-b311-6e7b6402ee42",
                    "revocDefType": "CL_ACCUM"
                },
                "type": "114"
            },
            "reqSignature": {
                "values": [
                    {
                        "from": "ELMkCtYoz86qnJKeQqrL1M",
                        "value": "5AWApogVnJmw85E7bM1RPhZRAviu599wHDG8FAX3ANXEgG2xg8JnXAC6CGJ7fCgqcSiZRysFixfhYLRsxfJ2AcEN"
                    }
                ],
                "type": "ED25519"
            },
            "txnMetadata": {
                "seqNo": 1147,
                "txnId": "5:ELMkCtYoz86qnJKeQqrL1M:4:ELMkCtYoz86qnJKeQqrL1M:3:CL:165:masterID Dev Rev:CL_ACCUM:4cd46f58-10da-4a8c-b311-6e7b6402ee42",
                "txnTime": 1616591033
            },
            "ledgerSize": 14447,
            "ver": "1",
            "rootHash": "6Jk59orRGXfxLDNuN4ytauEggqX4JRSuKBavms6uJuQR",
            "auditPath": [
                "Dpo8Qw8tGbwddKhjkUKVPHdNrFGyapGd6uASrqoRWKgk",
                "EAR37FpYZMXydRaQQDzSUp4PueY1LrFZsAkA2df36ycc",
                "45KrXBahZKNXRv9r9aD9JSxB6gTeyforq6gFj4L2CP85",
                "Gnu1tHWX74S13v7rpo1qkGyrA5izGcVS9n3GCdoeioaq",
                "3QRKAB2LmPe5QmjwdLEXZaWFRyajWMjSKLjLeHWJ29fp",
                "3maCbRzQ5DRgjL4otpjJiYyGMSzsiBqVrfkMgBn2eJya",
                "AS21T7B2JLCud34wxDUHwPpnW84mSLyGKv7QQbNUtN3D",
                "5Vkh7etEetsNVSVSF7YwWjyoG6qUVxNFyJivYDbFWF3S",
                "astG4S7TTCeNYA77nSKMkh44Zfr1xycPYNrtvNs3tqE",
                "D55swyeA7nX9x26wRzg7sGPBvWQvWz6hWC9D7qbNE1Yb",
                "uPGNU4bkQRuHroV8JUt7eE5ymsGqPFVfTeS3CrvC5HM",
                "HpfigmHLoo9ZuMCWLJ9BK6nY5RM1BgvLX3G5oTwkVUY5",
                "9kDxFJ48JkNtNcDofFsoCx4HrRHX7wKEMir3thuh95PU",
                "A3uUdJw8MjBofru2TmYDvwidBgKmVGseY47mqhGQWn6N"
            ]
        },
        "state_proof": {
            "multi_signature": {
                "signature": "RbDdTgSjDa2MLtKporG6A3sZLLURjkhy3jpwLzn8gFAF1JG6uwK9ukebSAPTXqcouUX8Eo8UK33H61aecmcpvF9harWrXxf3Ce7uoCZkdisd9sxTaU6sXJvmTSssEZSYWUx8X1d5iBPLaHmwSSftcELXomhAQexjavpVrr3ZRE8cvj",
                "value": {
                    "timestamp": 1660338530,
                    "state_root_hash": "C2LZfa1BKD2XNTjHXcdDT3naHqWhSpbKHNJq433L4BFT",
                    "pool_state_root_hash": "AVRsdoXHo9k3yRABWz8SdrJVzzHynUwtJj9ok8s4dyzM",
                    "txn_root_hash": "6Jk59orRGXfxLDNuN4ytauEggqX4JRSuKBavms6uJuQR",
                    "ledger_id": 1
                },
                "participants": [
                    "Commerzbank",
                    "regio_iT",
                    "T-Labs",
                    "Bundesdruckerei",
                    "GS1Germany",
                    "MainIncubator",
                    "DeutscheBahn",
                    "DATEV01",
                    "Swisscom-node",
                    "Spherity_GmbH",
                    "Bosch",
                    "mgm_tp",
                    "tubzecm"
                ]
            }
        },
        "identifier": "LibindyDid111111111111"
    }
}
"#;
        let tx = parse(raw_tx);
        if tx.is_err() {
            return Err(tx.err().unwrap().to_string());
        }
        Ok(())
    }
}
