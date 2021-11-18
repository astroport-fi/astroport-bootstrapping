import 'dotenv/config'
import { getMerkleRoots } from "./helpers/merkle_tree_utils.js";
import {
  deployContract,
  executeContract,
  newClient,
  readArtifact,
  writeArtifact,
  Client
} from "./helpers/helpers.js";
import { bombay_testnet } from "./deploy_configs.js";
import { LCDClient } from "@terra-money/terra.js"
import { join } from "path"

const ARTIFACTS_PATH = "../artifacts"


const FROM_TIMESTAMP = parseInt((Date.now() / 1000).toFixed(0)) + 150

const AIRDROP_INCENTIVES = 25_000_000_000000
const LOCKDROP_INCENTIVES = 75_000_000_000000
const AUCTION_INCENTIVES = 10_000_000_000000

async function main() {

  const { terra, wallet } = newClient()
  console.log(`chainID: ${terra.config.chainID} wallet: ${wallet.key.accAddress}`)
  console.log(`FROM_TIMESTAMP: ${FROM_TIMESTAMP} `)

  const network = readArtifact(terra.config.chainID)
  console.log('network:', network)


  if (!network.astrotokenAddress) {
    console.log(`Please deploy the CW20-base ASTRO token, and then set this address in the deploy config before running this script...`)
    return
  }

  /*************************************** DEPLOYMENT :: AIRDROP CONTRACT  *****************************************/
  if (!network.airdropAddress) {
    console.log('Deploy Airdrop...')

    bombay_testnet.airdrop_InitMsg.config.owner = wallet.key.accAddress;
    bombay_testnet.airdrop_InitMsg.config.astro_token_address = network.astrotokenAddress;
    bombay_testnet.airdrop_InitMsg.config.merkle_roots = await getMerkleRoots();
    bombay_testnet.airdrop_InitMsg.config.from_timestamp = FROM_TIMESTAMP;
    bombay_testnet.airdrop_InitMsg.config.to_timestamp = FROM_TIMESTAMP + 86400 * 90;
    bombay_testnet.airdrop_InitMsg.config.total_airdrop_size = String(AIRDROP_INCENTIVES);

    network.airdropAddress = await deployContract(terra, wallet, join(ARTIFACTS_PATH, 'astroport_airdrop.wasm'), bombay_testnet.airdrop_InitMsg.config)
    console.log(`Airdrop Contract Address : ${network.airdropAddress}`)

    //  ************* Transfer tokens to Airdrop Contract *************
    let msg = { transfer: { recipient: network.airdropAddress, amount: String(AIRDROP_INCENTIVES) } }
    console.log('execute', network.astrotokenAddress, JSON.stringify(msg))
    let out = await executeContract(terra, wallet, network.astrotokenAddress, msg)
    console.log(out.txhash)
  }

  /*************************************** DEPLOYMENT :: LOCKDROP CONTRACT  *****************************************/

  if (!network.lockdropAddress) {
    console.log('Deploy Lockdrop...')

    bombay_testnet.lockdrop_InitMsg.config.owner = wallet.key.accAddress;
    bombay_testnet.lockdrop_InitMsg.config.init_timestamp = FROM_TIMESTAMP;
    bombay_testnet.lockdrop_InitMsg.config.deposit_window = 86400;
    bombay_testnet.lockdrop_InitMsg.config.withdrawal_window = 86400;
    console.log(bombay_testnet.lockdrop_InitMsg.config)

    network.lockdropAddress = await deployContract(terra, wallet, join(ARTIFACTS_PATH, 'astroport_lockdrop.wasm'), bombay_testnet.lockdrop_InitMsg.config)
    console.log(`Lockdrop Contract Address : ${network.lockdropAddress}`)

    // set astro token
    console.log('set astro token for lockdrop...')
    await executeContract(terra, wallet, network.lockdropAddress, {
      "update_config": {
        "new_config": {
          "owner": undefined,
          "astro_token_address": network.astrotokenAddress,
          "auction_contract_address": undefined,
          "generator_address": undefined,
        }
      }
    }
    )
    console.log(`Lockdrop Contract :: astro token successfully set`)

    // increase lockdrop incentives
    let msg = { send: { contract: network.lockdropAddress, amount: String(LOCKDROP_INCENTIVES), msg: Buffer.from(JSON.stringify({ increase_astro_incentives: {} })).toString('base64') } }
    console.log('execute', network.astrotokenAddress, JSON.stringify(msg))
    let out = await executeContract(terra, wallet, network.astrotokenAddress, msg)
    console.log(out.txhash)
  }

  /******************************4********* DEPLOYMENT :: AUCTION CONTRACT  *****************************************/

  if (!network.auctionAddress) {
    console.log('Deploy Auction...')

    let auction_init_timestamp = bombay_testnet.lockdrop_InitMsg.config.init_timestamp + bombay_testnet.lockdrop_InitMsg.config.deposit_window + bombay_testnet.lockdrop_InitMsg.config.withdrawal_window;
    bombay_testnet.auction_InitMsg.config.owner = wallet.key.accAddress;
    bombay_testnet.auction_InitMsg.config.astro_token_address = network.astrotokenAddress;
    bombay_testnet.auction_InitMsg.config.airdrop_contract_address = network.airdropAddress;
    bombay_testnet.auction_InitMsg.config.lockdrop_contract_address = network.lockdropAddress;
    bombay_testnet.auction_InitMsg.config.lp_tokens_vesting_duration = 86400;
    bombay_testnet.auction_InitMsg.config.init_timestamp = auction_init_timestamp;
    bombay_testnet.auction_InitMsg.config.deposit_window = 86400;
    bombay_testnet.auction_InitMsg.config.withdrawal_window = 86400;
    // console.log(bombay_testnet.auction_InitMsg.config)

    network.auctionAddress = await deployContract(terra, wallet, join(ARTIFACTS_PATH, 'astroport_auction.wasm'), bombay_testnet.auction_InitMsg.config)
    console.log(`Auction Contract Address : ${network.auctionAddress}`)

    // increase lockdrop incentives
    let msg = { send: { contract: network.auctionAddress, amount: String(AUCTION_INCENTIVES), msg: Buffer.from(JSON.stringify({ increase_astro_incentives: {} })).toString('base64') } }
    console.log('execute', network.astrotokenAddress, JSON.stringify(msg))
    let out = await executeContract(terra, wallet, network.astrotokenAddress, msg)
    console.log(out.txhash)
  }

  /*************************************** UPDATE TRANSACTION :: AIRDROP CONTRACT  *****************************************/
  if (network.airdropAddress) {
    console.log('Set auction_address in Airdrop Contract ...')

    await executeContract(terra, wallet, network.airdropAddress, {
      "update_config": {
        "owner": undefined,
        "auction_contract_address": network.auctionAddress,
        "merkle_roots": undefined,
        "from_timestamp": undefined,
        "to_timestamp": undefined
      }
    }
    )
    console.log(`Airdrop Contract :: auction_address successfully set`)
  }

  /*************************************** UPDATE TRANSACTION :: LOCKDROP CONTRACT  *****************************************/

  if (network.lockdropAddress) {
    console.log('Set auction_address in Lockdrop Contract...')

    await executeContract(terra, wallet, network.lockdropAddress, {
      "update_config": {
        "new_config": {
          "owner": undefined,
          "astro_token_address": undefined,
          "auction_contract_address": network.auctionAddress,
          "generator_address": undefined,
        }
      }
    }
    )
    console.log(`Lockdrop Contract :: auction_address successfully set`)
  }

  // Set UST-ASTRO pool address

  if (network.ust_pair_address) {
    console.log('Set UST-ASTRO pool address in Auction Contract...')
    await executeContract(terra, wallet, network.auctionAddress, {
      "update_config": {
        "new_config": {
          "owner": undefined,
          "astro_ust_pair_address": network.ust_pair_address,
          "generator_address": undefined,
        }
      }
    }
    )
    console.log(`Auction Contract :: ust_pair_address successfully set`)

  }

  // Set generator
  if (network.generator_contract) {
    console.log('Update generator in contracts')

    await executeContract(terra, wallet, network.lockdropAddress, {
      "update_config": {
        "new_config": {
          "owner": undefined,
          "astro_token_address": undefined,
          "auction_contract_address": undefined,
          "generator_address": network.generator_contract,
        }
      }
    }
    )
    console.log(`Lockdrop Contract :: generator successfully set`)

    await executeContract(terra, wallet, network.auctionAddress, {
      "update_config": {
        "new_config": {
          "owner": undefined,
          "astro_ust_pair_address": undefined,
          "generator_address": network.generator_contract,
        }
      }
    }
    )
    console.log(`Auction Contract :: generator successfully set`)

  }

  writeArtifact(network, terra.config.chainID)
  console.log('FINISH')
}


main().catch(console.log)

