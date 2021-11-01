import {getMerkleRoots}  from "./helpers/merkle_tree_utils.js";
import {
    transferCW20Tokens,
    deployContract,
    executeContract,
    instantiateContract,
    queryContract,
    recover,
  } from "./helpers/helpers.js";
import { bombay_testnet } from "./configs.js";
import { LCDClient } from "@terra-money/terra.js"
import { join } from "path"



const ARTIFACTS_PATH = "../artifacts"

const ASTRO_TOKEN_ADDRESS = "terra146hem5nuxgfg87xqhqlkqnk2fjrhekxdkxdjzh";
const GENERATOR_CONTRACT_ADDRESS = "terra146hem5nuxgfg87xqhqlkqnk2fjrhekxdkxdjzh";

const FROM_TIMESTAMP = parseInt((Date.now()/1000).toFixed(0))

const AIRDROP_INCENTIVES = 1000000
const LOCKDROP_INCENTIVES = "1000000"



async function main() {

  let terra = new LCDClient({ URL: 'https://bombay-lcd.terra.dev', chainID: 'bombay-12'})
  let wallet = recover(terra, process.env.TEST_MAIN!)

  console.log(`Wallet address from seed: ${wallet.key.accAddress}`)

/*************************************** DEPLOYMENT :: AIRDROP CONTRACT  *****************************************/

  // MERKLE ROOTS :: TERRA USERS
  let merkle_roots = await getMerkleRoots();

   // AIRDROP :: INIT MSG
  bombay_testnet.airdrop_InitMsg.config.owner = wallet.key.accAddress;
  bombay_testnet.airdrop_InitMsg.config.astro_token_address = ASTRO_TOKEN_ADDRESS;
  bombay_testnet.airdrop_InitMsg.config.merkle_roots = merkle_roots;
  bombay_testnet.airdrop_InitMsg.config.from_timestamp = FROM_TIMESTAMP;
  bombay_testnet.airdrop_InitMsg.config.to_timestamp = FROM_TIMESTAMP + 86400*90;
  bombay_testnet.airdrop_InitMsg.config.total_airdrop_size = AIRDROP_INCENTIVES;
  console.log(bombay_testnet.airdrop_InitMsg.config)

  const airdrop_contract_address = await deployContract(terra, wallet, join(ARTIFACTS_PATH, 'astro_airdrop.wasm'),  bombay_testnet.airdrop_InitMsg.config)
  console.log('AIRDROP CONTRACT ADDRESS : ' + airdrop_contract_address )

  // TRANSFER ASTRO TOKENS TO THE AIRDROP CONTRACT
//   let mars_rewards = 50000000000;
//   await transferCW20Tokens(terra, wallet, ASTRO_TOKEN_ADDRESS, airdrop_contract_address, mars_rewards);
//   console.log( (mars_rewards/(10**6)).toString() +  ' MARS TRANSFERRED TO THE AIRDROP CONTRACT :: ' + airdrop_contract_address )


/*************************************** DEPLOYMENT :: LOCKDROP CONTRACT  *****************************************/

   // LOCKDROP :: INIT MSG
   bombay_testnet.lockdrop_InitMsg.config.owner = wallet.key.accAddress;
   bombay_testnet.lockdrop_InitMsg.config.init_timestamp = FROM_TIMESTAMP;
   bombay_testnet.lockdrop_InitMsg.config.deposit_window = 86400;
   bombay_testnet.lockdrop_InitMsg.config.withdrawal_window = 86400;
 
   
   const lockdrop_contract_address = await deployContract(terra, wallet, join(ARTIFACTS_PATH, 'astroport_lockdrop.wasm'),  bombay_testnet.lockdrop_InitMsg.config)
   console.log('LOCKDROP CONTRACT ADDRESS : ' + lockdrop_contract_address )
 


/*************************************** DEPLOYMENT :: AUCTION CONTRACT  *****************************************/

   // AUCTION :: INIT MSG
   bombay_testnet.auction_InitMsg.config.owner = wallet.key.accAddress;
   bombay_testnet.auction_InitMsg.config.astro_token_address = ASTRO_TOKEN_ADDRESS;
   bombay_testnet.auction_InitMsg.config.airdrop_contract_address = airdrop_contract_address;
   bombay_testnet.auction_InitMsg.config.lockdrop_contract_address = lockdrop_contract_address;
   bombay_testnet.auction_InitMsg.config.generator_contract = GENERATOR_CONTRACT_ADDRESS;
   bombay_testnet.auction_InitMsg.config.astro_vesting_duration = 86400;
   bombay_testnet.auction_InitMsg.config.lp_tokens_vesting_duration = 86400;
   bombay_testnet.auction_InitMsg.config.init_timestamp = FROM_TIMESTAMP;
   bombay_testnet.auction_InitMsg.config.deposit_window = 86400;
   bombay_testnet.auction_InitMsg.config.withdrawal_window = 86400;
 

   const auction_contract_address = await deployContract(terra, wallet, join(ARTIFACTS_PATH, 'astro_auction.wasm'),  bombay_testnet.lockdrop_InitMsg.config)
   console.log('AUCTION CONTRACT ADDRESS : ' + auction_contract_address )
 


/*************************************** UPDATE :: LOCKDROP CONTRACT  *****************************************/

   // LOCKDROP :: UPDATE MSG
   bombay_testnet.lockdropUpdateMsg.config.auction_contract_address = auction_contract_address;
   bombay_testnet.lockdropUpdateMsg.config.generator_address = GENERATOR_CONTRACT_ADDRESS;
   bombay_testnet.lockdropUpdateMsg.config.lockdrop_incentives = LOCKDROP_INCENTIVES;
 
   await executeContract(terra, wallet, lockdrop_contract_address,  { "update_config" : { "new_config": bombay_testnet.lockdropUpdateMsg.config}} )
   console.log('LOCKDROP CONFIG UPDATED')
}


main().catch(console.log)

