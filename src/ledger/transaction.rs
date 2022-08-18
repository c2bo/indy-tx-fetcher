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

#[cfg(test)]
mod tests {
    use crate::{parse, run_python, RevRegState};
    use serde_json::{Map, Number, Value};
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
