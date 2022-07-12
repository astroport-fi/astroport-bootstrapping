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

let MULTI_SIG_TO_USE = "";

async function main() {
  let CONFIGURATION: Config;

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
    CONFIGURATION = pisco_testnet;
  } else if (terra.config.chainID == "columbus-5") {
    CONFIGURATION = mainnet;
  }

  // ASTRO token addresss should be set
  if (!network.lockdrop_address) {
    console.log(
      `Please deploy the Lockdrop Contract in the deploy config before running this script...`
    );
    return;
  }

  /*************************************** LOCKDROP :: IF NETWORK IS BOMBAY-12  *****************************************/

  if (terra.config.chainID == "bombay-12") {
    // Migrating Liquidity to Astroport :: LUNA/UST
    if (!network.luna_ust_liquidity_migrated) {
      // if Astroport pool address not provided
      if (
        !network.luna_ust_astroport_pool ||
        network.luna_ust_astroport_pool == ""
      ) {
        console.log(
          `${terra.config.chainID} :: Set LUNA/UST Astroport pool address to migrate liquidity`
        );
      } else {
        console.log(
          `${terra.config.chainID} :: Lockdrop :: Migrating Liquidity to Astroport :: LUNA/UST`
        );
        let tx = await executeContract(
          terra,
          wallet,
          network.lockdrop_address,
          {
            migrate_liquidity: {
              terraswap_lp_token: network.luna_ust_terraswap_lp_token_address,
              astroport_pool_addr: network.luna_ust_astroport_pool,
            },
          },
          [],
          "Lockdrop :: Liquidity Migration to Astroport :: LUNA/UST"
        );

        console.log(
          `Lockdrop :: Liquidity successfully migrated :: LUNA/UST :: ${tx.txhash}\n`
        );
        network.luna_ust_liquidity_migrated = true;
        writeArtifact(network, terra.config.chainID);
      }
    }

    // Migrating Liquidity to Astroport :: BLUNA/LUNA
    if (!network.bluna_luna_liquidity_migrated) {
      // if Astroport pool address not provided
      if (
        !network.bluna_luna_astroport_pool ||
        network.bluna_luna_astroport_pool == ""
      ) {
        console.log(
          `${terra.config.chainID} :: Set BLUNA/LUNA Astroport pool address to migrate liquidity`
        );
      } else {
        console.log(
          `${terra.config.chainID} :: Lockdrop :: Migrating Liquidity to Astroport :: BLUNA/LUNA`
        );
        let tx = await executeContract(
          terra,
          wallet,
          network.lockdrop_address,
          {
            migrate_liquidity: {
              terraswap_lp_token: network.bluna_luna_terraswap_lp_token_address,
              astroport_pool_addr: network.bluna_luna_astroport_pool,
            },
          },
          [],
          "Lockdrop :: Liquidity Migration to Astroport :: BLUNA/LUNA"
        );

        console.log(
          `Lockdrop :: Liquidity successfully migrated :: BLUNA/LUNA :: ${tx.txhash}\n`
        );
        network.bluna_luna_liquidity_migrated = true;
        writeArtifact(network, terra.config.chainID);
      }
    }

    // Migrating Liquidity to Astroport :: ANC/UST
    if (!network.anc_ust_liquidity_migrated) {
      // if Astroport pool address not provided
      if (
        !network.anc_ust_astroport_pool ||
        network.anc_ust_astroport_pool == ""
      ) {
        console.log(
          `${terra.config.chainID} :: Set ANC/UST Astroport pool address to migrate liquidity`
        );
      } else {
        console.log(
          `${terra.config.chainID} :: Lockdrop :: Migrating Liquidity to Astroport :: ANC/UST`
        );
        let tx = await executeContract(
          terra,
          wallet,
          network.lockdrop_address,
          {
            migrate_liquidity: {
              terraswap_lp_token: network.anc_ust_terraswap_lp_token_address,
              astroport_pool_addr: network.anc_ust_astroport_pool,
            },
          },
          [],
          "Lockdrop :: Liquidity Migration to Astroport :: ANC/UST"
        );

        console.log(
          `Lockdrop :: Liquidity successfully migrated :: ANC/UST :: ${tx.txhash}\n`
        );
        network.anc_ust_liquidity_migrated = true;
        writeArtifact(network, terra.config.chainID);
      }
    }

    // Migrating Liquidity to Astroport :: MIR/UST
    if (!network.mir_ust_liquidity_migrated) {
      // if Astroport pool address not provided
      if (
        !network.mir_ust_astroport_pool ||
        network.mir_ust_astroport_pool == ""
      ) {
        console.log(
          `${terra.config.chainID} :: Set MIR/UST Astroport pool address to migrate liquidity`
        );
      } else {
        console.log(
          `${terra.config.chainID} :: Lockdrop :: Migrating Liquidity to Astroport :: MIR/UST`
        );
        let tx = await executeContract(
          terra,
          wallet,
          network.lockdrop_address,
          {
            migrate_liquidity: {
              terraswap_lp_token: network.mir_ust_terraswap_lp_token_address,
              astroport_pool_addr: network.mir_ust_astroport_pool,
            },
          },
          [],
          "Lockdrop :: Liquidity Migration to Astroport :: MIR/UST"
        );

        console.log(
          `Lockdrop :: Liquidity successfully migrated :: MIR/UST :: ${tx.txhash}\n`
        );
        network.mir_ust_liquidity_migrated = true;
        writeArtifact(network, terra.config.chainID);
      }
    }

    // Migrating Liquidity to Astroport :: PSI/UST
    if (!network.psi_ust_liquidity_migrated) {
      // if Astroport pool address not provided
      if (
        !network.psi_ust_astroport_pool ||
        network.psi_ust_astroport_pool == ""
      ) {
        console.log(
          `${terra.config.chainID} :: Set PSI/UST Astroport pool address to migrate liquidity`
        );
      } else {
        console.log(
          `${terra.config.chainID} :: Lockdrop :: Migrating Liquidity to Astroport :: PSI/UST`
        );
        let tx = await executeContract(
          terra,
          wallet,
          network.lockdrop_address,
          {
            migrate_liquidity: {
              terraswap_lp_token: network.psi_ust_terraswap_lp_token_address,
              astroport_pool_addr: network.psi_ust_astroport_pool,
            },
          },
          [],
          "Lockdrop :: Liquidity Migration to Astroport :: PSI/UST"
        );

        console.log(
          `Lockdrop :: Liquidity successfully migrated :: PSI/UST :: ${tx.txhash}\n`
        );
        network.psi_ust_liquidity_migrated = true;
        writeArtifact(network, terra.config.chainID);
      }
    }

    // Migrating Liquidity to Astroport :: ORION/UST
    if (!network.orion_ust_liquidity_migrated) {
      // if Astroport pool address not provided
      if (
        !network.orion_ust_astroport_pool ||
        network.orion_ust_astroport_pool == ""
      ) {
        console.log(
          `${terra.config.chainID} :: Set ORION/UST Astroport pool address to migrate liquidity`
        );
      } else {
        console.log(
          `${terra.config.chainID} :: Lockdrop :: Migrating Liquidity to Astroport :: ORION/UST`
        );
        let tx = await executeContract(
          terra,
          wallet,
          network.lockdrop_address,
          {
            migrate_liquidity: {
              terraswap_lp_token: network.orion_ust_terraswap_lp_token_address,
              astroport_pool_addr: network.orion_ust_astroport_pool,
            },
          },
          [],
          "Lockdrop :: Liquidity Migration to Astroport :: ORION/UST"
        );

        console.log(
          `Lockdrop :: Liquidity successfully migrated :: ORION/UST :: ${tx.txhash}\n`
        );
        network.orion_ust_liquidity_migrated = true;
        writeArtifact(network, terra.config.chainID);
      }
    }

    // Migrating Liquidity to Astroport :: STT/UST
    if (!network.stt_ust_liquidity_migrated) {
      // if Astroport pool address not provided
      if (
        !network.stt_ust_astroport_pool ||
        network.stt_ust_astroport_pool == ""
      ) {
        console.log(
          `${terra.config.chainID} :: Set STT/UST Astroport pool address to migrate liquidity`
        );
      } else {
        console.log(
          `${terra.config.chainID} :: Lockdrop :: Migrating Liquidity to Astroport :: STT/UST`
        );
        let tx = await executeContract(
          terra,
          wallet,
          network.lockdrop_address,
          {
            migrate_liquidity: {
              terraswap_lp_token: network.stt_ust_terraswap_lp_token_address,
              astroport_pool_addr: network.stt_ust_astroport_pool,
            },
          },
          [],
          "Lockdrop :: Liquidity Migration to Astroport :: STT/UST"
        );

        console.log(
          `Lockdrop :: Liquidity successfully migrated :: STT/UST :: ${tx.txhash}\n`
        );
        network.stt_ust_liquidity_migrated = true;
        writeArtifact(network, terra.config.chainID);
      }
    }

    // Migrating Liquidity to Astroport :: VKR/UST
    if (!network.vkr_ust_liquidity_migrated) {
      // if Astroport pool address not provided
      if (
        !network.vkr_ust_astroport_pool ||
        network.vkr_ust_astroport_pool == ""
      ) {
        console.log(
          `${terra.config.chainID} :: Set VKR/UST Astroport pool address to migrate liquidity`
        );
      } else {
        console.log(
          `${terra.config.chainID} :: Lockdrop :: Migrating Liquidity to Astroport :: VKR/UST`
        );
        let tx = await executeContract(
          terra,
          wallet,
          network.lockdrop_address,
          {
            migrate_liquidity: {
              terraswap_lp_token: network.vkr_ust_terraswap_lp_token_address,
              astroport_pool_addr: network.vkr_ust_astroport_pool,
            },
          },
          [],
          "Lockdrop :: Liquidity Migration to Astroport :: VKR/UST"
        );

        console.log(
          `Lockdrop :: Liquidity successfully migrated :: VKR/UST :: ${tx.txhash}\n`
        );
        network.vkr_ust_liquidity_migrated = true;
        writeArtifact(network, terra.config.chainID);
      }
    }

    // Migrating Liquidity to Astroport :: MINE/UST
    if (!network.mine_ust_liquidity_migrated) {
      // if Astroport pool address not provided
      if (
        !network.mine_ust_astroport_pool ||
        network.mine_ust_astroport_pool == ""
      ) {
        console.log(
          `${terra.config.chainID} :: Set MINE/UST Astroport pool address to migrate liquidity`
        );
      } else {
        console.log(
          `${terra.config.chainID} :: Lockdrop :: Migrating Liquidity to Astroport :: MINE/UST`
        );
        let tx = await executeContract(
          terra,
          wallet,
          network.lockdrop_address,
          {
            migrate_liquidity: {
              terraswap_lp_token: network.mine_ust_terraswap_lp_token_address,
              astroport_pool_addr: network.mine_ust_astroport_pool,
            },
          },
          [],
          "Lockdrop :: Liquidity Migration to Astroport :: MINE/UST"
        );

        console.log(
          `Lockdrop :: Liquidity successfully migrated :: MINE/UST :: ${tx.txhash}\n`
        );
        network.mine_ust_liquidity_migrated = true;
        writeArtifact(network, terra.config.chainID);
      }
    }

    // Migrating Liquidity to Astroport :: APOLLO/UST
    if (!network.apollo_ust_liquidity_migrated) {
      // if Astroport pool address not provided
      if (
        !network.apollo_ust_astroport_pool ||
        network.apollo_ust_astroport_pool == ""
      ) {
        console.log(
          `${terra.config.chainID} :: Set APOLLO/UST Astroport pool address to migrate liquidity`
        );
      } else {
        console.log(
          `${terra.config.chainID} :: Lockdrop :: Migrating Liquidity to Astroport :: APOLLO/UST`
        );
        let tx = await executeContract(
          terra,
          wallet,
          network.lockdrop_address,
          {
            migrate_liquidity: {
              terraswap_lp_token: network.apollo_ust_terraswap_lp_token_address,
              astroport_pool_addr: network.apollo_ust_astroport_pool,
            },
          },
          [],
          "Lockdrop :: Liquidity Migration to Astroport :: APOLLO/UST"
        );

        console.log(
          `Lockdrop :: Liquidity successfully migrated :: APOLLO/UST :: ${tx.txhash}\n`
        );
        network.apollo_ust_liquidity_migrated = true;
        writeArtifact(network, terra.config.chainID);
      }
    }
  }

  console.log("FINISH");
}

main().catch(console.log);
