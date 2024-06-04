import { CodePromise, Abi, ContractPromise } from '@polkadot/api-contract';
import { ApiPromise, WsProvider, Keyring} from '@polkadot/api';
import { BN, BN_ONE, BN_ZERO } from "@polkadot/util";
import { json } from "./abi.js"
 
  const wsProvider = new WsProvider('ws://127.0.0.1:9944');
  // const wsProvider = new WsProvider('wss://wss.gaming.5ire.network');
  const api = await ApiPromise.create({ provider: wsProvider });  

  // instantiating wasm blob on the blockchain
  const code = new CodePromise(api, json, json.source.wasm);
  const gasLimit = api.registry.createType("WeightV2", {
    refTime: new BN("10000000000"),
    proofSize: new BN("10000000000"),
  });

  const storageDepositLimit = null;
  
  const keyring = new Keyring({ type: "sr25519" });
  const userKeyring = keyring.addFromUri('//Alice');
  // const userKeyring = keyring.addFromUri('0x330a99a914a315909e44b3a5da723798c2010f3720608db7c18b6f32ac86f858');

  const tx = code.tx['new']({ value:0, gasLimit:gasLimit , storageDepositLimit }, userKeyring.publicKey);

  const unsub = await tx.signAndSend(userKeyring, {signer: userKeyring}, ({contract, status, events}) => {
    console.log('status', status.toHuman())
    if(contract) {
      const addr = events.filter(e => e.event.method == 'Instantiated')[0].event.data.toHuman().contract;
      console.log('Contract address: ', addr)
      unsub()
    }
  })

  