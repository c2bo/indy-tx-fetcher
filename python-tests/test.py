#!/usr/bin/python3.5
import indy_vdr
import requests
import asyncio

class Tester:
    pool: indy_vdr.Pool
    genesis: str

    def __init__(self, genesis_url: str):
        try: 
            self.genesis = requests.get(genesis_url, allow_redirects=True).text
            asyncio.get_event_loop().run_until_complete(self.open())
        except Exception as err: 
            print(f"Could not resolve genesis file: {err=}")
            return None
        
    async def open(self):
        try: 
            pool = await indy_vdr.pool.open_pool(transactions=self.genesis)
            self.pool = pool
        except Exception as err:
            print(f"Could not initialize pool: {err=}")
    

    async def get_delta(self, id: str, ts: int) -> dict:
        request = indy_vdr.ledger.build_get_revoc_reg_delta_request(submitter_did=None, revoc_reg_id=id, from_ts=None, to_ts=ts)
        result = await self.pool.submit_request(request)
        return result

    async def get_from_tx(self, tx_id: int) -> dict:
        tx_req = indy_vdr.ledger.build_get_txn_request(None, 1, tx_id)
        tx_raw = await self.pool.submit_request(tx_req)
        rev_reg_id = tx_raw["data"]["txn"]["data"]["revocRegDefId"]
        timestamp = tx_raw["data"]["txnMetadata"]["txnTime"]
        res = await self.get_delta(id=rev_reg_id, ts=timestamp)
        return res
        

def main():
    tester = Tester(genesis_url="https://raw.githubusercontent.com/idunion/idunion_testnet_genesis/master/pool_transactions_genesis")
    
    print(asyncio.get_event_loop().run_until_complete(tester.get_from_tx(843)))

if __name__ == "__main__":
    main()
