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

  const network = readArtifact(terra.config.chainID);
  console.log("network:", network);

  // ##################### DEPLOYMENT === ANC-UST proxy contract #####################

  // ANC-UST proxy contract ID
  if (!network.anc_generator_proxy_contract_code_id) {
    network.anc_generator_proxy_contract_code_id = await uploadContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "generator_proxy_to_anc.wasm")
    );
    console.log(
      `generator_proxy_to_anc id = ${network.anc_generator_proxy_contract_code_id}`
    );
    writeArtifact(network, terra.config.chainID);
  }

  // Deploy :: ANC-UST proxy contract
  if (!network.anc_generator_proxy_contract_address) {
    network.anc_generator_proxy_contract_address = await instantiateContract(
      terra,
      wallet,
      network.anc_generator_proxy_contract_code_id,
      {
        generator_contract_addr: network.astroport_generator_address,
        pair_addr: network.anc_token,
        lp_token_addr: network.anc_ust_astroport_lp_token_address,
        reward_contract_addr: network.anc_lp_staking_contract_address, // ANC-UST LP staking contract which gives ANC emissions
        reward_token_addr: network.anc_token,
      }
    );
    console.log(
      `ANC-UST proxy contract deployed successfully, address : ${network.anc_generator_proxy_contract_address}`
    );
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(`ANC-UST proxy contract already deployed on bombay-12`);
  }

  // ##################### DEPLOYMENT === MIR-UST proxy contract #####################

  // MIR-UST proxy contract ID
  if (!network.mir_generator_proxy_contract_code_id) {
    network.mir_generator_proxy_contract_code_id = await uploadContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "generator_proxy_to_mirror.wasm")
    );
    console.log(
      `generator_proxy_to_mir id = ${network.mir_generator_proxy_contract_code_id}`
    );
    writeArtifact(network, terra.config.chainID);
  }

  // Deploy :: MIR-UST proxy contract
  if (!network.mir_generator_proxy_contract_address) {
    network.mir_generator_proxy_contract_address = await instantiateContract(
      terra,
      wallet,
      network.mir_generator_proxy_contract_code_id,
      {
        generator_contract_addr: network.astroport_generator_address,
        pair_addr: network.mir_token,
        lp_token_addr: network.mir_ust_astroport_lp_token_address,
        reward_contract_addr: network.mir_lp_staking_contract_address, // MIR-UST LP staking contract which gives MIR emissions
        reward_token_addr: network.mir_token,
      }
    );
    console.log(
      `MIR-UST proxy contract deployed successfully, address : ${network.mir_generator_proxy_contract_address}`
    );
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(`MIR-UST proxy contract already deployed on bombay-12`);
  }

  // ##################### DEPLOYMENT === ORION-UST proxy contract #####################

  // ORION-UST proxy contract ID
  if (!network.orion_generator_proxy_contract_code_id) {
    network.orion_generator_proxy_contract_code_id = await uploadContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "generator_proxy_to_orion.wasm")
    );
    console.log(
      `generator_proxy_to_orion id = ${network.orion_generator_proxy_contract_code_id}`
    );
    writeArtifact(network, terra.config.chainID);
  }

  // Deploy :: ORION-UST proxy contract
  if (!network.orion_generator_proxy_contract_address) {
    network.orion_generator_proxy_contract_address = await instantiateContract(
      terra,
      wallet,
      network.orion_generator_proxy_contract_code_id,
      {
        generator_contract_addr: network.astroport_generator_address,
        pair_addr: network.orion_token,
        lp_token_addr: network.orion_ust_astroport_lp_token_address,
        reward_contract_addr: network.orion_lp_staking_contract_address, // ORION-UST LP staking contract which gives ORION emissions
        reward_token_addr: network.orion_token,
      }
    );
    console.log(
      `ORION-UST proxy contract deployed successfully, address : ${network.orion_generator_proxy_contract_address}`
    );
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(`ORION-UST proxy contract already deployed on bombay-12`);
  }

  // ##################### DEPLOYMENT === STT-UST proxy contract #####################

  // STT-UST proxy contract ID
  if (!network.stt_generator_proxy_contract_code_id) {
    network.stt_generator_proxy_contract_code_id = await uploadContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "generator_proxy_to_stt.wasm")
    );
    console.log(
      `generator_proxy_to_stt_ id = ${network.stt_generator_proxy_contract_code_id}`
    );
    writeArtifact(network, terra.config.chainID);
  }

  // Deploy :: STT-UST proxy contract
  if (!network.stt_generator_proxy_contract_address) {
    network.stt_generator_proxy_contract_address = await instantiateContract(
      terra,
      wallet,
      network.stt_generator_proxy_contract_code_id,
      {
        generator_contract_addr: network.astroport_generator_address,
        pair_addr: network.stt_token,
        lp_token_addr: network.stt_ust_astroport_lp_token_address,
        reward_contract_addr: network.stt_lp_staking_contract_address, // STT-UST LP staking contract which gives STT emissions
        reward_token_addr: network.stt_token,
      }
    );
    console.log(
      `STT-UST proxy contract deployed successfully, address : ${network.stt_generator_proxy_contract_address}`
    );
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(`STT-UST proxy contract already deployed on bombay-12`);
  }

  // ##################### DEPLOYMENT === VKR-UST proxy contract #####################

  // VKR-UST proxy contract ID
  if (!network.vkr_generator_proxy_contract_code_id) {
    network.vkr_generator_proxy_contract_code_id = await uploadContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "generator_proxy_to_vkr.wasm")
    );
    console.log(
      `generator_proxy_to_vkr id = ${network.vkr_generator_proxy_contract_code_id}`
    );
    writeArtifact(network, terra.config.chainID);
  }

  // Deploy :: VKR-UST proxy contract
  if (!network.vkr_generator_proxy_contract_address) {
    network.vkr_generator_proxy_contract_address = await instantiateContract(
      terra,
      wallet,
      network.vkr_generator_proxy_contract_code_id,
      {
        generator_contract_addr: network.astroport_generator_address,
        pair_addr: network.vkr_token,
        lp_token_addr: network.vkr_ust_astroport_lp_token_address,
        reward_contract_addr: network.vkr_lp_staking_contract_address, // VKR-UST LP staking contract which gives VKR emissions
        reward_token_addr: network.vkr_token,
      }
    );
    console.log(
      `VKR-UST proxy contract deployed successfully, address : ${network.vkr_generator_proxy_contract_address}`
    );
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(`VKR-UST proxy contract already deployed on bombay-12`);
  }

  // ##################### DEPLOYMENT === MINE-UST proxy contract #####################

  // MINE-UST proxy contract ID
  if (!network.mine_generator_proxy_contract_code_id) {
    network.mine_generator_proxy_contract_code_id = await uploadContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "generator_proxy_to_mine.wasm")
    );
    console.log(
      `generator_proxy_to_mine id = ${network.mine_generator_proxy_contract_code_id}`
    );
    writeArtifact(network, terra.config.chainID);
  }

  // Deploy :: MINE-UST proxy contract
  if (!network.mine_generator_proxy_contract_address) {
    network.mine_generator_proxy_contract_address = await instantiateContract(
      terra,
      wallet,
      network.mine_generator_proxy_contract_code_id,
      {
        generator_contract_addr: network.astroport_generator_address,
        pair_addr: network.mine_token,
        lp_token_addr: network.mine_ust_astroport_lp_token_address,
        reward_contract_addr: network.mine_lp_staking_contract_address, // MINE-UST LP staking contract which gives MINE emissions
        reward_token_addr: network.mine_token,
      }
    );
    console.log(
      `MINE-UST proxy contract deployed successfully, address : ${network.mine_generator_proxy_contract_address}`
    );
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(`MINE-UST proxy contract already deployed on bombay-12`);
  }

  // ##################### DEPLOYMENT === PSI-UST proxy contract #####################

  // PSI-UST proxy contract ID
  if (!network.psi_generator_proxy_contract_code_id) {
    network.psi_generator_proxy_contract_code_id = await uploadContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "generator_proxy_to_psi.wasm")
    );
    console.log(
      `generator_proxy_to_psi id = ${network.psi_generator_proxy_contract_code_id}`
    );
    writeArtifact(network, terra.config.chainID);
  }

  // Deploy :: PSI-UST proxy contract
  if (!network.psi_generator_proxy_contract_address) {
    network.psi_generator_proxy_contract_address = await instantiateContract(
      terra,
      wallet,
      network.psi_generator_proxy_contract_code_id,
      {
        generator_contract_addr: network.astroport_generator_address,
        pair_addr: network.psi_token,
        lp_token_addr: network.psi_ust_astroport_lp_token_address,
        reward_contract_addr: network.psi_lp_staking_contract_address, // PSI-UST LP staking contract which gives PSI emissions
        reward_token_addr: network.psi_token,
      }
    );
    console.log(
      `PSI-UST proxy contract deployed successfully, address : ${network.psi_generator_proxy_contract_address}`
    );
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(`PSI-UST proxy contract already deployed on bombay-12`);
  }

  // ##################### DEPLOYMENT === APOLLO-UST proxy contract #####################

  // APOLLO-UST proxy contract ID
  // if (!network.apollo_generator_proxy_contract_code_id) {
  //   network.apollo_generator_proxy_contract_code_id = await uploadContract(
  //     terra,
  //     wallet,
  //     join(ARTIFACTS_PATH, "generator_proxy_to_apollo.wasm")
  //   );
  //   console.log(
  //     `generator_proxy_to_apollo id = ${network.apollo_generator_proxy_contract_code_id}`
  //   );
  //   writeArtifact(network, terra.config.chainID);
  // }

  // Deploy :: APOLLO-UST proxy contract
  // if (!network.apollo_generator_proxy_contract_address) {
  //   network.apollo_generator_proxy_contract_address = await instantiateContract(
  //     terra,
  //     wallet,
  //     network.apollo_generator_proxy_contract_code_id,
  //     {
  //       generator_contract_addr: network.astroport_generator_address,
  //       pair_addr: network.apollo_token,
  //       lp_token_addr: network.apollo_ust_astroport_lp_token_address,
  //       reward_contract_addr: network.apollo_lp_staking_contract_address, // APOLLO-UST LP staking contract which gives APOLLO emissions
  //       reward_token_addr: network.apollo_token,
  //       strategy_id: network.apollo_staking_strategy_id,
  //     }
  //   );
  //   console.log(
  //     `APOLLO-UST proxy contract deployed successfully, address : ${network.apollo_generator_proxy_contract_address}`
  //   );
  //   writeArtifact(network, terra.config.chainID);
  // } else {
  //   console.log(`APOLLO-UST proxy contract already deployed on bombay-12`);
  // }
}

function delay(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

main().catch(console.log);
