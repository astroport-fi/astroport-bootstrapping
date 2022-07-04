import "dotenv/config";
import {
  LegacyAminoMultisigPublicKey,
  MsgExecuteContract,
  SimplePublicKey,
} from "@terra-money/terra.js";
import {
  deployContract,
  executeContract,
  newClient,
  executeContractJsonForMultiSig,
  readArtifact,
  writeArtifact,
  Client,
} from "./helpers/helpers.js";
import { pisco_testnet, mainnet, Config } from "./deploy_configs.js";
import { join } from "path";
import { writeFileSync } from "fs";

const LOCKDROP_INCENTIVES = 75_000_000_000000; // 7.5 Million = 7.5%
const AIRDROP_INCENTIVES = 25_000_000_000000;  // 2.5 Million = 2.5%
const AUCTION_INCENTIVES = 10_000_000_000000;  // 1.0 Million = 1%

// LOCKDROP INCENTIVES
const LUNA_UST_ASTRO_INCENTIVES = 21_750_000_000000;
const LUNA_BLUNA_ASTRO_INCENTIVES = 17_250_000_000000;
const ANC_UST_ASTRO_INCENTIVES = 14_250_000_000000;
const MIR_UST_ASTRO_INCENTIVES = 6_750_000_000000;
const ORION_UST_ASTRO_INCENTIVES = 1_500_000_000000;
const STT_UST_ASTRO_INCENTIVES = 3_750_000_000000;
const VKR_UST_ASTRO_INCENTIVES = 2_250_000_000000;
const MINE_UST_ASTRO_INCENTIVES = 3_000_000_000000;
const PSI_UST_ASTRO_INCENTIVES = 2_250_000_000000;
const APOLLO_UST_ASTRO_INCENTIVES = 2_250_000_000000;

const ARTIFACTS_PATH = "../artifacts";

async function main() {
  let CONFIGURATION: Config = pisco_testnet;

  // terra, wallet
  const { terra, wallet } = newClient();
  console.log(
    `chainID: ${terra.config.chainID} wallet: ${wallet.key.accAddress}`
  );

  // network : stores contract addresses
  let network = readArtifact(terra.config.chainID);
  console.log("network:", network);

  if (terra.config.chainID != "pisco-1") {
    console.log("Network is not testnet. Wrong script... terminating ... ");
    return;
  }

  // ASTRO token addresss should be set
  if (!network.astro_token_address) {
    console.log(
      `Please deploy the CW20-base ASTRO token, and then set this address in the deploy config before running this script...`
    );
    return;
  }

  // DEPOLYMENT CONFIGURATION FOR BOMBAY-12
  const LOCKDROP_INIT_TIMESTAMP =
    parseInt((Date.now() / 1000).toFixed(0)) + 180;
  const LOCKDROP_DEPOSIT_WINDOW = 3600 * 1; // 3600 * 9;
  const LOCKDROP_WITHDRAWAL_WINDOW = 3600 * 1; // * 3;

  const AUCTION_DEPOSIT_WINDOW = 3600 * 1;
  const AUCTION_WITHDRAWAL_WINDOW = 3600 * 1;

  // LOCKDROP :: CONFIG
  CONFIGURATION.lockdrop_InitMsg.config.init_timestamp =
    LOCKDROP_INIT_TIMESTAMP;
  CONFIGURATION.lockdrop_InitMsg.config.deposit_window =
    LOCKDROP_DEPOSIT_WINDOW;
  CONFIGURATION.lockdrop_InitMsg.config.withdrawal_window =
    LOCKDROP_WITHDRAWAL_WINDOW;
  // AIRDROP :: CONFIG
  CONFIGURATION.airdrop_InitMsg.config.from_timestamp =
    LOCKDROP_INIT_TIMESTAMP +
    LOCKDROP_DEPOSIT_WINDOW +
    LOCKDROP_WITHDRAWAL_WINDOW;
  CONFIGURATION.airdrop_InitMsg.config.to_timestamp =
    LOCKDROP_INIT_TIMESTAMP +
    LOCKDROP_DEPOSIT_WINDOW +
    LOCKDROP_WITHDRAWAL_WINDOW +
    86400 * 90;
  // AUCTION :: CONFIG
  CONFIGURATION.auction_InitMsg.config.init_timestamp =
    LOCKDROP_INIT_TIMESTAMP +
    LOCKDROP_DEPOSIT_WINDOW +
    LOCKDROP_WITHDRAWAL_WINDOW;
  CONFIGURATION.auction_InitMsg.config.deposit_window = AUCTION_DEPOSIT_WINDOW;
  CONFIGURATION.auction_InitMsg.config.withdrawal_window =
    AUCTION_WITHDRAWAL_WINDOW;
  CONFIGURATION.auction_InitMsg.config.lp_tokens_vesting_duration = 86400;

  /*************************************** DEPLOYMENT :: LOCKDROP CONTRACT  *****************************************/

  if (!network.lockdrop_address) {
    console.log(`${terra.config.chainID} :: Deploying Lockdrop Contract`);
    CONFIGURATION.lockdrop_InitMsg.config.owner = wallet.key.accAddress;
    console.log(CONFIGURATION.lockdrop_InitMsg);
    network.lockdrop_address = await deployContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "astroport_lockdrop.wasm"),
      CONFIGURATION.lockdrop_InitMsg.config,
      CONFIGURATION.memos.lockdrop
    );
    writeArtifact(network, terra.config.chainID);
    console.log(
      `${terra.config.chainID} :: Lockdrop Contract Address : ${network.lockdrop_address} \n`
    );
  }

  /*************************************** DEPLOYMENT :: AIRDROP CONTRACT  *****************************************/

  if (!network.airdrop_address) {
    console.log(`${terra.config.chainID} :: Deploying Airdrop Contract`);
    // Set configuration
    CONFIGURATION.airdrop_InitMsg.config.owner = wallet.key.accAddress;
    CONFIGURATION.airdrop_InitMsg.config.merkle_roots = [];
    CONFIGURATION.airdrop_InitMsg.config.astro_token_address =
      network.astro_token_address;
    // deploy airdrop contract
    console.log(CONFIGURATION.airdrop_InitMsg);
    network.airdrop_address = await deployContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "astroport_airdrop.wasm"),
      CONFIGURATION.airdrop_InitMsg.config,
      CONFIGURATION.memos.airdrop
    );
    console.log(
      `${terra.config.chainID} :: Airdrop Contract Address : ${network.airdrop_address} \n`
    );
    writeArtifact(network, terra.config.chainID);
  }

  /*************************************** DEPLOYMENT :: AUCTION CONTRACT  *****************************************/

  if (!network.auction_address) {
    console.log(`${terra.config.chainID} :: Deploying Auction Contract`);
    // Set configuration
    CONFIGURATION.auction_InitMsg.config.owner = wallet.key.accAddress;
    CONFIGURATION.auction_InitMsg.config.astro_token_address =
      network.astro_token_address;
    CONFIGURATION.auction_InitMsg.config.airdrop_contract_address =
      network.airdrop_address;
    CONFIGURATION.auction_InitMsg.config.lockdrop_contract_address =
      network.lockdrop_address;
    // deploy auction contract
    console.log(CONFIGURATION.auction_InitMsg);
    network.auction_address = await deployContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "astroport_auction.wasm"),
      CONFIGURATION.auction_InitMsg.config,
      CONFIGURATION.memos.auction
    );
    console.log(
      `${terra.config.chainID} :: Auction Contract Address : ${network.auction_address} \n`
    );
    writeArtifact(network, terra.config.chainID);
  }

  //  UpdateConfig :: SET ASTRO token and Auction Contract in Lockdrop
  if (!network.lockdrop_astro_token_set && !network.auction_set_in_lockdrop) {
    console.log(
      `${terra.config.chainID} :: Setting ASTRO token for Lockdrop...`
    );
    let tx = await executeContract(
      terra,
      wallet,
      network.lockdrop_address,
      {
        update_config: {
          new_config: {
            owner: undefined,
            astro_token_address: network.astro_token_address,
            auction_contract_address: network.auction_address,
            generator_address: undefined,
          },
        },
      },
      [],
      CONFIGURATION.memos.lockdrop_set_astro
    );
    console.log(
      `Lockdrop :: ASTRO token & Auction contract set successfully: ${tx.txhash}\n`
    );
    network.lockdrop_astro_token_set = true;
    network.auction_set_in_lockdrop = true;
    writeArtifact(network, terra.config.chainID);
  }

  // UpdateConfig :: Set Auction address in airdrop
  if (!network.auction_set_in_airdrop) {
    // update Config Tx
    let out = await executeContract(
      terra,
      wallet,
      network.airdrop_address,
      {
        update_config: {
          owner: undefined,
          auction_contract_address: network.auction_address,
          merkle_roots: undefined,
          from_timestamp: undefined,
          to_timestamp: undefined,
        },
      },
      [],
      " ASTRO Airdrop : Set Auction address "
    );
    console.log(
      `${terra.config.chainID} :: Setting auction contract address in ASTRO Airdrop contract,  ${out.txhash}`
    );
    network.auction_set_in_airdrop = true;
    writeArtifact(network, terra.config.chainID);
  }

  // // ASTRO::Send::Lockdrop::IncreaseAstroIncentives:: Transfer ASTRO to Lockdrop and set total incentives
  // if (!network.lockdrop_astro_token_transferred) {
  //   let transfer_msg = {
  //     send: {
  //       contract: network.lockdrop_address,
  //       amount: String(LOCKDROP_INCENTIVES),
  //       msg: Buffer.from(
  //         JSON.stringify({ increase_astro_incentives: {} })
  //       ).toString("base64"),
  //     },
  //   };
  //   let increase_astro_incentives = await executeContract(
  //     terra,
  //     wallet,
  //     network.astro_token_address,
  //     transfer_msg,
  //     [],
  //     "Transfer ASTRO to Lockdrop for Incentives"
  //   );
  //   console.log(
  //     `${terra.config.chainID} :: Transferring ASTRO token and setting incentives in Lockdrop... ${increase_astro_incentives.txhash}`
  //   );
  //   network.lockdrop_astro_token_transferred = true;
  //   writeArtifact(network, terra.config.chainID);
  // }
  //
  // // ASTRO::Send::Airdrop::IncreaseAstroIncentives:: Transfer ASTRO to Airdrop
  // if (!network.airdrop_astro_token_transferred) {
  //   // Transfer ASTRO Tx
  //   let tx = await executeContract(
  //     terra,
  //     wallet,
  //     network.astro_token_address,
  //     {
  //       send: {
  //         contract: network.airdrop_address,
  //         amount: String(AIRDROP_INCENTIVES),
  //         msg: Buffer.from(
  //           JSON.stringify({ increase_astro_incentives: {} })
  //         ).toString("base64"),
  //       },
  //     },
  //     [],
  //     " Airdrop : Transferring ASTRO "
  //   );
  //   console.log(
  //     `${terra.config.chainID} :: Transferring ASTRO token and setting tokens in Airdrop... ${tx.txhash}`
  //   );
  //   network.airdrop_astro_token_transferred = true;
  //   writeArtifact(network, terra.config.chainID);
  // }
  //
  // // Set Auction incentives
  // if (!network.auction_astro_token_transferred) {
  //   // Transfer ASTRO Tx
  //   let msg = {
  //     send: {
  //       contract: network.auction_address,
  //       amount: String(AUCTION_INCENTIVES),
  //       msg: Buffer.from(
  //         JSON.stringify({ increase_astro_incentives: {} })
  //       ).toString("base64"),
  //     },
  //   };
  //   let out = await executeContract(
  //     terra,
  //     wallet,
  //     network.astro_token_address,
  //     msg,
  //     [],
  //     " Transferring ASTRO token to Auction for auction participation incentives"
  //   );
  //   console.log(
  //     `${terra.config.chainID} :: Transferring ASTRO token and setting incentives in Auction... ${out.txhash}`
  //   );
  //   network.auction_astro_token_transferred = true;
  //   writeArtifact(network, terra.config.chainID);
  // }
  //
  // // Lockdrop -::- Initialize LUNA-UST Pool
  // if (!network.luna_ust_lockdrop_pool_initialized) {
  //   let luna_ust_init_msg = {
  //     initialize_pool: {
  //       terraswap_lp_token: network.luna_ust_terraswap_lp_token_address,
  //       incentives_share: LUNA_UST_ASTRO_INCENTIVES,
  //     },
  //   };
  //   console.log(
  //     `${terra.config.chainID} :: Initializing LUNA-UST LP Token Pool in Lockdrop...`
  //   );
  //   let luna_ust_pool_init = await executeContract(
  //     terra,
  //     wallet,
  //     network.lockdrop_address,
  //     luna_ust_init_msg,
  //     [],
  //     "Lockdrop -::- Initialize LUNA-UST Pool"
  //   );
  //   console.log(luna_ust_pool_init.txhash);
  //   console.log(
  //     `Lockdrop :: Luna-ust Pool successfully initialized with Lockdrop \n`
  //   );
  //   network.luna_ust_lockdrop_pool_initialized = true;
  //   writeArtifact(network, terra.config.chainID);
  // }
  //
  // // Initialize LUNA-BLUNA Pool in Lockdrop
  // if (!network.bluna_luna_lockdrop_pool_initialized) {
  //   let bluna_luna_init_msg = {
  //     initialize_pool: {
  //       terraswap_lp_token: network.bluna_luna_terraswap_lp_token_address,
  //       incentives_share: LUNA_BLUNA_ASTRO_INCENTIVES,
  //     },
  //   };
  //
  //   console.log(
  //     `${terra.config.chainID} :: Lockdrop -::- Initialize LUNA-BLUNA LP Pool...`
  //   );
  //   let bluna_luna_pool_init = await executeContract(
  //     terra,
  //     wallet,
  //     network.lockdrop_address,
  //     bluna_luna_init_msg,
  //     [],
  //     "Lockdrop -::- Initialize LUNA-BLUNA LP Pool"
  //   );
  //   console.log(bluna_luna_pool_init.txhash);
  //   console.log(
  //     `Lockdrop :: LUNA-BLUNA Pool successfully initialized with Lockdrop \n`
  //   );
  //   network.bluna_luna_lockdrop_pool_initialized = true;
  //   writeArtifact(network, terra.config.chainID);
  // }
  //
  // // Initialize ANC-UST Pool in Lockdrop
  // if (!network.anc_ust_lockdrop_pool_initialized) {
  //   let anc_ust_init_msg = {
  //     initialize_pool: {
  //       terraswap_lp_token: network.anc_ust_terraswap_lp_token_address,
  //       incentives_share: ANC_UST_ASTRO_INCENTIVES,
  //     },
  //   };
  //   console.log(
  //     `${terra.config.chainID} :: Lockdrop -::- Initialize ANC-UST LP Pool...`
  //   );
  //   let anc_ust_pool_init = await executeContract(
  //     terra,
  //     wallet,
  //     network.lockdrop_address,
  //     anc_ust_init_msg,
  //     [],
  //     "Lockdrop -::- Initialize ANC-UST LP Pool"
  //   );
  //   console.log(anc_ust_pool_init.txhash);
  //   console.log(
  //     `Lockdrop :: ANC-UST Pool successfully initialized with Lockdrop \n`
  //   );
  //   network.anc_ust_lockdrop_pool_initialized = true;
  //   writeArtifact(network, terra.config.chainID);
  // }
  //
  // // Initialize MIR-UST Pool in Lockdrop
  // // Initialize MIR-UST Pool in Lockdrop
  // if (!network.mir_ust_lockdrop_pool_initialized) {
  //   let mir_ust_init_msg = {
  //     initialize_pool: {
  //       terraswap_lp_token: network.mir_ust_terraswap_lp_token_address,
  //       incentives_share: MIR_UST_ASTRO_INCENTIVES,
  //     },
  //   };
  //   console.log(
  //     `${terra.config.chainID} :: Lockdrop -::- Initialize MIR-UST LP Pool...`
  //   );
  //   let mir_ust_pool_init = await executeContract(
  //     terra,
  //     wallet,
  //     network.lockdrop_address,
  //     mir_ust_init_msg,
  //     [],
  //     "Lockdrop -::- Initialize MIR-UST LP Pool"
  //   );
  //   console.log(mir_ust_pool_init.txhash);
  //   console.log(
  //     `Lockdrop :: MIR-UST Pool successfully initialized with Lockdrop \n`
  //   );
  //   network.mir_ust_lockdrop_pool_initialized = true;
  //   writeArtifact(network, terra.config.chainID);
  // }
  //
  // // Initialize ORION-UST Pool in Lockdrop
  // if (!network.orion_ust_lockdrop_pool_initialized) {
  //   let orion_ust_init_msg = {
  //     initialize_pool: {
  //       terraswap_lp_token: network.orion_ust_terraswap_lp_token_address,
  //       incentives_share: ORION_UST_ASTRO_INCENTIVES,
  //     },
  //   };
  //   console.log(
  //     `${terra.config.chainID} :: Lockdrop -::- Initialize ORION-UST LP Pool...`
  //   );
  //   let orion_ust_pool_init = await executeContract(
  //     terra,
  //     wallet,
  //     network.lockdrop_address,
  //     orion_ust_init_msg,
  //     [],
  //     "Lockdrop -::- Initialize ORION-UST LP Pool"
  //   );
  //   console.log(orion_ust_pool_init.txhash);
  //   console.log(
  //     `Lockdrop :: ORION-UST Pool successfully initialized with Lockdrop \n`
  //   );
  //   network.orion_ust_lockdrop_pool_initialized = true;
  //   writeArtifact(network, terra.config.chainID);
  // }
  //
  // // Initialize STT-UST Pool in Lockdrop
  // if (!network.stt_ust_lockdrop_pool_initialized) {
  //   let stt_ust_init_msg = {
  //     initialize_pool: {
  //       terraswap_lp_token: network.stt_ust_terraswap_lp_token_address,
  //       incentives_share: STT_UST_ASTRO_INCENTIVES,
  //     },
  //   };
  //   console.log(
  //     `${terra.config.chainID} :: Lockdrop -::- Initialize STT-UST LP Pool...`
  //   );
  //   let stt_ust_pool_init = await executeContract(
  //     terra,
  //     wallet,
  //     network.lockdrop_address,
  //     stt_ust_init_msg,
  //     [],
  //     "Lockdrop -::- Initialize STT-UST LP Pool"
  //   );
  //   console.log(stt_ust_pool_init.txhash);
  //   console.log(
  //     `Lockdrop :: STT-UST Pool successfully initialized with Lockdrop \n`
  //   );
  //   network.stt_ust_lockdrop_pool_initialized = true;
  //   writeArtifact(network, terra.config.chainID);
  // }
  //
  // // Initialize VKR-UST Pool in Lockdrop
  // if (!network.vkr_ust_lockdrop_pool_initialized) {
  //   let vkr_ust_init_msg = {
  //     initialize_pool: {
  //       terraswap_lp_token: network.vkr_ust_terraswap_lp_token_address,
  //       incentives_share: VKR_UST_ASTRO_INCENTIVES,
  //     },
  //   };
  //
  //   console.log(
  //     `${terra.config.chainID} :: Lockdrop -::- Initialize VKR-UST LP Pool...`
  //   );
  //   let vkr_ust_pool_init = await executeContract(
  //     terra,
  //     wallet,
  //     network.lockdrop_address,
  //     vkr_ust_init_msg,
  //     [],
  //     "Lockdrop -::- Initialize VKR-UST LP Pool"
  //   );
  //   console.log(vkr_ust_pool_init.txhash);
  //   console.log(
  //     `Lockdrop :: VKR-UST Pool successfully initialized with Lockdrop \n`
  //   );
  //   network.vkr_ust_lockdrop_pool_initialized = true;
  //   writeArtifact(network, terra.config.chainID);
  // }
  //
  // // Initialize MINE-UST Pool in Lockdrop
  // if (!network.mine_ust_lockdrop_pool_initialized) {
  //   let mine_ust_init_msg = {
  //     initialize_pool: {
  //       terraswap_lp_token: network.mine_ust_terraswap_lp_token_address,
  //       incentives_share: MINE_UST_ASTRO_INCENTIVES,
  //     },
  //   };
  //
  //   console.log(
  //     `${terra.config.chainID} :: Lockdrop -::- Initialize MINE-UST LP Pool...`
  //   );
  //   let mine_ust_pool_init = await executeContract(
  //     terra,
  //     wallet,
  //     network.lockdrop_address,
  //     mine_ust_init_msg,
  //     [],
  //     "Lockdrop -::- Initialize MINE-UST LP Pool"
  //   );
  //   console.log(mine_ust_pool_init.txhash);
  //   console.log(
  //     `Lockdrop :: MINE-UST Pool successfully initialized with Lockdrop \n`
  //   );
  //   network.mine_ust_lockdrop_pool_initialized = true;
  //   writeArtifact(network, terra.config.chainID);
  // }
  //
  // // Initialize PSI-UST Pool in Lockdrop
  // if (!network.psi_ust_lockdrop_pool_initialized) {
  //   let psi_ust_init_msg = {
  //     initialize_pool: {
  //       terraswap_lp_token: network.psi_ust_terraswap_lp_token_address,
  //       incentives_share: PSI_UST_ASTRO_INCENTIVES,
  //     },
  //   };
  //
  //   console.log(
  //     `${terra.config.chainID} :: Lockdrop -::- Initialize PSI-UST LP Pool...`
  //   );
  //   let psi_ust_pool_init = await executeContract(
  //     terra,
  //     wallet,
  //     network.lockdrop_address,
  //     psi_ust_init_msg,
  //     [],
  //     " Lockdrop -::- Initialize PSI-UST LP Pool"
  //   );
  //   console.log(psi_ust_pool_init.txhash);
  //   console.log(
  //     `Lockdrop :: PSI-UST Pool successfully initialized with Lockdrop \n`
  //   );
  //   network.psi_ust_lockdrop_pool_initialized = true;
  //   writeArtifact(network, terra.config.chainID);
  // }
  //
  // // Initialize APOLLO-UST Pool with incentive
  // if (!network.apollo_ust_lockdrop_pool_initialized) {
  //   let apollo_ust_init_msg = {
  //     initialize_pool: {
  //       terraswap_lp_token: network.apollo_ust_terraswap_lp_token_address,
  //       incentives_share: APOLLO_UST_ASTRO_INCENTIVES,
  //     },
  //   };
  //   console.log(
  //     `${terra.config.chainID} :: Lockdrop -::- Initialize APOLLO-UST LP Pool...`
  //   );
  //   let apollo_ust_pool_init = await executeContract(
  //     terra,
  //     wallet,
  //     network.lockdrop_address,
  //     apollo_ust_init_msg,
  //     [],
  //     "Lockdrop -::- Initialize APOLLO-UST LP Pool"
  //   );
  //   console.log(apollo_ust_pool_init.txhash);
  //   console.log(
  //     `Lockdrop :: APOLLO-UST Pool successfully initialized with Lockdrop \n`
  //   );
  //   network.apollo_ust_lockdrop_pool_initialized = true;
  //   writeArtifact(network, terra.config.chainID);
  // }

  writeArtifact(network, terra.config.chainID);
  console.log("FINISH");
}

main().catch(console.log);
