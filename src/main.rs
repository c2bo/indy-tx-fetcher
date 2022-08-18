mod ledger;

use log::{info};

fn main() {
    env_logger::init();

    let ledger = ledger::ledger::Ledger::new("idunion_test".to_string(), "https://raw.githubusercontent.com/IDunion/IDunion_TestNet_Genesis/master/pool_transactions_genesis".to_string(), "db".to_string());
    let ledger = ledger.unwrap();
    info!("Initialized network: {}", ledger.name);
    ledger.sync().unwrap();
    info!("Size after sync: {}", ledger.get_size().unwrap());
    let list = ledger.test_ordering().unwrap();
    info!("Got {} results", list.len());
    for finding in list {
        info!("[{}]: {}", finding.tx, finding)
    }
}
