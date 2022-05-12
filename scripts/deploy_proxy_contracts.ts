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

  // ##################### DEPLOYMENT === ANC-UST PROXY CONTRACT #####################
  // ##################### DEPLOYMENT === ANC-UST PROXY CONTRACT #####################
  // ##################### DEPLOYMENT === ANC-UST PROXY CONTRACT #####################

  // ANC-UST PROXY CONTRACT ID
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

  // Deploy :: ANC-UST PROXY CONTRACT
  if (!network.anc_generator_proxy_contract_address) {
    network.anc_generator_proxy_contract_address = await instantiateContract(
      terra,
      wallet,
      network.anc_generator_proxy_contract_code_id,
      {
        generator_contract_addr: network.astroport_generator_address,
        pair_addr: network.anc_token,
        lp_token_addr: network.anc_ust_astroport_lp_token_address,
        reward_contract_addr: network.anc_lp_staking_contract_address, // ANC-UST LP Staking contract which gives ANC emissions
        reward_token_addr: network.anc_token,
      }
    );
    console.log(
      `ANC-UST PROXY CONTRACT deployed successfully, address : ${network.anc_generator_proxy_contract_address}`
    );
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(`ANC-UST PROXY CONTRACT already deployed on bombay-12`);
  }

  // ##################### DEPLOYMENT === MIR-UST PROXY CONTRACT #####################
  // ##################### DEPLOYMENT === MIR-UST PROXY CONTRACT #####################
  // ##################### DEPLOYMENT === MIR-UST PROXY CONTRACT #####################

  // MIR-UST PROXY CONTRACT ID
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

  // Deploy :: MIR-UST PROXY CONTRACT
  if (!network.mir_generator_proxy_contract_address) {
    network.mir_generator_proxy_contract_address = await instantiateContract(
      terra,
      wallet,
      network.mir_generator_proxy_contract_code_id,
      {
        generator_contract_addr: network.astroport_generator_address,
        pair_addr: network.mir_token,
        lp_token_addr: network.mir_ust_astroport_lp_token_address,
        reward_contract_addr: network.mir_lp_staking_contract_address, // MIR-UST LP Staking contract which gives MIR emissions
        reward_token_addr: network.mir_token,
      }
    );
    console.log(
      `MIR-UST PROXY CONTRACT deployed successfully, address : ${network.mir_generator_proxy_contract_address}`
    );
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(`MIR-UST PROXY CONTRACT already deployed on bombay-12`);
  }

  // // ##################### DEPLOYMENT === ORION-UST PROXY CONTRACT #####################
  // // ##################### DEPLOYMENT === ORION-UST PROXY CONTRACT #####################
  // // ##################### DEPLOYMENT === ORION-UST PROXY CONTRACT #####################

  // ORION-UST PROXY CONTRACT ID
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

  // Deploy :: ORION-UST PROXY CONTRACT
  if (!network.orion_generator_proxy_contract_address) {
    network.orion_generator_proxy_contract_address = await instantiateContract(
      terra,
      wallet,
      network.orion_generator_proxy_contract_code_id,
      {
        generator_contract_addr: network.astroport_generator_address,
        pair_addr: network.orion_token,
        lp_token_addr: network.orion_ust_astroport_lp_token_address,
        reward_contract_addr: network.orion_lp_staking_contract_address, // ORION-UST LP Staking contract which gives ORION emissions
        reward_token_addr: network.orion_token,
      }
    );
    console.log(
      `ORION-UST PROXY CONTRACT deployed successfully, address : ${network.orion_generator_proxy_contract_address}`
    );
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(`ORION-UST PROXY CONTRACT already deployed on bombay-12`);
  }

  // // ##################### DEPLOYMENT === STT-UST PROXY CONTRACT #####################
  // // ##################### DEPLOYMENT === STT-UST PROXY CONTRACT #####################
  // // ##################### DEPLOYMENT === STT-UST PROXY CONTRACT #####################

  // STT-UST PROXY CONTRACT ID
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

  // Deploy :: STT-UST PROXY CONTRACT
  if (!network.stt_generator_proxy_contract_address) {
    network.stt_generator_proxy_contract_address = await instantiateContract(
      terra,
      wallet,
      network.stt_generator_proxy_contract_code_id,
      {
        generator_contract_addr: network.astroport_generator_address,
        pair_addr: network.stt_token,
        lp_token_addr: network.stt_ust_astroport_lp_token_address,
        reward_contract_addr: network.stt_lp_staking_contract_address, // STT-UST LP Staking contract which gives STT emissions
        reward_token_addr: network.stt_token,
      }
    );
    console.log(
      `STT-UST PROXY CONTRACT deployed successfully, address : ${network.stt_generator_proxy_contract_address}`
    );
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(`STT-UST PROXY CONTRACT already deployed on bombay-12`);
  }

  // // ##################### DEPLOYMENT === VKR-UST PROXY CONTRACT #####################
  // // ##################### DEPLOYMENT === VKR-UST PROXY CONTRACT #####################
  // // ##################### DEPLOYMENT === VKR-UST PROXY CONTRACT #####################

  // VKR-UST PROXY CONTRACT ID
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

  // Deploy :: VKR-UST PROXY CONTRACT
  if (!network.vkr_generator_proxy_contract_address) {
    network.vkr_generator_proxy_contract_address = await instantiateContract(
      terra,
      wallet,
      network.vkr_generator_proxy_contract_code_id,
      {
        generator_contract_addr: network.astroport_generator_address,
        pair_addr: network.vkr_token,
        lp_token_addr: network.vkr_ust_astroport_lp_token_address,
        reward_contract_addr: network.vkr_lp_staking_contract_address, // VKR-UST LP Staking contract which gives VKR emissions
        reward_token_addr: network.vkr_token,
      }
    );
    console.log(
      `VKR-UST PROXY CONTRACT deployed successfully, address : ${network.vkr_generator_proxy_contract_address}`
    );
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(`VKR-UST PROXY CONTRACT already deployed on bombay-12`);
  }

  // ##################### DEPLOYMENT === MINE-UST PROXY CONTRACT #####################
  // ##################### DEPLOYMENT === MINE-UST PROXY CONTRACT #####################
  // ##################### DEPLOYMENT === MINE-UST PROXY CONTRACT #####################

  // MINE-UST PROXY CONTRACT ID
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

  // Deploy :: MINE-UST PROXY CONTRACT
  if (!network.mine_generator_proxy_contract_address) {
    network.mine_generator_proxy_contract_address = await instantiateContract(
      terra,
      wallet,
      network.mine_generator_proxy_contract_code_id,
      {
        generator_contract_addr: network.astroport_generator_address,
        pair_addr: network.mine_token,
        lp_token_addr: network.mine_ust_astroport_lp_token_address,
        reward_contract_addr: network.mine_lp_staking_contract_address, // MINE-UST LP Staking contract which gives MINE emissions
        reward_token_addr: network.mine_token,
      }
    );
    console.log(
      `MINE-UST PROXY CONTRACT deployed successfully, address : ${network.mine_generator_proxy_contract_address}`
    );
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(`MINE-UST PROXY CONTRACT already deployed on bombay-12`);
  }

  // ##################### DEPLOYMENT === PSI-UST PROXY CONTRACT #####################
  // ##################### DEPLOYMENT === PSI-UST PROXY CONTRACT #####################
  // ##################### DEPLOYMENT === PSI-UST PROXY CONTRACT #####################

  // PSI-UST PROXY CONTRACT ID
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

  // Deploy :: PSI-UST PROXY CONTRACT
  if (!network.psi_generator_proxy_contract_address) {
    network.psi_generator_proxy_contract_address = await instantiateContract(
      terra,
      wallet,
      network.psi_generator_proxy_contract_code_id,
      {
        generator_contract_addr: network.astroport_generator_address,
        pair_addr: network.psi_token,
        lp_token_addr: network.psi_ust_astroport_lp_token_address,
        reward_contract_addr: network.psi_lp_staking_contract_address, // PSI-UST LP Staking contract which gives PSI emissions
        reward_token_addr: network.psi_token,
      }
    );
    console.log(
      `PSI-UST PROXY CONTRACT deployed successfully, address : ${network.psi_generator_proxy_contract_address}`
    );
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(`PSI-UST PROXY CONTRACT already deployed on bombay-12`);
  }

  // // ##################### DEPLOYMENT === APOLLO-UST PROXY CONTRACT #####################
  // // ##################### DEPLOYMENT === APOLLO-UST PROXY CONTRACT #####################
  // // ##################### DEPLOYMENT === APOLLO-UST PROXY CONTRACT #####################

  // // APOLLO-UST PROXY CONTRACT ID
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

  // // Deploy :: APOLLO-UST PROXY CONTRACT
  // if (!network.apollo_generator_proxy_contract_address) {
  //   network.apollo_generator_proxy_contract_address = await instantiateContract(
  //     terra,
  //     wallet,
  //     network.apollo_generator_proxy_contract_code_id,
  //     {
  //       generator_contract_addr: network.astroport_generator_address,
  //       pair_addr: network.apollo_token,
  //       lp_token_addr: network.apollo_ust_astroport_lp_token_address,
  //       reward_contract_addr: network.apollo_lp_staking_contract_address, // APOLLO-UST LP Staking contract which gives APOLLO emissions
  //       reward_token_addr: network.apollo_token,
  //       strategy_id: network.apollo_staking_strategy_id,
  //     }
  //   );
  //   console.log(
  //     `APOLLO-UST PROXY CONTRACT deployed successfully, address : ${network.apollo_generator_proxy_contract_address}`
  //   );
  //   writeArtifact(network, terra.config.chainID);
  // } else {
  //   console.log(`APOLLO-UST PROXY CONTRACT already deployed on bombay-12`);
  // }
}

function delay(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

main().catch(console.log);
