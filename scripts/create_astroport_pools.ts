import "dotenv/config";
import {
  executeContract,
  newClient,
  readArtifact,
  writeArtifact,
  extract_astroport_pool_info,
} from "./helpers/helpers.js";
import { join } from "path";

const ARTIFACTS_PATH = "../artifacts";

// ########### LOCKDROP :: LP TOKENS ELIGIBLE ###########

// - LUNA/UST
// - LUNA/BLUNA
// - ANC/UST
// - MIR/UST
// - ORION/UST
// - STT/UST
// - VKR/UST
// - MINE/UST
// - PSI/UST
// - APOLLO/UST

async function main() {
  const { terra, wallet } = newClient();
  console.log(
    `chainID: ${terra.config.chainID} wallet: ${wallet.key.accAddress}`
  );

  const network = readArtifact(terra.config.chainID);
  console.log("network:", network);

  // Astroport factory address should be set
  if (!network.astroport_factory_address) {
    console.log(
      `Please set Astroport factory address in the deploy config before running this script...`
    );
    return;
  }

  /*************************************** BOMBAY TESTNET :::: ASTROPORT :: CREATE PAIR :: LUNA/UST *****************************************/

  // ASTROPORT :: CREATE PAIR :: LUNA/UST
  // ASTROPORT :: CREATE PAIR :: LUNA/UST
  // ASTROPORT :: CREATE PAIR :: LUNA/UST
  // ASTROPORT :: CREATE PAIR :: LUNA/UST
  if (!network.luna_ust_astroport_pool) {
    console.log(
      `${terra.config.chainID} :: Creating LUNA/UST pool on Astroport`
    );
    // create pair tx
    let tx = await executeContract(
      terra,
      wallet,
      network.astroport_factory_address,
      {
        create_pair: {
          pair_type: { xyk: {} },
          asset_infos: [
            { native_token: { denom: "uusd" } },
            { native_token: { denom: "uluna" } },
          ],
          init_params: null,
        },
      },
      [],
      "Astroport :: Initializing LUNA/UST Pool"
    );
    let tx_resp = extract_astroport_pool_info(tx);
    network.luna_ust_astroport_pool = tx_resp.pool_address;
    network.luna_ust_astroport_lp_token_address = tx_resp.lp_token_address;
    writeArtifact(network, terra.config.chainID);
    console.log(
      `LUNA/UST pool on Astroport successfully initialized :: ${terra.config.chainID}\n`
    );
    await delay(300);
  } else {
    console.log(
      `LUNA/UST pool on already exists on Astroport :: ${terra.config.chainID}`
    );
  }

  // ASTROPORT :: CREATE PAIR :: LUNA/BLUNA
  // ASTROPORT :: CREATE PAIR :: LUNA/BLUNA
  // ASTROPORT :: CREATE PAIR :: LUNA/BLUNA
  // ASTROPORT :: CREATE PAIR :: LUNA/BLUNA
  if (!network.bluna_luna_astroport_pool) {
    console.log(
      `${terra.config.chainID} :: Creating LUNA/BLUNA pool on Astroport`
    );
    // create pair tx
    let tx = await executeContract(
      terra,
      wallet,
      network.astroport_factory_address,
      {
        create_pair: {
          pair_type: { stable: {} },
          asset_infos: [
            { token: { contract_addr: network.bluna_token_address } },
            { native_token: { denom: "uluna" } },
          ],
          init_params: Buffer.from(JSON.stringify({ amp: 100 })).toString(
            "base64"
          ),
        },
      },
      [],
      "Astroport :: Initializing LUNA/BLUNA Pool"
    );
    let tx_resp = extract_astroport_pool_info(tx);
    network.bluna_luna_astroport_pool = tx_resp.pool_address;
    network.bluna_luna_astroport_lp_token_address = tx_resp.lp_token_address;
    writeArtifact(network, terra.config.chainID);
    console.log(
      `LUNA/BLUNA pool on Astroport successfully initialized ${tx.txhash}:: ${terra.config.chainID}\n`
    );
    await delay(300);
  } else {
    console.log(
      `LUNA/BLUNA pool on already exists on Astroport :: ${terra.config.chainID}`
    );
  }

  // ASTROPORT :: CREATE PAIR :: ANC/UST
  // ASTROPORT :: CREATE PAIR :: ANC/UST
  // ASTROPORT :: CREATE PAIR :: ANC/UST
  // ASTROPORT :: CREATE PAIR :: ANC/UST
  if (!network.anc_ust_astroport_pool) {
    console.log(
      `${terra.config.chainID} :: Creating ANC/UST pool on Astroport`
    );
    // create pair tx
    let tx = await executeContract(
      terra,
      wallet,
      network.astroport_factory_address,
      {
        create_pair: {
          pair_type: { xyk: {} },
          asset_infos: [
            { token: { contract_addr: network.anc_token } },
            { native_token: { denom: "uusd" } },
          ],
          init_params: null,
        },
      },
      [],
      "Astroport :: Initializing ANC/UST Pool"
    );
    let tx_resp = extract_astroport_pool_info(tx);
    network.anc_ust_astroport_pool = tx_resp.pool_address;
    network.anc_ust_astroport_lp_token_address = tx_resp.lp_token_address;
    writeArtifact(network, terra.config.chainID);
    console.log(
      `ANC/UST pool on Astroport successfully initialized ${tx.txhash} :: ${terra.config.chainID}\n`
    );
    await delay(300);
  } else {
    console.log(
      `ANC/UST pool on already exists on Astroport :: ${terra.config.chainID}`
    );
  }

  // ASTROPORT :: CREATE PAIR :: MIR/UST
  // ASTROPORT :: CREATE PAIR :: MIR/UST
  // ASTROPORT :: CREATE PAIR :: MIR/UST
  // ASTROPORT :: CREATE PAIR :: MIR/UST
  if (!network.mir_ust_astroport_pool) {
    console.log(
      `${terra.config.chainID} :: Creating MIR/UST pool on Astroport`
    );
    // create pair tx
    let tx = await executeContract(
      terra,
      wallet,
      network.astroport_factory_address,
      {
        create_pair: {
          pair_type: { xyk: {} },
          asset_infos: [
            { token: { contract_addr: network.mir_token } },
            { native_token: { denom: "uusd" } },
          ],
          init_params: null,
        },
      },
      [],
      "Astroport :: Initializing MIR/UST Pool"
    );
    let tx_resp = extract_astroport_pool_info(tx);
    network.mir_ust_astroport_pool = tx_resp.pool_address;
    network.mir_ust_astroport_lp_token_address = tx_resp.lp_token_address;
    writeArtifact(network, terra.config.chainID);
    console.log(
      `MIR/UST pool on Astroport successfully initialized ${tx.txhash}:: ${terra.config.chainID}\n`
    );
    await delay(300);
  } else {
    console.log(
      `MIR/UST pool on already exists on Astroport :: ${terra.config.chainID}`
    );
  }

  // ASTROPORT :: CREATE PAIR :: ORION/UST
  // ASTROPORT :: CREATE PAIR :: ORION/UST
  // ASTROPORT :: CREATE PAIR :: ORION/UST
  // ASTROPORT :: CREATE PAIR :: ORION/UST
  if (!network.orion_ust_astroport_pool) {
    console.log(
      `${terra.config.chainID} :: Creating ORION/UST pool on Astroport`
    );
    // create pair tx
    let tx = await executeContract(
      terra,
      wallet,
      network.astroport_factory_address,
      {
        create_pair: {
          pair_type: { xyk: {} },
          asset_infos: [
            { token: { contract_addr: network.orion_token } },
            { native_token: { denom: "uusd" } },
          ],
          init_params: null,
        },
      },
      [],
      "Astroport :: Initializing ORION/UST Pool"
    );
    let tx_resp = extract_astroport_pool_info(tx);
    network.orion_ust_astroport_pool = tx_resp.pool_address;
    network.orion_ust_astroport_lp_token_address = tx_resp.lp_token_address;
    writeArtifact(network, terra.config.chainID);
    console.log(
      `ORION/UST pool on Astroport successfully initialized ${tx.txhash}:: ${terra.config.chainID}\n`
    );
    await delay(300);
  } else {
    console.log(
      `ORION/UST pool on already exists on Astroport :: ${terra.config.chainID}`
    );
  }

  // ASTROPORT :: CREATE PAIR :: STT/UST
  // ASTROPORT :: CREATE PAIR :: STT/UST
  // ASTROPORT :: CREATE PAIR :: STT/UST
  // ASTROPORT :: CREATE PAIR :: STT/UST
  if (!network.stt_ust_astroport_pool) {
    console.log(
      `${terra.config.chainID} :: Creating STT/UST pool on Astroport`
    );
    // create pair tx
    let tx = await executeContract(
      terra,
      wallet,
      network.astroport_factory_address,
      {
        create_pair: {
          pair_type: { xyk: {} },
          asset_infos: [
            { token: { contract_addr: network.stt_token } },
            { native_token: { denom: "uusd" } },
          ],
          init_params: null,
        },
      },
      [],
      "Astroport :: Initializing STT/UST Pool"
    );
    let tx_resp = extract_astroport_pool_info(tx);
    network.stt_ust_astroport_pool = tx_resp.pool_address;
    network.stt_ust_astroport_lp_token_address = tx_resp.lp_token_address;
    writeArtifact(network, terra.config.chainID);
    console.log(
      `STT/UST pool on Astroport successfully initialized ${tx.txhash}:: ${terra.config.chainID}\n`
    );
    await delay(300);
  } else {
    console.log(
      `STT/UST pool on already exists on Astroport :: ${terra.config.chainID}`
    );
  }

  // ASTROPORT :: CREATE PAIR :: VKR/UST
  // ASTROPORT :: CREATE PAIR :: VKR/UST
  // ASTROPORT :: CREATE PAIR :: VKR/UST
  // ASTROPORT :: CREATE PAIR :: VKR/UST
  if (!network.vkr_ust_astroport_pool) {
    console.log(
      `${terra.config.chainID} :: Creating VKR/UST pool on Astroport`
    );
    // create pair tx
    let tx = await executeContract(
      terra,
      wallet,
      network.astroport_factory_address,
      {
        create_pair: {
          pair_type: { xyk: {} },
          asset_infos: [
            { token: { contract_addr: network.vkr_token } },
            { native_token: { denom: "uusd" } },
          ],
          init_params: null,
        },
      },
      [],
      "Astroport :: Initializing VKR/UST Pool"
    );
    let tx_resp = extract_astroport_pool_info(tx);
    network.vkr_ust_astroport_pool = tx_resp.pool_address;
    network.vkr_ust_astroport_lp_token_address = tx_resp.lp_token_address;
    writeArtifact(network, terra.config.chainID);
    console.log(
      `VKR/UST pool on Astroport successfully initialized ${tx.txhash}:: ${terra.config.chainID}\n`
    );
    await delay(300);
  } else {
    console.log(
      `VKR/UST pool on already exists on Astroport :: ${terra.config.chainID}`
    );
  }

  // ASTROPORT :: CREATE PAIR :: MINE/UST
  // ASTROPORT :: CREATE PAIR :: MINE/UST
  // ASTROPORT :: CREATE PAIR :: MINE/UST
  // ASTROPORT :: CREATE PAIR :: MINE/UST
  if (!network.mine_ust_astroport_pool) {
    console.log(
      `${terra.config.chainID} :: Creating MINE/UST pool on Astroport`
    );
    // create pair tx
    let tx = await executeContract(
      terra,
      wallet,
      network.astroport_factory_address,
      {
        create_pair: {
          pair_type: { xyk: {} },
          asset_infos: [
            { token: { contract_addr: network.mine_token } },
            { native_token: { denom: "uusd" } },
          ],
          init_params: null,
        },
      },
      [],
      "Astroport :: Initializing MINE/UST Pool"
    );
    let tx_resp = extract_astroport_pool_info(tx);
    network.mine_ust_astroport_pool = tx_resp.pool_address;
    network.mine_ust_astroport_lp_token_address = tx_resp.lp_token_address;
    writeArtifact(network, terra.config.chainID);
    console.log(
      `MINE/UST pool on Astroport successfully initialized ${tx.txhash}:: ${terra.config.chainID}\n`
    );
    await delay(300);
  } else {
    console.log(
      `MINE/UST pool on already exists on Astroport :: ${terra.config.chainID}`
    );
  }

  // ASTROPORT :: CREATE PAIR :: PSI/UST
  // ASTROPORT :: CREATE PAIR :: PSI/UST
  // ASTROPORT :: CREATE PAIR :: PSI/UST
  // ASTROPORT :: CREATE PAIR :: PSI/UST
  if (!network.psi_ust_astroport_pool) {
    console.log(
      `${terra.config.chainID} :: Creating PSI/UST pool on Astroport`
    );
    // create pair tx
    let tx = await executeContract(
      terra,
      wallet,
      network.astroport_factory_address,
      {
        create_pair: {
          pair_type: { xyk: {} },
          asset_infos: [
            { token: { contract_addr: network.psi_token } },
            { native_token: { denom: "uusd" } },
          ],
          init_params: null,
        },
      },
      [],
      "Astroport :: Initializing PSI/UST Pool"
    );
    let tx_resp = extract_astroport_pool_info(tx);
    network.psi_ust_astroport_pool = tx_resp.pool_address;
    network.psi_ust_astroport_lp_token_address = tx_resp.lp_token_address;
    writeArtifact(network, terra.config.chainID);
    console.log(
      `PSI/UST pool on Astroport successfully initialized ${tx.txhash}:: ${terra.config.chainID}\n`
    );
    await delay(300);
  } else {
    console.log(
      `PSI/UST pool on already exists on Astroport :: ${terra.config.chainID}`
    );
  }

  // ASTROPORT :: CREATE PAIR :: APOLLO/UST
  // ASTROPORT :: CREATE PAIR :: APOLLO/UST
  // ASTROPORT :: CREATE PAIR :: APOLLO/UST
  // ASTROPORT :: CREATE PAIR :: APOLLO/UST
  if (!network.apollo_ust_astroport_pool) {
    console.log(
      `${terra.config.chainID} :: Creating APOLLO/UST pool on Astroport`
    );
    // create pair tx
    let tx = await executeContract(
      terra,
      wallet,
      network.astroport_factory_address,
      {
        create_pair: {
          pair_type: { xyk: {} },
          asset_infos: [
            { token: { contract_addr: network.apollo_token } },
            { native_token: { denom: "uusd" } },
          ],
          init_params: null,
        },
      },
      [],
      "Astroport :: Initializing APOLLO/UST Pool"
    );
    let tx_resp = extract_astroport_pool_info(tx);
    network.apollo_ust_astroport_pool = tx_resp.pool_address;
    network.apollo_ust_astroport_lp_token_address = tx_resp.lp_token_address;
    writeArtifact(network, terra.config.chainID);
    console.log(
      `APOLLO/UST pool on Astroport successfully initialized ${tx.txhash}:: ${terra.config.chainID}\n`
    );
    await delay(300);
  } else {
    console.log(
      `APOLLO/UST pool on already exists on Astroport :: ${terra.config.chainID}`
    );
  }
}

function delay(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

main().catch(console.log);
