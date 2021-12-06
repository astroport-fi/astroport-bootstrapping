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
import { getMerkleRoots } from "./helpers/merkle_tree_utils.js";
import { bombay_testnet, mainnet, Config } from "./deploy_configs.js";
import { join } from "path";

let MULTI_SIG_TO_USE = "";

const LOCKDROP_INCENTIVES = 75_000_00_000000; // 7.5 Million = 7.5%
const AIRDROP_INCENTIVES = 25_000_00_000000; // 2.5 Million = 2.5%
const AUCTION_INCENTIVES = 10_000_00_000000; // 1.0 Million = 1%
// LOCKDROP INCENTIVES
const LUNA_UST_ASTRO_INCENTIVES = 21_750_000_000000;
const LUNA_BLUNA_ASTRO_INCENTIVES = 17_250_000_000000;
const ANC_UST_ASTRO_INCENTIVES = 14_250_000_000000;
const MIR_UST_ASTRO_INCENTIVES = 6_250_000_000000;
const ORION_UST_ASTRO_INCENTIVES = 1_500_000_000000;
const STT_UST_ASTRO_INCENTIVES = 3_750_000_000000;
const VKR_UST_ASTRO_INCENTIVES = 2_250_000_000000;
const MINE_UST_ASTRO_INCENTIVES = 3_000_000_000000;
const PSI_UST_ASTRO_INCENTIVES = 2_250_000_000000;
const APOLLO_UST_ASTRO_INCENTIVES = 2_250_000_000000;

const ARTIFACTS_PATH = "../artifacts";

async function main() {
  let CONFIGURATION: Config = bombay_testnet;

  // terra, wallet
  const { terra, wallet } = newClient();
  console.log(
    `chainID: ${terra.config.chainID} wallet: ${wallet.key.accAddress}`
  );

  // network : stores contract addresses
  const network = readArtifact(terra.config.chainID);
  console.log("network:", network);

  // Configuration to use based on network instance
  if (terra.config.chainID == "bombay-12") {
    MULTI_SIG_TO_USE = wallet.key.accAddress;
    CONFIGURATION = bombay_testnet;
  } else if (terra.config.chainID == "columbus-5") {
    CONFIGURATION = mainnet;
  }

  // ASTRO Token addresss should be set
  if (!network.astrotokenAddress) {
    console.log(
      `Please deploy the CW20-base ASTRO token, and then set this address in the deploy config before running this script...`
    );
    return;
  }

  // DEPOLYMENT CONFIGURATION IF CHAIN == BOMBAY-12
  if (terra.config.chainID == "bombay-12") {
    const LOCKDROP_INIT_TIMESTAMP =
      parseInt((Date.now() / 1000).toFixed(0)) + 180;
    const LOCKDROP_DEPOSIT_WINDOW = 3600 * 0.25;
    const LOCKDROP_WITHDRAWAL_WINDOW = 3600 * 0.1;

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
    CONFIGURATION.auction_InitMsg.config.deposit_window =
      LOCKDROP_DEPOSIT_WINDOW;
    CONFIGURATION.auction_InitMsg.config.withdrawal_window =
      LOCKDROP_WITHDRAWAL_WINDOW;
    CONFIGURATION.auction_InitMsg.config.lp_tokens_vesting_duration = 86400;
  }

  /*************************************** DEPLOYMENT :: LOCKDROP CONTRACT  *****************************************/
  /*************************************** DEPLOYMENT :: LOCKDROP CONTRACT  *****************************************/
  /*************************************** DEPLOYMENT :: LOCKDROP CONTRACT  *****************************************/

  if (!network.lockdropAddress) {
    console.log(`${terra.config.chainID} :: Deploying Lockdrop Contract`);
    CONFIGURATION.lockdrop_InitMsg.config.owner = MULTI_SIG_TO_USE;
    network.lockdropAddress = await deployContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "astroport_lockdrop.wasm"),
      CONFIGURATION.lockdrop_InitMsg.config,
      CONFIGURATION.memos.lockdrop
    );
    writeArtifact(network, terra.config.chainID);
    console.log(
      `${terra.config.chainID} :: Lockdrop Contract Address : ${network.lockdropAddress} \n`
    );
  }

  /*************************************** DEPLOYMENT :: AIRDROP CONTRACT  *****************************************/
  /*************************************** DEPLOYMENT :: AIRDROP CONTRACT  *****************************************/
  /*************************************** DEPLOYMENT :: AIRDROP CONTRACT  *****************************************/

  if (!network.airdrop_Address) {
    console.log(`${terra.config.chainID} :: Deploying Airdrop Contract`);
    // Set configuration
    CONFIGURATION.airdrop_InitMsg.config.owner = MULTI_SIG_TO_USE;
    CONFIGURATION.airdrop_InitMsg.config.merkle_roots = await getMerkleRoots();
    CONFIGURATION.airdrop_InitMsg.config.astro_token_address =
      network.astrotokenAddress;
    CONFIGURATION.airdrop_InitMsg.config.total_airdrop_size =
      String(AIRDROP_INCENTIVES);
    // deploy airdrop contract
    network.airdrop_Address = await deployContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "astroport_airdrop.wasm"),
      CONFIGURATION.airdrop_InitMsg.config,
      CONFIGURATION.memos.airdrop
    );
    console.log(
      `${terra.config.chainID} :: Airdrop Contract Address : ${network.airdrop_Address} \n`
    );
    writeArtifact(network, terra.config.chainID);
  }

  /*************************************** DEPLOYMENT :: AUCTION CONTRACT  *****************************************/
  /*************************************** DEPLOYMENT :: AUCTION CONTRACT  *****************************************/
  /*************************************** DEPLOYMENT :: AUCTION CONTRACT  *****************************************/

  if (!network.auction_Address) {
    console.log(`${terra.config.chainID} :: Deploying Auction Contract`);
    // Set configuration
    CONFIGURATION.auction_InitMsg.config.owner = MULTI_SIG_TO_USE;
    CONFIGURATION.auction_InitMsg.config.astro_token_address =
      network.astrotokenAddress;
    CONFIGURATION.auction_InitMsg.config.airdrop_contract_address =
      network.airdrop_Address;
    CONFIGURATION.auction_InitMsg.config.lockdrop_contract_address =
      network.lockdropAddress;
    // deploy auction contract
    network.auction_Address = await deployContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "astroport_auction.wasm"),
      CONFIGURATION.auction_InitMsg.config,
      CONFIGURATION.memos.auction
    );
    console.log(
      `${terra.config.chainID} :: Auction Contract Address : ${network.auction_Address} \n`
    );
    writeArtifact(network, terra.config.chainID);
  }

  /*************************************** LOCKDROP :: IF NETWORK IS BOMBAY-12  *****************************************/
  /*************************************** LOCKDROP :: IF NETWORK IS BOMBAY-12  *****************************************/
  /*************************************** LOCKDROP :: IF NETWORK IS BOMBAY-12  *****************************************/
  /*************************************** LOCKDROP :: IF NETWORK IS BOMBAY-12  *****************************************/

  if (terra.config.chainID == "bombay-12") {
    //  UpdateConfig :: SET ASTRO Token and Auction Contract in Lockdrop if bombay-12
    //  UpdateConfig :: SET ASTRO Token and Auction Contract in Lockdrop if bombay-12
    if (!network.lockdrop_astro_token_set && !network.auction_set_in_lockdrop) {
      console.log(
        `${terra.config.chainID} :: Setting ASTRO Token for Lockdrop...`
      );
      let tx = await executeContract(
        terra,
        wallet,
        network.lockdropAddress,
        {
          update_config: {
            new_config: {
              owner: undefined,
              astro_token_address: network.astrotokenAddress,
              auction_contract_address: network.auction_Address,
              generator_address: undefined,
            },
          },
        },
        [],
        CONFIGURATION.memos.lockdrop_set_astro
      );
      console.log(
        `Lockdrop :: ASTRO Token & Auction contract set successfully set ${tx.txhash}\n`
      );
      network.lockdrop_astro_token_set = true;
      network.auction_set_in_lockdrop = true;
      writeArtifact(network, terra.config.chainID);
    }

    // UpdateConfig :: Set Auction address in airdrop if bombay-12
    // UpdateConfig :: Set Auction address in airdrop if bombay-12
    if (!network.auction_set_in_airdrop) {
      // update Config Tx
      let out = await executeContract(
        terra,
        wallet,
        network.airdrop_Address,
        {
          update_config: {
            owner: undefined,
            auction_contract_address: network.auction_Address,
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

    // ASTRO::Send::Lockdrop::IncreaseAstroIncentives:: Transfer ASTRO to Lockdrop and set total incentives if bombay-12
    // ASTRO::Send::Lockdrop::IncreaseAstroIncentives:: Transfer ASTRO to Lockdrop and set total incentives if bombay-12
    if (!network.lockdrop_astro_token_transferred) {
      let transfer_msg = {
        send: {
          contract: network.lockdropAddress,
          amount: String(LOCKDROP_INCENTIVES),
          msg: Buffer.from(
            JSON.stringify({ increase_astro_incentives: {} })
          ).toString("base64"),
        },
      };
      let increase_astro_incentives = await executeContract(
        terra,
        wallet,
        network.astrotokenAddress,
        transfer_msg,
        [],
        "Transfer ASTRO to Lockdrop for Incentives"
      );
      console.log(
        `${terra.config.chainID} :: Transfering ASTRO Token and setting incentives in Lockdrop... ${increase_astro_incentives.txhash}`
      );
      network.lockdrop_astro_token_transferred = true;
      writeArtifact(network, terra.config.chainID);
    }

    // ASTRO::TRANSFER : Transfer ASTRO to Airdrop if bombay-12
    // ASTRO::TRANSFER : Transfer ASTRO to Airdrop if bombay-12
    if (!network.airdrop_astro_token_transferred) {
      // transfer ASTRO Tx
      let tx = await executeContract(
        terra,
        wallet,
        network.astrotokenAddress,
        {
          transfer: {
            recipient: network.airdrop_Address,
            amount: String(AIRDROP_INCENTIVES),
          },
        },
        [],
        " Transfering ASTRO Token to Airdrop"
      );
      console.log(
        `${terra.config.chainID} :: Transfering ASTRO Token and setting incentives in Airdrop... ${tx.txhash}`
      );
      network.airdrop_astro_token_transferred = true;
      writeArtifact(network, terra.config.chainID);
    }

    // Set Auction incentives if bombay-12
    // Set Auction incentives if bombay-12
    if (!network.auction_astro_token_transferred) {
      // transfer ASTRO Tx
      let msg = {
        send: {
          contract: network.auction_Address,
          amount: String(AUCTION_INCENTIVES),
          msg: Buffer.from(
            JSON.stringify({ increase_astro_incentives: {} })
          ).toString("base64"),
        },
      };
      let out = await executeContract(
        terra,
        wallet,
        network.astrotokenAddress,
        msg,
        [],
        " Transfering ASTRO Token to Auction for auction participation incentives"
      );
      console.log(
        `${terra.config.chainID} :: Transfering ASTRO Token and setting incentives in Auction... ${out.txhash}`
      );
      network.auction_astro_token_transferred = true;
      writeArtifact(network, terra.config.chainID);
    }

    // Initialize LUNA-UST Pool in Lockdrop
    // Initialize LUNA-UST Pool in Lockdrop
    if (!network.luna_ust_lockdrop_pool_initialized) {
      let luna_ust_init_msg = {
        initialize_pool: {
          terraswap_lp_token: network.luna_ust_terraswap_lp_token_address,
          incentives_share: LUNA_UST_ASTRO_INCENTIVES,
        },
      };
      console.log(
        `${terra.config.chainID} :: Initializing LUNA-UST LP Token Pool in Lockdrop...`
      );
      let luna_ust_pool_init = await executeContract(
        terra,
        wallet,
        network.lockdropAddress,
        luna_ust_init_msg,
        [],
        "Initialize LUNA-UST Pool in Lockdrop"
      );
      console.log(luna_ust_pool_init.txhash);
      console.log(
        `Lockdrop :: Luna-ust Pool successfully initialized with Lockdrop \n`
      );
      network.luna_ust_lockdrop_pool_initialized = true;
      writeArtifact(network, terra.config.chainID);
    }

    // Initialize LUNA-BLUNA Pool in Lockdrop
    // Initialize LUNA-BLUNA Pool in Lockdrop
    if (!network.bluna_luna_lockdrop_pool_initialized) {
      let bluna_luna_init_msg = {
        initialize_pool: {
          terraswap_lp_token: network.bluna_luna_terraswap_lp_token_address,
          incentives_share: LUNA_BLUNA_ASTRO_INCENTIVES,
        },
      };

      console.log(
        `${terra.config.chainID} :: Initializing LUNA-BLUNA LP Token Pool in Lockdrop...`
      );
      let bluna_luna_pool_init = await executeContract(
        terra,
        wallet,
        network.lockdropAddress,
        bluna_luna_init_msg,
        [],
        "Initializing LUNA-BLUNA LP Token Pool in Lockdrop"
      );
      console.log(bluna_luna_pool_init.txhash);
      console.log(
        `Lockdrop :: LUNA-BLUNA Pool successfully initialized with Lockdrop \n`
      );
      network.bluna_luna_lockdrop_pool_initialized = true;
      writeArtifact(network, terra.config.chainID);
    }

    // Initialize ANC-UST Pool in Lockdrop
    // Initialize ANC-UST Pool in Lockdrop
    if (!network.anc_ust_lockdrop_pool_initialized) {
      let anc_ust_init_msg = {
        initialize_pool: {
          terraswap_lp_token: network.anc_ust_terraswap_lp_token_address,
          incentives_share: ANC_UST_ASTRO_INCENTIVES,
        },
      };
      console.log(
        `${terra.config.chainID} :: Initializing ANC-UST LP Token Pool in Lockdrop...`
      );
      let anc_ust_pool_init = await executeContract(
        terra,
        wallet,
        network.lockdropAddress,
        anc_ust_init_msg,
        [],
        "Initializing ANC-UST LP Token Pool in Lockdrop"
      );
      console.log(anc_ust_pool_init.txhash);
      console.log(
        `Lockdrop :: ANC-UST Pool successfully initialized with Lockdrop \n`
      );
      network.anc_ust_lockdrop_pool_initialized = true;
      writeArtifact(network, terra.config.chainID);
    }

    // Initialize MIR-UST Pool in Lockdrop
    // Initialize MIR-UST Pool in Lockdrop
    if (!network.mir_ust_lockdrop_pool_initialized) {
      let mir_ust_init_msg = {
        initialize_pool: {
          terraswap_lp_token: network.mir_ust_terraswap_lp_token_address,
          incentives_share: MIR_UST_ASTRO_INCENTIVES,
        },
      };
      console.log(
        `${terra.config.chainID} :: Initializing MIR-UST LP Token Pool in Lockdrop...`
      );
      let mir_ust_pool_init = await executeContract(
        terra,
        wallet,
        network.lockdropAddress,
        mir_ust_init_msg,
        [],
        "Initializing MIR-UST LP Token Pool in Lockdrop"
      );
      console.log(mir_ust_pool_init.txhash);
      console.log(
        `Lockdrop :: MIR-UST Pool successfully initialized with Lockdrop \n`
      );
      network.mir_ust_lockdrop_pool_initialized = true;
      writeArtifact(network, terra.config.chainID);
    }

    // Initialize ORION-UST Pool in Lockdrop
    // Initialize ORION-UST Pool in Lockdrop
    if (!network.orion_ust_lockdrop_pool_initialized) {
      let orion_ust_init_msg = {
        initialize_pool: {
          terraswap_lp_token: network.orion_ust_terraswap_lp_token_address,
          incentives_share: ORION_UST_ASTRO_INCENTIVES,
        },
      };
      console.log(
        `${terra.config.chainID} :: Initializing ORION-UST LP Token Pool in Lockdrop...`
      );
      let orion_ust_pool_init = await executeContract(
        terra,
        wallet,
        network.lockdropAddress,
        orion_ust_init_msg,
        [],
        "Initializing ORION-UST LP Token Pool in Lockdrop"
      );
      console.log(orion_ust_pool_init.txhash);
      console.log(
        `Lockdrop :: ORION-UST Pool successfully initialized with Lockdrop \n`
      );
      network.orion_ust_lockdrop_pool_initialized = true;
      writeArtifact(network, terra.config.chainID);
    }

    // Initialize STT-UST Pool in Lockdrop
    // Initialize STT-UST Pool in Lockdrop
    if (!network.stt_ust_lockdrop_pool_initialized) {
      let stt_ust_init_msg = {
        initialize_pool: {
          terraswap_lp_token: network.stt_ust_terraswap_lp_token_address,
          incentives_share: STT_UST_ASTRO_INCENTIVES,
        },
      };
      console.log(
        `${terra.config.chainID} :: Initializing STT-UST LP Token Pool in Lockdrop...`
      );
      let stt_ust_pool_init = await executeContract(
        terra,
        wallet,
        network.lockdropAddress,
        stt_ust_init_msg,
        [],
        "Initializing STT-UST LP Token Pool in Lockdrop"
      );
      console.log(stt_ust_pool_init.txhash);
      console.log(
        `Lockdrop :: STT-UST Pool successfully initialized with Lockdrop \n`
      );
      network.stt_ust_lockdrop_pool_initialized = true;
      writeArtifact(network, terra.config.chainID);
    }

    // Initialize VKR-UST Pool in Lockdrop
    // Initialize VKR-UST Pool in Lockdrop
    if (!network.vkr_ust_lockdrop_pool_initialized) {
      let vkr_ust_init_msg = {
        initialize_pool: {
          terraswap_lp_token: network.vkr_ust_terraswap_lp_token_address,
          incentives_share: VKR_UST_ASTRO_INCENTIVES,
        },
      };

      console.log(
        `${terra.config.chainID} :: Initializing VKR-UST LP Token Pool in Lockdrop...`
      );
      let vkr_ust_pool_init = await executeContract(
        terra,
        wallet,
        network.lockdropAddress,
        vkr_ust_init_msg,
        [],
        "Initializing VKR-UST LP Token Pool in Lockdrop"
      );
      console.log(vkr_ust_pool_init.txhash);
      console.log(
        `Lockdrop :: VKR-UST Pool successfully initialized with Lockdrop \n`
      );
      network.vkr_ust_lockdrop_pool_initialized = true;
      writeArtifact(network, terra.config.chainID);
    }

    // Initialize MINE-UST Pool in Lockdrop
    // Initialize MINE-UST Pool in Lockdrop
    if (!network.mine_ust_lockdrop_pool_initialized) {
      let mine_ust_init_msg = {
        initialize_pool: {
          terraswap_lp_token: network.mine_ust_terraswap_lp_token_address,
          incentives_share: MINE_UST_ASTRO_INCENTIVES,
        },
      };

      console.log(
        `${terra.config.chainID} :: Initializing MINE-UST LP Token Pool in Lockdrop...`
      );
      let mine_ust_pool_init = await executeContract(
        terra,
        wallet,
        network.lockdropAddress,
        mine_ust_init_msg,
        [],
        "Initialize MINE-UST Pool in Lockdrop"
      );
      console.log(mine_ust_pool_init.txhash);
      console.log(
        `Lockdrop :: MINE-UST Pool successfully initialized with Lockdrop \n`
      );
      network.mine_ust_lockdrop_pool_initialized = true;
      writeArtifact(network, terra.config.chainID);
    }

    // Initialize PSI-UST Pool in Lockdrop
    // Initialize PSI-UST Pool in Lockdrop
    if (!network.psi_ust_lockdrop_pool_initialized) {
      let psi_ust_init_msg = {
        initialize_pool: {
          terraswap_lp_token: network.psi_ust_terraswap_lp_token_address,
          incentives_share: PSI_UST_ASTRO_INCENTIVES,
        },
      };

      console.log(
        `${terra.config.chainID} :: Initializing PSI-UST LP Token Pool in Lockdrop...`
      );
      let psi_ust_pool_init = await executeContract(
        terra,
        wallet,
        network.lockdropAddress,
        psi_ust_init_msg,
        [],
        " Initializing PSI-UST LP Token Pool in Lockdrop"
      );
      console.log(psi_ust_pool_init.txhash);
      console.log(
        `Lockdrop :: PSI-UST Pool successfully initialized with Lockdrop \n`
      );
      network.psi_ust_lockdrop_pool_initialized = true;
      writeArtifact(network, terra.config.chainID);
    }

    // Initialize APOLLO-UST Pool with incentive
    // Initialize APOLLO-UST Pool with incentive
    if (!network.apollo_ust_lockdrop_pool_initialized) {
      let apollo_ust_init_msg = {
        initialize_pool: {
          terraswap_lp_token: network.apollo_ust_terraswap_lp_token_address,
          incentives_share: APOLLO_UST_ASTRO_INCENTIVES,
        },
      };
      console.log(
        `${terra.config.chainID} :: Initializing APOLLO-UST LP Token Pool in Lockdrop...`
      );
      let apollo_ust_pool_init = await executeContract(
        terra,
        wallet,
        network.lockdropAddress,
        apollo_ust_init_msg,
        [],
        "Initializing APOLLO-UST LP Token Pool in Lockdrop"
      );
      console.log(apollo_ust_pool_init.txhash);
      console.log(
        `Lockdrop :: APOLLO-UST Pool successfully initialized with Lockdrop \n`
      );
      network.apollo_ust_lockdrop_pool_initialized = true;
      writeArtifact(network, terra.config.chainID);
    }
  }

  /*************************************** LOCKDROP :: IF NETWORK IS COLUMBUS-5  *****************************************/
  /*************************************** LOCKDROP :: IF NETWORK IS COLUMBUS-5  *****************************************/
  /*************************************** LOCKDROP :: IF NETWORK IS COLUMBUS-5  *****************************************/
  /*************************************** LOCKDROP :: IF NETWORK IS COLUMBUS-5  *****************************************/

  if (terra.config.chainID == "columbus-5") {
    // Multisig details:
    // Multisig details:
    // Multisig details:
    const MULTISIG_PUBLIC_KEYS = process.env
      .MULTISIG_PUBLIC_KEYS!.split(",")
      // terrad sorts keys of multisigs by comparing bytes of their address
      .sort((a, b) => {
        return Buffer.from(new SimplePublicKey(a).rawAddress()).compare(
          Buffer.from(new SimplePublicKey(b).rawAddress())
        );
      })
      .map((x) => new SimplePublicKey(x));

    const MULTISIG_THRESHOLD = parseInt(process.env.MULTISIG_THRESHOLD!);

    // PubKey
    const multisigPubKey = new LegacyAminoMultisigPublicKey(
      MULTISIG_THRESHOLD,
      MULTISIG_PUBLIC_KEYS
    );
    const multisigAddress = multisigPubKey.address();
    console.log("Astroport Multi-Sig:", multisigAddress);

    const accInfo = await terra.auth.accountInfo(multisigAddress);
    let sequence_number = accInfo.getSequenceNumber();

    // Purpose:  SET ASTRO Token and Auction Contract in Lockdrop
    // Contract Address: "Lockdrop Contract"
    if (!network.lockdrop_astro_token_set && !network.auction_set_in_lockdrop) {
      console.log(
        `${terra.config.chainID} :: Need to make Multi-sig tx to set ASTRO token address & Auction contract address in Lockdrop contract`
      );

      let unsigned_lockdrop_set_astro_and_auction =
        await executeContractJsonForMultiSig(
          terra,
          multisigAddress,
          sequence_number,
          accInfo.getPublicKey(),
          network.lockdropAddress,
          {
            update_config: {
              new_config: {
                owner: undefined,
                astro_token_address: network.astrotokenAddress,
                auction_contract_address: network.auction_Address,
                generator_address: undefined,
              },
            },
          },
          CONFIGURATION.memos.lockdrop_set_astro
        );
      writeArtifact(
        unsigned_lockdrop_set_astro_and_auction,
        `${sequence_number}-unsigned_lockdrop_set_astro_and_auction`
      );
      console.log(
        `${terra.config.chainID} :: ${sequence_number}-unsigned_lockdrop_set_astro_and_auction.json successfully created.\n`
      );
      network.lockdrop_astro_token_set = true;
      network.auction_set_in_lockdrop = true;
      writeArtifact(network, terra.config.chainID);
      sequence_number += 1;
    }

    // Purpose:  SET Auction Contract in Airdrop
    // Contract Address: "Airdrop Contract"
    if (!network.auction_set_in_airdrop) {
      console.log("Set auction_address in Airdrop Contract ...");
      // update Config Tx
      let out = await executeContract(
        terra,
        wallet,
        network.airdrop_Address,
        {
          update_config: {
            owner: undefined,
            auction_contract_address: network.auction_Address,
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
      sequence_number += 1;
    }

    // Purpose:  Transfer ASTRO to Lockdrop and set total incentives
    // Contract Address: "Lockdrop Contract"
    if (!network.lockdrop_astro_token_transferred) {
      console.log(
        `${terra.config.chainID} :: Need to make Multi-sig tx to transfer ASTRO and set incentives in Lockdrop contract`
      );

      let unsigned_lockdrop_increase_astro_incentives =
        await executeContractJsonForMultiSig(
          terra,
          multisigAddress,
          sequence_number,
          accInfo.getPublicKey(),
          network.lockdropAddress,
          {
            send: {
              contract: network.lockdropAddress,
              amount: String(LOCKDROP_INCENTIVES),
              msg: Buffer.from(
                JSON.stringify({ increase_astro_incentives: {} })
              ).toString("base64"),
            },
          },
          "Transfer ASTRO and set Lockdrop incentives"
        );
      writeArtifact(
        unsigned_lockdrop_increase_astro_incentives,
        `${sequence_number}-unsigned_lockdrop_increase_astro_incentives`
      );
      console.log(
        `${terra.config.chainID} :: ${sequence_number}-unsigned_lockdrop_increase_astro_incentives.json successfully created.\n`
      );
      network.lockdrop_astro_token_transferred = true;
      writeArtifact(network, terra.config.chainID);
      sequence_number += 1;
    }

    // Purpose:  Transfer ASTRO to Airdrop
    // Contract Address: "ASTRO Token Contract"
    if (!network.airdrop_astro_token_transferred) {
      let unsigned_transfer_astro_to_airdrop =
        await executeContractJsonForMultiSig(
          terra,
          multisigAddress,
          sequence_number,
          accInfo.getPublicKey(),
          network.lockdropAddress,
          {
            transfer: {
              recipient: network.airdrop_Address,
              amount: String(AIRDROP_INCENTIVES),
            },
          },
          "Transfer ASTRO to Airdrop Contract"
        );
      writeArtifact(
        unsigned_transfer_astro_to_airdrop,
        `${sequence_number}-unsigned_transfer_astro_to_airdrop`
      );
      network.airdrop_astro_token_transferred = true;
      console.log(
        `${terra.config.chainID} :: ${sequence_number}-unsigned_transfer_astro_to_airdrop.json successfully created.\n`
      );
      sequence_number += 1;
    }

    // Purpose:  Transfer ASTRO to Auction to set incentives
    // Contract Address: "ASTRO Token Contract"
    if (!network.auction_astro_token_transferred) {
      let unsigned_transfer_astro_to_auction =
        await executeContractJsonForMultiSig(
          terra,
          multisigAddress,
          sequence_number,
          accInfo.getPublicKey(),
          network.auction_Address,
          {
            send: {
              contract: network.auction_Address,
              amount: String(AUCTION_INCENTIVES),
              msg: Buffer.from(
                JSON.stringify({ increase_astro_incentives: {} })
              ).toString("base64"),
            },
          },
          "Transfer ASTRO to Auction Contract for participation incentives"
        );
      writeArtifact(
        unsigned_transfer_astro_to_auction,
        `${sequence_number}-unsigned_transfer_astro_to_auction`
      );
      console.log(
        `${terra.config.chainID} :: ${sequence_number}-unsigned_transfer_astro_to_auction.json successfully created.\n`
      );
      sequence_number += 1;
    }

    // Purpose: Initialize LUNA-UST Pool in Lockdrop
    // Contract Address: "Lockdrop Contract"
    if (!network.luna_ust_lockdrop_pool_initialized) {
      console.log(
        `${terra.config.chainID} :: Need to make Multi-sig tx to initialize Luna-ust pool in Lockdrop contract`
      );
      let unsigned_lockdrop_luna_ust_pool_init =
        await executeContractJsonForMultiSig(
          terra,
          multisigAddress,
          sequence_number,
          accInfo.getPublicKey(),
          network.lockdropAddress,
          {
            initialize_pool: {
              terraswap_lp_token: network.luna_ust_terraswap_lp_token_address,
              incentives_share: LUNA_UST_ASTRO_INCENTIVES,
            },
          },
          "Initialize LUNA-UST Pool in Lockdrop"
        );
      writeArtifact(
        unsigned_lockdrop_luna_ust_pool_init,
        `${sequence_number}-unsigned_lockdrop_luna_ust_pool_init`
      );
      console.log(
        `${terra.config.chainID} :: ${sequence_number}-unsigned_lockdrop_luna_ust_pool_init.json successfully created. \n`
      );
      network.luna_ust_lockdrop_pool_initialized = true;
      writeArtifact(network, terra.config.chainID);
      sequence_number += 1;
    }

    // Purpose: Initialize LUNA-BLUNA Pool in Lockdrop
    // Contract Address: "Lockdrop Contract"
    if (!network.bluna_luna_lockdrop_pool_initialized) {
      let bluna_luna_init_msg = {
        initialize_pool: {
          terraswap_lp_token: network.bluna_luna_terraswap_lp_token_address,
          incentives_share: LUNA_BLUNA_ASTRO_INCENTIVES,
        },
      };
      console.log(
        `${terra.config.chainID} :: Need to make Multi-sig tx to initialize Luna-bluna pool in Lockdrop contract`
      );
      let unsigned_lockdrop_bluna_luna_pool_init =
        await executeContractJsonForMultiSig(
          terra,
          multisigAddress,
          sequence_number,
          accInfo.getPublicKey(),
          network.lockdropAddress,
          {
            initialize_pool: {
              terraswap_lp_token: network.bluna_luna_terraswap_lp_token_address,
              incentives_share: LUNA_BLUNA_ASTRO_INCENTIVES,
            },
          },
          "Initializing LUNA-BLUNA LP Pool in Lockdrop"
        );
      writeArtifact(
        unsigned_lockdrop_bluna_luna_pool_init,
        `${sequence_number}-unsigned_lockdrop_bluna_luna_pool_init`
      );
      console.log(
        `${terra.config.chainID} :: ${sequence_number}-unsigned_lockdrop_bluna_luna_pool_init.json successfully created. \n`
      );
      network.bluna_luna_lockdrop_pool_initialized = true;
      writeArtifact(network, terra.config.chainID);
      sequence_number += 1;
    }

    // Purpose: Initialize ANC-UST Pool in Lockdrop
    // Contract Address: "Lockdrop Contract"
    if (!network.anc_ust_lockdrop_pool_initialized) {
      let anc_ust_init_msg = {
        initialize_pool: {
          terraswap_lp_token: network.anc_ust_terraswap_lp_token_address,
          incentives_share: ANC_UST_ASTRO_INCENTIVES,
        },
      };
      console.log(
        `${terra.config.chainID} :: Need to make Multi-sig tx to initialize ANC-UST pool in Lockdrop contract`
      );
      let unsigned_lockdrop_anc_ust_pool_init =
        await executeContractJsonForMultiSig(
          terra,
          multisigAddress,
          sequence_number,
          accInfo.getPublicKey(),
          network.lockdropAddress,
          anc_ust_init_msg,
          "Initialize ANC-UST Pool in Lockdrop"
        );
      writeArtifact(
        unsigned_lockdrop_anc_ust_pool_init,
        `${sequence_number}-unsigned_lockdrop_anc_ust_pool_init`
      );
      console.log(
        `${terra.config.chainID} :: ${sequence_number}-unsigned_lockdrop_anc_ust_pool_init.json successfully created. \n`
      );
      network.anc_ust_lockdrop_pool_initialized = true;
      writeArtifact(network, terra.config.chainID);
      sequence_number += 1;
    }

    // Purpose: Initialize MIR-UST Pool in Lockdrop
    // Contract Address: "Lockdrop Contract"
    if (!network.mir_ust_lockdrop_pool_initialized) {
      let mir_ust_init_msg = {
        initialize_pool: {
          terraswap_lp_token: network.mir_ust_terraswap_lp_token_address,
          incentives_share: MIR_UST_ASTRO_INCENTIVES,
        },
      };
      console.log(
        `${terra.config.chainID} :: Need to make Multi-sig tx to initialize MIR-UST pool in Lockdrop contract`
      );

      let unsigned_lockdrop_mir_ust_pool_init =
        await executeContractJsonForMultiSig(
          terra,
          multisigAddress,
          sequence_number,
          accInfo.getPublicKey(),
          network.lockdropAddress,
          mir_ust_init_msg,
          "Initialize MIR-UST Pool in Lockdrop"
        );
      writeArtifact(
        unsigned_lockdrop_mir_ust_pool_init,
        `${sequence_number}-unsigned_lockdrop_mir_ust_pool_init`
      );
      console.log(
        `${terra.config.chainID} :: ${sequence_number}-unsigned_lockdrop_mir_ust_pool_init.json successfully created. \n`
      );

      network.mir_ust_lockdrop_pool_initialized = true;
      writeArtifact(network, terra.config.chainID);
      sequence_number += 1;
    }

    /*************************************** LOCKDROP ::  Initialize ORION-UST Pool with incentives  *****************************************/

    // Initialize ORION-UST Pool with incentives
    if (!network.orion_ust_lockdrop_pool_initialized) {
      let orion_ust_init_msg = {
        initialize_pool: {
          terraswap_lp_token: network.orion_ust_terraswap_lp_token_address,
          incentives_share: ORION_UST_ASTRO_INCENTIVES,
        },
      };

      console.log(
        `${terra.config.chainID} :: Need to make Multi-sig tx to initialize ORION-UST pool in Lockdrop contract`
      );

      let unsigned_lockdrop_orion_ust_pool_init =
        await executeContractJsonForMultiSig(
          terra,
          multisigAddress,
          sequence_number,
          accInfo.getPublicKey(),
          network.lockdropAddress,
          orion_ust_init_msg,
          "Initializing ORION-UST LP Token Pool in Lockdrop"
        );
      writeArtifact(
        unsigned_lockdrop_orion_ust_pool_init,
        `${sequence_number}-unsigned_lockdrop_orion_ust_pool_init`
      );
      console.log(
        `${terra.config.chainID} :: ${sequence_number}-unsigned_lockdrop_orion_ust_pool_init.json successfully created. \n`
      );

      network.orion_ust_lockdrop_pool_initialized = true;
      writeArtifact(network, terra.config.chainID);
      sequence_number += 1;
    }

    /*************************************** LOCKDROP ::  Initialize STT-UST Pool with incentives  *****************************************/

    // Initialize STT-UST Pool with incentives
    if (!network.stt_ust_lockdrop_pool_initialized) {
      let stt_ust_init_msg = {
        initialize_pool: {
          terraswap_lp_token: network.stt_ust_terraswap_lp_token_address,
          incentives_share: STT_UST_ASTRO_INCENTIVES,
        },
      };
      console.log(
        `${terra.config.chainID} :: Need to make Multi-sig tx to initialize STT-UST pool in Lockdrop contract`
      );
      let unsigned_lockdrop_stt_ust_pool_init =
        await executeContractJsonForMultiSig(
          terra,
          multisigAddress,
          sequence_number,
          accInfo.getPublicKey(),
          network.lockdropAddress,
          stt_ust_init_msg,
          "Initializing STT-UST LP Token Pool in Lockdrop"
        );
      writeArtifact(
        unsigned_lockdrop_stt_ust_pool_init,
        `${sequence_number}-unsigned_lockdrop_stt_ust_pool_init`
      );
      console.log(
        `${terra.config.chainID} :: ${sequence_number}-unsigned_lockdrop_stt_ust_pool_init.json successfully created. \n`
      );

      network.stt_ust_lockdrop_pool_initialized = true;
      writeArtifact(network, terra.config.chainID);
      sequence_number += 1;
    }

    // /*************************************** LOCKDROP ::  Initialize VKR-UST Pool with incentives  *****************************************/

    // Initialize VKR-UST Pool with incentives
    if (!network.vkr_ust_lockdrop_pool_initialized) {
      let vkr_ust_init_msg = {
        initialize_pool: {
          terraswap_lp_token: network.vkr_ust_terraswap_lp_token_address,
          incentives_share: VKR_UST_ASTRO_INCENTIVES,
        },
      };
      console.log(
        `${terra.config.chainID} :: Need to make Multi-sig tx to initialize Luna-ust pool in Lockdrop contract`
      );
      let unsigned_lockdrop_vkr_ust_pool_init =
        await executeContractJsonForMultiSig(
          terra,
          multisigAddress,
          sequence_number,
          accInfo.getPublicKey(),
          network.lockdropAddress,
          vkr_ust_init_msg,
          "Initializing VKR-UST LP Token Pool in Lockdrop"
        );
      writeArtifact(
        unsigned_lockdrop_vkr_ust_pool_init,
        `${sequence_number}-unsigned_lockdrop_vkr_ust_pool_init`
      );
      console.log(
        `${terra.config.chainID} :: ${sequence_number}-unsigned_lockdrop_vkr_ust_pool_init.json successfully created. \n`
      );
      network.vkr_ust_lockdrop_pool_initialized = true;
      writeArtifact(network, terra.config.chainID);
      sequence_number += 1;
    }

    // /*************************************** LOCKDROP ::  Initialize MINE-UST Pool with incentives  *****************************************/

    // Initialize MINE-UST Pool with incentives
    if (!network.mine_ust_lockdrop_pool_initialized) {
      let mine_ust_init_msg = {
        initialize_pool: {
          terraswap_lp_token: network.mine_ust_terraswap_lp_token_address,
          incentives_share: MINE_UST_ASTRO_INCENTIVES,
        },
      };
      console.log(
        `${terra.config.chainID} :: Need to make Multi-sig tx to initialize Luna-ust pool in Lockdrop contract`
      );
      let unsigned_lockdrop_mine_ust_pool_init =
        await executeContractJsonForMultiSig(
          terra,
          multisigAddress,
          sequence_number,
          accInfo.getPublicKey(),
          network.lockdropAddress,
          mine_ust_init_msg,
          "Initializing STT-UST LP Token Pool in Lockdrop"
        );
      writeArtifact(
        unsigned_lockdrop_mine_ust_pool_init,
        `${sequence_number}-unsigned_lockdrop_mine_ust_pool_init`
      );
      console.log(
        `${terra.config.chainID} :: ${sequence_number}-unsigned_lockdrop_mine_ust_pool_init.json successfully created. \n`
      );

      network.mine_ust_lockdrop_pool_initialized = true;
      writeArtifact(network, terra.config.chainID);
      sequence_number += 1;
    }

    // /*************************************** LOCKDROP ::  Initialize PSI-UST Pool with incentives  *****************************************/

    //  Initialize PSI-UST Pool with incentives
    if (!network.psi_ust_lockdrop_pool_initialized) {
      let psi_ust_init_msg = {
        initialize_pool: {
          terraswap_lp_token: network.psi_ust_terraswap_lp_token_address,
          incentives_share: PSI_UST_ASTRO_INCENTIVES,
        },
      };
      console.log(
        `${terra.config.chainID} :: Need to make Multi-sig tx to initialize Luna-ust pool in Lockdrop contract`
      );
      let unsigned_lockdrop_psi_ust_pool_init =
        await executeContractJsonForMultiSig(
          terra,
          multisigAddress,
          sequence_number,
          accInfo.getPublicKey(),
          network.lockdropAddress,
          psi_ust_init_msg,
          "Initializing STT-UST LP Token Pool in Lockdrop"
        );
      writeArtifact(
        unsigned_lockdrop_psi_ust_pool_init,
        `${sequence_number}-unsigned_lockdrop_psi_ust_pool_init`
      );
      console.log(
        `${terra.config.chainID} :: ${sequence_number}-unsigned_lockdrop_psi_ust_pool_init.json successfully created. \n`
      );

      network.psi_ust_lockdrop_pool_initialized = true;
      writeArtifact(network, terra.config.chainID);
      sequence_number += 1;
    }

    // /*************************************** LOCKDROP ::  Initialize APOLLO-UST Pool with incentives  *****************************************/

    // Initialize APOLLO-UST Pool with incentives
    if (!network.apollo_ust_lockdrop_pool_initialized) {
      let apollo_ust_init_msg = {
        initialize_pool: {
          terraswap_lp_token: network.apollo_ust_terraswap_lp_token_address,
          incentives_share: APOLLO_UST_ASTRO_INCENTIVES,
        },
      };
      console.log(
        `${terra.config.chainID} :: Need to make Multi-sig tx to initialize APOLLO-UST pool in Lockdrop contract`
      );
      let unsigned_lockdrop_apollo_ust_pool_init =
        await executeContractJsonForMultiSig(
          terra,
          multisigAddress,
          sequence_number,
          accInfo.getPublicKey(),
          network.lockdropAddress,
          apollo_ust_init_msg,
          "Initializing APOLLO-UST LP Token Pool in Lockdrop"
        );
      writeArtifact(
        unsigned_lockdrop_apollo_ust_pool_init,
        `${sequence_number}-unsigned_lockdrop_apollo_ust_pool_init`
      );
      console.log(
        `${terra.config.chainID} :: ${sequence_number}-unsigned_lockdrop_apollo_ust_pool_init.json successfully created. \n`
      );
      network.apollo_ust_lockdrop_pool_initialized = true;
      writeArtifact(network, terra.config.chainID);
      sequence_number += 1;
    }
  }

  writeArtifact(network, terra.config.chainID);
  console.log("FINISH");
}

main().catch(console.log);
