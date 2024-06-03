import { CodePromise, Abi, ContractPromise } from '@polkadot/api-contract';
import { ApiPromise, WsProvider, Keyring} from '@polkadot/api';
import { BN, BN_ONE, BN_ZERO } from "@polkadot/util";


import { json } from "./abi.js"
 
  const wsProvider = new WsProvider('ws://127.0.0.1:9944');
  const api = await ApiPromise.create({ provider: wsProvider });
  // console.log('compiler:', json)
  
  // convert json into usable contract ABI 
  let contractAbi = new Abi(json, api?.registry?.getChainProperties());

  // instantiating wasm blob on the blockchain
  const code = new CodePromise(api, json, json.source.wasm);

  const storageDepositLimit = null;
  
  const value = '0'
  
  const keyring = new Keyring({ type: "sr25519" });
  
  const userKeyring = keyring.addFromUri('//Alice');

  let constructorIndex = 0;

  // const dryRunParams = [
  //  keyring.publicKey,
  //  api.registry.createType('Balance', BN_ZERO) ,
  //  api.registry.createType('WeightV2', {
  //   proofSize: PROOFSIZE,
  //   refTime: REF_TIME
  // }),
  // null,
  // api.registry.createType('Raw', json.source.wasm),
  // contractAbi?.constructors[constructorIndex]?.toU8a([userKeyring.publicKey]),
  // ''
  // ];

  const gasLimit = api.registry.createType("WeightV2", {
    refTime: new BN("10000000000"),
    proofSize: new BN("10000000000"),
  });
 
  const tx = code.tx['new']({ value:0, gasLimit:gasLimit , storageDepositLimit }, userKeyring.publicKey);

  const unsub = await tx.signAndSend(userKeyring, {signer: userKeyring}, ({contract, status, events}) => {
    console.log('status', status.toHuman())
    if(contract) {
      const addr = events.filter(e => e.event.method == 'Instantiated')[0].event.data.toHuman().contract;
      console.log('Contract address: ', addr)
      
      unsub()
    }
  })
