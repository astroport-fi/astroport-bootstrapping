import {getMerkleRootsForTerraUsers, getMerkleRootsForEVMUsers, get_Terra_MerkleProof, get_EVM_MerkleProof, get_EVM_Signature}  from "./helpers/merkle_tree_utils.js";
import {
    transferCW20Tokens,
    deployContract,
    recover,
  } from "./helpers/helpers.js";
  import { bombay_testnet } from "./configs.js";
  import {updateAirdropConfig, claimAirdropForTerraUser, claimAirdropForEVMUser, transferAstroByAdminFromAirdropContract
,getAirdropConfig, isAirdropClaimed, verify_EVM_SignatureForAirdrop }  from "./helpers/airdrop_helpers.js";
import { LCDClient } from "@terra-money/terra.js"
import { join } from "path"


/*************************************** DEPLOYMENT :: AIRDROP CONTRACT  *****************************************/

const ASTRO_ARTIFACTS_PATH = "../artifacts"
const FROM_TIMESTAMP = parseInt((Date.now()/1000).toFixed(0))
const TILL_TIMESTAMP = FROM_TIMESTAMP + (86400 * 30)

const ASTRO_TOKEN_ADDRESS = "terra1rfuctcuyyxqz468wha5m805vt43g83tep4rm5x";

async function main() {

  let terra = new LCDClient({ URL: 'https://bombay-lcd.terra.dev', chainID: 'bombay-11'})
  let wallet = recover(terra, process.env.TEST_MAIN!)
  console.log(`Wallet address from seed: ${wallet.key.accAddress}`)

  // MERKLE ROOTS :: TERRA USERS
  let terra_merkle_roots = await getMerkleRootsForTerraUsers();

  // MERKLE ROOTS :: EVM (BSC/ETHEREUM) USERS
  let evm_merkle_roots = await getMerkleRootsForEVMUsers();

   // AIRDROP :: INIT MSG
  bombay_testnet.airdrop_InitMsg.config.owner = wallet.key.accAddress;
  bombay_testnet.airdrop_InitMsg.config.astro_token_address = ASTRO_TOKEN_ADDRESS;
  bombay_testnet.airdrop_InitMsg.config.terra_merkle_roots = terra_merkle_roots;
  bombay_testnet.airdrop_InitMsg.config.evm_merkle_roots = evm_merkle_roots;
  bombay_testnet.airdrop_InitMsg.config.from_timestamp = FROM_TIMESTAMP;
  bombay_testnet.airdrop_InitMsg.config.till_timestamp = TILL_TIMESTAMP;

  const airdrop_contract_address = await deployContract(terra, wallet, join(ASTRO_ARTIFACTS_PATH, 'astro_airdrop.wasm'),  bombay_testnet.airdrop_InitMsg.config)
  console.log('AIRDROP CONTRACT ADDRESS : ' + airdrop_contract_address )

  // TRANSFER ASTRO TOKENS TO THE AIRDROP CONTRACT
  let astro_rewards = 50000000000;
  await transferCW20Tokens(terra, wallet, ASTRO_TOKEN_ADDRESS, airdrop_contract_address, astro_rewards);
  console.log( (astro_rewards/(10**6)).toString() +  ' ASTRO TRANSFERRED TO THE AIRDROP CONTRACT :: ' + airdrop_contract_address )
}




main().catch(console.log)