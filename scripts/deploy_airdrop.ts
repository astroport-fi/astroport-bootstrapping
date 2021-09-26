import {getMerkleRootsForTerraUsers, getMerkleRootsForEVMUsers, get_Terra_MerkleProof, get_EVM_MerkleProof, get_EVM_Signature}  from "./helpers/merkle_tree_utils.js";
import {
    transferCW20Tokens,
    deployContract,
    executeContract,
    instantiateContract,
    queryContract,
    recover,
    setTimeoutDuration,
    uploadContract,
  } from "./helpers/helpers.js";
  import { bombay_testnet } from "./configs.js";
  import {updateAirdropConfig, claimAirdropForTerraUser, claimAirdropForEVMUser, transferAstroByAdminFromAirdropContract
,getAirdropConfig, isAirdropClaimed, verify_EVM_SignatureForAirdrop }  from "./helpers/airdrop_helpers.js";
import Web3 from 'web3';
import { LCDClient } from "@terra-money/terra.js"
import { join } from "path"


/*************************************** DEPLOYMENT :: AIRDROP CONTRACT  *****************************************/

const ASTRO_ARTIFACTS_PATH = "../artifacts"
const ASTRO_TOKEN_ADDRESS = "terra1rfuctcuyyxqz468wha5m805vt43g83tep4rm5x";
const FROM_TIMESTAMP = parseInt((Date.now()/1000).toFixed(0))
const TILL_TIMESTAMP = FROM_TIMESTAMP + (86400 * 30)

async function main() {

  let terra = new LCDClient({ URL: 'https://bombay-lcd.terra.dev', chainID: 'bombay-10'})
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
  // console.log(bombay_testnet.airdrop_InitMsg.config)

  const airdrop_contract_address = await deployContract(terra, wallet, join(ASTRO_ARTIFACTS_PATH, 'astro_airdrop.wasm'),  bombay_testnet.airdrop_InitMsg.config)
  // const airdrop_contract_address = "terra1h8f84ztltpa530qdv9vc387zr48dru7gvr3703"
  console.log('AIRDROP CONTRACT ADDRESS : ' + airdrop_contract_address )

  // TRANSFER ASTRO TOKENS TO THE AIRDROP CONTRACT
  // let astro_rewards = 50000000000;
  // await transferCW20Tokens(terra, wallet, ASTRO_TOKEN_ADDRESS, airdrop_contract_address, astro_rewards);
  // console.log( (astro_rewards/(10**6)).toString() +  ' ASTRO TRANSFERRED TO THE AIRDROP CONTRACT :: ' + airdrop_contract_address )

// /*************************************** AIRDROP CONTRACT :: TESTING FUNCTION CALLS  *****************************************/
  let web3 = new Web3(Web3.givenProvider || 'ws://some.local-or-remote.node:8546');


  // GET CONFIGURATION
  // let config = await getAirdropConfig(terra, airdrop_contract_address);
  // console.log(config);

  // CHECK IF CLAIMED
  // let test_terra_address = wallet.key.accAddress
  // let is_claimed = await isAirdropClaimed(terra, airdrop_contract_address, test_terra_address );
  // console.log(is_claimed);

  // VERIFY SIGNATURE VIA CONTRACT QUERY
  let test_evm_account = web3.eth.accounts.privateKeyToAccount('89fa5355adfd0879b7dc568ac8b5d543d7609a96b0d8aa0486305403b7429c50');
  let test_msg_to_sign = "testing"
  let test_signature = get_EVM_Signature(test_evm_account, test_msg_to_sign);

  let msg_hash = test_signature["messageHash"].substr(2,66)  
  let signature_hash = test_signature["signature"].substr(2,128) 

  console.log("a/c address = " + test_evm_account.address)
  console.log("msg_hash = " + msg_hash)
  console.log("signature_hash = " + signature_hash)

  console.log(test_signature)
  let verify_response = await verify_EVM_SignatureForAirdrop(terra, airdrop_contract_address,test_evm_account.address.replace('0x', '').toLowerCase() , signature_hash, msg_hash);
  console.log(verify_response);


  // // AIRDROP CLAIM : GET MERKLE PROOF FOR TERRA USER --> CLAIM AIRDROP IF VALID PROOF
  // let airdrop_claim_amount = 474082154
  // let terra_user_merkle_proof = get_Terra_MerkleProof( { "address":wallet.key.accAddress, "amount":airdrop_claim_amount.toString() } );
  // console.log(terra_user_merkle_proof)
  // await claimAirdropForTerraUser(terra, wallet, airdrop_contract_address, airdrop_claim_amount, terra_user_merkle_proof["proof"], terra_user_merkle_proof["root_index"])

  // let is_claimed_ = await isAirdropClaimed(terra, airdrop_contract_address, wallet.key.accAddress );
  // console.log(is_claimed_);


  // // // AIRDROP CLAIM : GET MERKLE PROOF, SIGNATURE FOR EVM USER --> CLAIM AIRDROP IF VALID PROOF
  // let eth_user_ = web3.eth.accounts.privateKeyToAccount('89fa5355adfd0879b7dc568ac8b5d543d7609a96b0d8aa0486305403b7429c50');
  // let eth_user_address = eth_user_.address;
  // // const eth_user_address = ""
  // let airdrop_claim_amount_evm_user = 324473973
  // let  evm_user_merkle_proof = get_EVM_MerkleProof( { "address":eth_user_address, "amount":airdrop_claim_amount_evm_user.toString() } );
  // let msg_to_sign = "Testing" //eth_user_address.substr(2,42).toLowerCase()  + wallet.key.accAddress + airdrop_claim_amount_evm_user.toString();  
  // let signature =  get_EVM_Signature(eth_user_, msg_to_sign);

  // let msg_hash = signature["messageHash"].substr(2,66)
  // let signature_hash = signature["signature"].substr(2,128) 

  // // let is_valid_sig = await verifySignature( terra, airdrop_contract_address, eth_user_address, signature, msg_to_sign );
  // await claimAirdropForEVMUser( terra, wallet, airdrop_contract_address, eth_user_address, airdrop_claim_amount_evm_user, evm_user_merkle_proof["proof"], evm_user_merkle_proof["root_index"], signature_hash, msg_hash );
  
  // let is_claimed_evm = await isAirdropClaimed(terra, airdrop_contract_address, eth_user_address.substr(2,42).toLowerCase() );
  // console.log(is_claimed_evm);


  // // ADMIN FUNCTION : TRANSFER ASTRO FROM AIRDROP CONTRACT TO RECEPIENT
  // recepient = wallet.key.accAddress
  // await transferAstroByAdminFromAirdropContract(terra, wallet, airdrop_contract_address, recepient)


  // // ADMIN FUNCTION : UPDATE AIRDROP CONFIG
  // recepient = wallet.key.accAddress
  // await transferAstroByAdminFromAirdropContract(terra, wallet, airdrop_contract_address, recepient)

}

main().catch(console.log)