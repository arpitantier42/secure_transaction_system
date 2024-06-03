import { ApiPromise, WsProvider, Keyring } from '@polkadot/api';
import { CodePromise, ContractPromise } from '@polkadot/api-contract';
import { log } from 'console';
import fs from 'fs';

async function main() {
  // Connect to the local Substrate node
//   const wsProvider = new WsProvider('wss://wss.gaming.5ire.network');   
  const wsProvider = new WsProvider('ws://127.0.0.1:9944');

  const api = await ApiPromise.create({ provider: wsProvider });

  // Create a keyring instance and add Alice
  const keyring = new Keyring({ type: 'sr25519' });
  //   const alice = keyring.addFromUri('0xce93026a23211d7dc2ff024ad739cfb4d528be5784631e72464fd5f37f38242c');
  // const alice = keyring.addFromUri('0xc453dc30e094aa6be8dce239af9d8cb1cfb04f2772f1c22fa292e6c0cff17e04');
  const alice = keyring.addFromUri('//Alice');

    // Read the WASM and ABI files
  const wasm = fs.readFileSync('/data/secure_transaction_system/secure_transaction_system/target/ink/secure_payment_system.wasm');
  //   console.log(wasm,'wasm');
  const abi = JSON.parse(fs.readFileSync('/data/secure_transaction_system/secure_transaction_system/target/ink/secure_payment_system.json'));

  // Upload the contract code
  const code = new CodePromise(api, abi, wasm);

  // Define deployment options
  const gasLimit = 200000000;
  const storageDepositLimit = null;
  const value = 0;

  console.log('alice:', alice.publicKey)

  const uploadTx = await code.tx.new({
    storageDepositLimit,
    gasLimit    
  },alice.publicKey);

  // Replace 42 with your constructor argument(s)
  console.log("Uploading the contract...");

  //-----------------------

  const uploadResult = await uploadTx.signAndSend(alice, ({ status, events }) => {
    if (status.isInBlock || status.isFinalized) {
      console.log('Upload status:', status.type);

      events.forEach(({ event }) => {

        if (api.events.contracts.Instantiated.is(event)) {
          console.log(`Contract instantiated with address: ${event.data[1].toString()}`);
        }
        if(event.method.indexOf('ExtrinsicFailed') > -1) {
          console.log('event', event.data.toHuman())
        }

      });
    }
  });

}

main().catch(console.error);


