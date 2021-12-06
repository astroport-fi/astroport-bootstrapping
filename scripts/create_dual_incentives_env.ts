import "dotenv/config";
import { Coin } from "@terra-money/terra.js";
import {
  uploadContract,
  instantiateContract,
  executeContract,
  newClient,
  queryContract,
  readArtifact,
  writeArtifact,
  extract_terraswap_pool_info,
  Client,
} from "./helpers/helpers.js";
import { join } from "path";

const ARTIFACTS_PATH = "../artifacts";

// ########### LOCKDROP :: LP TOKENS ELIGIBLE ###########

// - LUNA/UST - dual incentives : ASTRO only
// - LUNA/BLUNA - dual incentives : ASTRO only
// - ANC/UST - dual incentives : ASTRO and ANC        [.wasm available]
// - MIR/UST - dual incentives : ASTRO and MIR        [.wasm available]
// - ORION/UST - dual incentives : ASTRO only         [.wasm available]
// - STT/UST - dual incentives : ASTRO and STT        [.wasm available]
// - VKR/UST - dual incentives : ASTRO and VKR        [.wasm available]
// - MINE/UST - dual incentives : ASTRO and MINE      [.wasm available]
// - PSI/UST - dual incentives : ASTRO and PSI        [.wasm available]
// - APOLLO/UST - dual incentives : ASTRO and APOLLO  [.wasm available]

async function main() {
  const { terra, wallet } = newClient();

  console.log(
    `chainID: ${terra.config.chainID} wallet: ${wallet.key.accAddress}`
  );

  if (terra.config.chainID != "bombay-12") {
    console.log(`network must be bombay-12`);
    return;
  }

  const network = readArtifact(terra.config.chainID);
  console.log("network:", network);
  const INIT_TIMETAMP = 0;
  const TILL_TIMETAMP = 0;
  const INCENTIVES = 50000000000000;

  // ##################### ANC-UST STAKING CONTRACT #####################
  // ##################### ANC-UST STAKING CONTRACT #####################
  // ##################### ANC-UST STAKING CONTRACT #####################

  // ANC-UST STAKING CONTRACT ID
  if (!network.anc_lp_staking_contract_code_id) {
    network.anc_lp_staking_contract_code_id = await uploadContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "anchor_staking.wasm")
    );
    console.log(`anc_staking id = ${network.anc_lp_staking_contract_code_id}`);
    writeArtifact(network, terra.config.chainID);
  }

  // Deploy :: ANC-UST STAKING CONTRACT
  if (!network.anc_lp_staking_contract_address) {
    network.anc_lp_staking_contract_address = await instantiateContract(
      terra,
      wallet,
      network.anc_lp_staking_contract_code_id,
      {
        anchor_token: network.anc_token,
        staking_token: network.anc_ust_terraswap_lp_token_address,
        distribution_schedule: [INIT_TIMETAMP, TILL_TIMETAMP, INCENTIVES],
      }
    );
    console.log(
      `ANC-UST STAKING CONTRACT deployed successfully, address : ${network.anc_lp_staking_contract_address}`
    );
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(`ANC-UST STAKING CONTRACT already deployed on bombay-12`);
  }

  // ##################### MIR-UST STAKING CONTRACT #####################
  // ##################### MIR-UST STAKING CONTRACT #####################
  // ##################### MIR-UST STAKING CONTRACT #####################

  // MIR-UST STAKING CONTRACT ID
  if (!network.mir_lp_staking_contract_code_id) {
    network.mir_lp_staking_contract_code_id = await uploadContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "mirror_staking.wasm")
    );
    console.log(`mir_staking id = ${network.mir_lp_staking_contract_code_id}`);
    writeArtifact(network, terra.config.chainID);
  }

  // Deploy :: MIR-UST STAKING CONTRACT
  if (!network.mir_lp_staking_contract_address) {
    network.mir_lp_staking_contract_address = await instantiateContract(
      terra,
      wallet,
      network.mir_lp_staking_contract_code_id,
      {
        owner: wallet.key.accAddress,
        mirror_token: network.mir_token,
        mint_contract: wallet.key.accAddress,
        oracle_contract: wallet.key.accAddress,
        terraswap_factory: wallet.key.accAddress,
        base_denom: "uusd",
        premium_min_update_interval: wallet.key.accAddress,
        short_reward_contract: wallet.key.accAddress,
      }
    );
    console.log(
      `MIR-UST STAKING CONTRACT deployed successfully, address : ${network.mir_lp_staking_contract_address}`
    );
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(`MIR-UST STAKING CONTRACT already deployed on bombay-12`);
  }

  // ##################### ORION-UST STAKING CONTRACT #####################
  // ##################### ORION-UST STAKING CONTRACT #####################
  // ##################### ORION-UST STAKING CONTRACT #####################

  // ORION-UST STAKING CONTRACT ID
  if (!network.orion_lp_staking_contract_code_id) {
    network.orion_lp_staking_contract_code_id = await uploadContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "orion_lp_staking.wasm")
    );
    console.log(
      `orion_staking id = ${network.orion_lp_staking_contract_code_id}`
    );
    writeArtifact(network, terra.config.chainID);
  }

  // Deploy :: ORION-UST STAKING CONTRACT
  if (!network.orion_lp_staking_contract_address) {
    network.orion_lp_staking_contract_address = await instantiateContract(
      terra,
      wallet,
      network.orion_lp_staking_contract_code_id,
      {
        reward_token: network.orion_token,
        reward_token_decimals: 6,
        staking_token: network.orion_ust_terraswap_lp_token_address,
        staking_token_decimals: 6,
      }
    );
    console.log(
      `ORION-UST STAKING CONTRACT deployed successfully, address : ${network.orion_lp_staking_contract_address}`
    );
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(`ORION-UST STAKING CONTRACT already deployed on bombay-12`);
  }

  // ##################### STT-UST STAKING CONTRACT #####################
  // ##################### STT-UST STAKING CONTRACT #####################
  // ##################### STT-UST STAKING CONTRACT #####################

  // STT-UST STAKING CONTRACT ID
  if (!network.stt_lp_staking_contract_code_id) {
    network.stt_lp_staking_contract_code_id = await uploadContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "starterra_staking.wasm")
    );
    console.log(`stt_staking id = ${network.stt_lp_staking_contract_code_id}`);
    writeArtifact(network, terra.config.chainID);
  }

  // Deploy :: STT-UST STAKING CONTRACT
  if (!network.stt_lp_staking_contract_address) {
    network.stt_lp_staking_contract_address = await instantiateContract(
      terra,
      wallet,
      network.stt_lp_staking_contract_code_id,
      {
        owner: wallet.key.accAddress,

        starterra_token: network.stt_token,
        staking_token: network.stt_ust_terraswap_lp_token_address,
        burn_address: wallet.key.accAddress,
        gateway_address: wallet.key.accAddress,
        distribution_schedule: wallet.key.accAddress,
        unbond_config: { minimum_time: "", percentage_loss: "0" },
        faction_name: "testing",
        fee_configuration: { operation: "", fee: "0" },
      }
    );
    console.log(
      `STT-UST STAKING CONTRACT deployed successfully, address : ${network.stt_lp_staking_contract_address}`
    );
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(`STT-UST STAKING CONTRACT already deployed on bombay-12`);
  }
}
