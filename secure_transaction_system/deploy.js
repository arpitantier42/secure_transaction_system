import { CodePromise, Abi, ContractPromise } from '@polkadot/api-contract';
import { ApiPromise, WsProvider, Keyring} from '@polkadot/api';

// import .contract file as json string
import { json } from "./abi.js"
import { GenericAccountId } from '@polkadot/types';

try {
    // let adminAccountId = api.createType('AccountId', "0x95de7914240ae17be7d8f5de780ca68372b658278602d6d747169b7729bd9efc");

  let address; // variable for storing the address of the deployed contract 

  // API creation for connection to the chain
  const wsProvider = new WsProvider('ws://44.234.186.215:9944');
  const api = await ApiPromise.create({ provider: wsProvider,noInitWarn: true });

  // convert json into usable contract ABI 
  let contractAbi = new Abi(json, api?.registry?.getChainProperties());

  // instantiating wasm blob on the blockchain
  const code = new CodePromise(api, json, json.source.wasm);
  
  // gas limit for deployment
  const gasLimit = 100000n * 1000000n

  // endowment
  const value = 0;
  
  // adding fire account for paying the gas fee
  const PHRASE = 'negative cheap cherry uncover absurd angle swarm armor tuna lounge hurdle lawsuit';
  const keyring = new Keyring({ type: "ed25519" });
  const userKeyring = keyring.addFromMnemonic(PHRASE);
  // parameters for constructor function inside the contract

  // Constructor New
  let constructorIndex = 0;

  try {
    console.log("dfsf");
    
    // upload wasm blob
    let newMethod = code && contractAbi?.constructors[constructorIndex]?.method
      ? code.tx[contractAbi.constructors[constructorIndex].method](

        {
        gasLimit: gasLimit,
        storageDepositLimit: null,
        value: value
      },)

    : null;

    // code deploy
    const unsub = await newMethod.signAndSend(userKeyring, async (response) => {
      if (response.status.isInBlock || response.status.isFinalized) {
        address = response.contract.address.toString();
        console.log("address ====== ", address);
        unsub();
      }
    });

} catch (e) {
    console.log("error catch", e);
}
}
catch(err){
  console.log("error",err.toString())
}


