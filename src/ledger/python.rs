use crate::ledger::ledger::RevRegState;
use log::error;
use serde_json::{Map, Value};
use std::error::Error;
use std::process::Command;

pub fn run_python(
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
        }
        REV_REG_STRATEGY_DEMAND => {
            output = output.args(["--strat_default", "False"]);
            output = output.arg("--issued_old");
            for number in rev_reg_state.issued.to_owned() {
                output = output.arg(format!("{}", number));
            }
        }
        _ => {
            error!("Unknown strategy: {}", rev_reg_state.strategy);
        }
    }

    output = output.arg("--issued_new");
    for number in value
        .get("issued")
        .map_or(&Vec::<Value>::new(), |x| x.as_array().unwrap())
    {
        output = output.arg(format!("{}", number.as_u64().unwrap()));
    }
    output = output.arg("--revoked_new");
    for number in value
        .get("revoked")
        .map_or(&Vec::<Value>::new(), |x| x.as_array().unwrap())
    {
        output = output.arg(format!("{}", number.as_u64().unwrap()));
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

pub const REV_REG_STRATEGY_DEFAULT: &str = "ISSUANCE_BY_DEFAULT";
pub const REV_REG_STRATEGY_DEMAND: &str = "ISSUANCE_ON_DEMAND";