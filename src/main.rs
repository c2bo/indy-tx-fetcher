use futures::executor::block_on;
use indy_vdr::ledger::identifiers::RevocationRegistryId;
use indy_vdr::pool::helpers::{perform_ledger_request, perform_refresh};
use indy_vdr::pool::RequestResult::Reply;
use indy_vdr::pool::{Pool, PoolBuilder, PoolTransactions, RequestResult};
use log::info;
use reqwest::blocking;

fn main() {
    env_logger::init();
    let resp = blocking::get("https://raw.githubusercontent.com/IDunion/IDunion_TestNet_Genesis/master/pool_transactions_genesis").unwrap().text().unwrap();

    info!("Got: {}", resp);
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
    let request = req_builder.build_get_txn_request(None, 1, 843).unwrap();
    let (res, _) = block_on(perform_ledger_request(&pool, &request)).unwrap();
    match res {
        Reply(data) => {
            info!("TX 843: {}", data);
        }
        _ => {}
    }

    // Get Rev Reg
    let request = req_builder.build_get_revoc_reg_request(None, &RevocationRegistryId("ELMkCtYoz86qnJKeQqrL1M:4:ELMkCtYoz86qnJKeQqrL1M:3:CL:165:masterID Dev Rev:CL_ACCUM:a2813ebf-eb5d-47c0-b86a-a4ed97c89ec2".to_string()), 1615927210).unwrap();
    info!("Raw RevReg Request: {}",request.req_json.to_string());
    let (res, _) = block_on(perform_ledger_request(&pool, &request)).unwrap();
    match res {
        Reply(data) => {
            info!("RevReg {}", data);
        }
        _ => {}
    }

    let request = req_builder.build_get_revoc_reg_def_request(None, &RevocationRegistryId("ELMkCtYoz86qnJKeQqrL1M:4:ELMkCtYoz86qnJKeQqrL1M:3:CL:165:masterID Dev Rev:CL_ACCUM:a2813ebf-eb5d-47c0-b86a-a4ed97c89ec2".to_string())).unwrap();
    info!("Raw RevRegDef Request: {}",request.req_json.to_string());
    let (res, _) = block_on(perform_ledger_request(&pool, &request)).unwrap();
    match res {
        Reply(data) => {
            info!("RevRegDef {}", data);
        }
        _ => {}
    }
}
