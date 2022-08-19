mod ledger;

use std::fs;
use log::{debug, info};

fn main() {
    env_logger::init();

    let raw_networks = fs::read_to_string("./ledgers.json").unwrap();
    let mut networks: serde_json::Value = serde_json::from_str(raw_networks.as_str()).unwrap();
    let networks = networks.as_object_mut().unwrap();

    info!("Starting to query networks");
    for (network, url) in networks {
        info!("Initializing network: {}", network);
        let ledger = ledger::ledger::Ledger::new(network.to_string(), url.as_str().unwrap().to_string(), "db".to_string());
        let ledger = ledger.unwrap();
        info!("Initialized network: {}", ledger.name);
        ledger.sync().unwrap();
        info!("Size after sync: {}", ledger.get_size().unwrap());
        let list = ledger.test_ordering().unwrap();
        info!("Got {} results", list.len());
        for finding in list {
            debug!("[{}]: {}", finding.tx, finding)
        }
    }
}
