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
  const INIT_TIMETAMP = parseInt((Date.now() / 1000).toFixed(0)) + 180;
  const TILL_TIMETAMP = INIT_TIMETAMP + 86400 * 365;
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
        staking_token: network.anc_ust_astroport_lp_token_address,
        distribution_schedule: [
          [INIT_TIMETAMP, TILL_TIMETAMP, String(INCENTIVES)],
        ],
      }
    );
    console.log(
      `ANC-UST STAKING CONTRACT deployed successfully, address : ${network.anc_lp_staking_contract_address}`
    );
    writeArtifact(network, terra.config.chainID);
    await delay(300);
  } else {
    console.log(`ANC-UST STAKING CONTRACT already deployed on bombay-12`);
  }

  // Transfer :: Transfer ANC to ANC-UST STAKING CONTRACT for incentives
  if (!network.anc__sent_to_lp_staking_contract_) {
    let tx = await executeContract(terra, wallet, network.anc_token, {
      transfer: {
        recipient: network.anc_lp_staking_contract_address,
        amount: String(INCENTIVES),
      },
    });
    console.log(
      `ANC for incentives sent to ANC-UST LP staking contract :: ${tx.txhash}`
    );
    network.anc__sent_to_lp_staking_contract_ = true;
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(
      `ANC already sent for incentives to ANC-UST LP staking contract bombay-12`
    );
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
        premium_min_update_interval: 86400,
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

  // Transfer :: Register MIR as an incentive asset  with the MIR-UST STAKING CONTRACT
  if (!network.mir_incentive_asset_registered_testing_) {
    let tx = await executeContract(
      terra,
      wallet,
      network.mir_lp_staking_contract_address,
      {
        register_asset: {
          asset_token: network.mir_token,
          staking_token: network.mir_ust_astroport_lp_token_address,
        },
      }
    );
    console.log(
      `MIR registered as asset for incentives with the MIR-UST LP staking contract :: ${tx.txhash}`
    );
    network.mir_incentive_asset_registered_testing_ = true;
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(
      `MIR already registered as asset for incentives with the MIR-UST LP staking contract bombay-12`
    );
  }

  // Transfer :: Transfer MIR to ANC-UST STAKING CONTRACT for incentives
  if (!network.mir__sent_to_lp_staking_contract_) {
    let tx = await executeContract(terra, wallet, network.mir_token, {
      send: {
        contract: network.mir_lp_staking_contract_address,
        amount: String(INCENTIVES),
        msg: Buffer.from(
          JSON.stringify({
            deposit_reward: {
              rewards: [[network.mir_token, String(INCENTIVES)]],
            },
          })
        ).toString("base64"),
      },
    });
    console.log(
      `MIR for incentives sent to MIR-UST LP staking contract :: ${tx.txhash}`
    );
    network.mir__sent_to_lp_staking_contract_ = true;
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(
      `MIR already sent for incentives to MIR-UST LP staking contract bombay-12`
    );
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
  // if (!network.orion_lp_staking_contract_address) {
  //   network.orion_lp_staking_contract_address = await instantiateContract(
  //     terra,
  //     wallet,
  //     network.orion_lp_staking_contract_code_id,
  //     {
  //       reward_token: network.orion_token,
  //       reward_token_decimals: 6,
  //       staking_token: network.orion_ust_terraswap_lp_token_address,
  //       staking_token_decimals: 6,
  //     }
  //   );
  //   console.log(
  //     `ORION-UST STAKING CONTRACT deployed successfully, address : ${network.orion_lp_staking_contract_address}`
  //   );
  //   writeArtifact(network, terra.config.chainID);
  // } else {
  //   console.log(`ORION-UST STAKING CONTRACT already deployed on bombay-12`);
  // }

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
  // if (!network.stt_lp_staking_contract_address) {
  //   network.stt_lp_staking_contract_address = await instantiateContract(
  //     terra,
  //     wallet,
  //     network.stt_lp_staking_contract_code_id,
  //     {
  //       owner: wallet.key.accAddress,

  //       starterra_token: network.stt_token,
  //       staking_token: network.stt_ust_terraswap_lp_token_address,
  //       burn_address: wallet.key.accAddress,
  //       gateway_address: wallet.key.accAddress,
  //       distribution_schedule: wallet.key.accAddress,
  //       unbond_config: { minimum_time: "", percentage_loss: "0" },
  //       faction_name: "testing",
  //       fee_configuration: { operation: "", fee: "0" },
  //     }
  //   );
  //   console.log(
  //     `STT-UST STAKING CONTRACT deployed successfully, address : ${network.stt_lp_staking_contract_address}`
  //   );
  //   writeArtifact(network, terra.config.chainID);
  // } else {
  //   console.log(`STT-UST STAKING CONTRACT already deployed on bombay-12`);
  // }

  // ##################### VKR-UST STAKING CONTRACT #####################
  // ##################### VKR-UST STAKING CONTRACT #####################
  // ##################### VKR-UST STAKING CONTRACT #####################

  // VKR-UST STAKING CONTRACT ID
  if (!network.vkr_lp_staking_contract_code_id) {
    network.vkr_lp_staking_contract_code_id = await uploadContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "valkyrie_lp_staking.wasm")
    );
    console.log(`vkr_staking id = ${network.vkr_lp_staking_contract_code_id}`);
    writeArtifact(network, terra.config.chainID);
  }

  // Deploy :: VKR-UST STAKING CONTRACT
  // if (!network.vkr_lp_staking_contract_address) {
  //   network.vkr_lp_staking_contract_address = await instantiateContract(
  //     terra,
  //     wallet,
  //     network.vkr_lp_staking_contract_code_id,
  //     {
  //       owner: wallet.key.accAddress,

  //       starterra_token: network.vkr_token,
  //       staking_token: network.vkr_ust_terraswap_lp_token_address,
  //       burn_address: wallet.key.accAddress,
  //       gateway_address: wallet.key.accAddress,
  //       distribution_schedule: wallet.key.accAddress,
  //       unbond_config: { minimum_time: "", percentage_loss: "0" },
  //       faction_name: "testing",
  //       fee_configuration: { operation: "", fee: "0" },
  //     }
  //   );
  //   console.log(
  //     `VKR-UST STAKING CONTRACT deployed successfully, address : ${network.vkr_lp_staking_contract_address}`
  //   );
  //   writeArtifact(network, terra.config.chainID);
  // } else {
  //   console.log(`VKR-UST STAKING CONTRACT already deployed on bombay-12`);
  // }

  // ##################### MINE-UST STAKING CONTRACT #####################
  // ##################### MINE-UST STAKING CONTRACT #####################
  // ##################### MINE-UST STAKING CONTRACT #####################

  // MINE-UST STAKING CONTRACT ID
  if (!network.mine_lp_staking_contract_code_id) {
    network.mine_lp_staking_contract_code_id = await uploadContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "pylon_staking.wasm")
    );
    console.log(
      `mine_staking id = ${network.mine_lp_staking_contract_code_id}`
    );
    writeArtifact(network, terra.config.chainID);
  }

  // Deploy :: MINE-UST STAKING CONTRACT
  // if (!network.mine_lp_staking_contract_address) {
  //   network.mine_lp_staking_contract_address = await instantiateContract(
  //     terra,
  //     wallet,
  //     network.mine_lp_staking_contract_code_id,
  //     {
  //       owner: wallet.key.accAddress,

  //       starterra_token: network.mine_token,
  //       staking_token: network.mine_ust_terraswap_lp_token_address,
  //       burn_address: wallet.key.accAddress,
  //       gateway_address: wallet.key.accAddress,
  //       distribution_schedule: wallet.key.accAddress,
  //       unbond_config: { minimum_time: "", percentage_loss: "0" },
  //       faction_name: "testing",
  //       fee_configuration: { operation: "", fee: "0" },
  //     }
  //   );
  //   console.log(
  //     `MINE-UST STAKING CONTRACT deployed successfully, address : ${network.mine_lp_staking_contract_address}`
  //   );
  //   writeArtifact(network, terra.config.chainID);
  // } else {
  //   console.log(`MINE-UST STAKING CONTRACT already deployed on bombay-12`);
  // }

  // ##################### PSI-UST STAKING CONTRACT #####################
  // ##################### PSI-UST STAKING CONTRACT #####################
  // ##################### PSI-UST STAKING CONTRACT #####################

  // PSI-UST STAKING CONTRACT ID
  if (!network.mine_lp_staking_contract_code_id) {
    network.mine_lp_staking_contract_code_id = await uploadContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "pylon_staking.wasm")
    );
    console.log(
      `mine_staking id = ${network.mine_lp_staking_contract_code_id}`
    );
    writeArtifact(network, terra.config.chainID);
  }

  // Deploy :: PSI-UST STAKING CONTRACT
  // if (!network.psi_lp_staking_contract_address) {
  //   network.psi_lp_staking_contract_address = await instantiateContract(
  //     terra,
  //     wallet,
  //     network.psi_lp_staking_contract_code_id,
  //     {
  //       owner: wallet.key.accAddress,

  //       starterra_token: network.psi_token,
  //       staking_token: network.psi_ust_terraswap_lp_token_address,
  //       burn_address: wallet.key.accAddress,
  //       gateway_address: wallet.key.accAddress,
  //       distribution_schedule: wallet.key.accAddress,
  //       unbond_config: { minimum_time: "", percentage_loss: "0" },
  //       faction_name: "testing",
  //       fee_configuration: { operation: "", fee: "0" },
  //     }
  //   );
  //   console.log(
  //     `PSI-UST STAKING CONTRACT deployed successfully, address : ${network.psi_lp_staking_contract_address}`
  //   );
  //   writeArtifact(network, terra.config.chainID);
  // } else {
  //   console.log(`PSI-UST STAKING CONTRACT already deployed on bombay-12`);
  // }

  // ##################### APOLLO-UST STAKING CONTRACT #####################
  // ##################### APOLLO-UST STAKING CONTRACT #####################
  // ##################### APOLLO-UST STAKING CONTRACT #####################

  // APOLLO-FACTORY  CONTRACT ID
  if (!network.apollo_factory_contract_code_id) {
    network.apollo_factory_contract_code_id = await uploadContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "apollo_factory.wasm")
    );
    console.log(
      `apollo_factory id = ${network.apollo_factory_contract_code_id}`
    );
    writeArtifact(network, terra.config.chainID);
  }

  // APOLLO-UST STAKING CONTRACT ID
  if (!network.apollo_lp_staking_contract_code_id) {
    network.apollo_lp_staking_contract_code_id = await uploadContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "apollo_staking.wasm")
    );
    console.log(
      `apollo_staking id = ${network.apollo_lp_staking_contract_code_id}`
    );
    writeArtifact(network, terra.config.chainID);
  }

  // Deploy :: APOLLO-UST STAKING CONTRACT
  // if (!network.psi_lp_staking_contract_address) {
  //   network.psi_lp_staking_contract_address = await instantiateContract(
  //     terra,
  //     wallet,
  //     network.psi_lp_staking_contract_code_id,
  //     {
  //       owner: wallet.key.accAddress,

  //       starterra_token: network.psi_token,
  //       staking_token: network.psi_ust_terraswap_lp_token_address,
  //       burn_address: wallet.key.accAddress,
  //       gateway_address: wallet.key.accAddress,
  //       distribution_schedule: wallet.key.accAddress,
  //       unbond_config: { minimum_time: "", percentage_loss: "0" },
  //       faction_name: "testing",
  //       fee_configuration: { operation: "", fee: "0" },
  //     }
  //   );
  //   console.log(
  //     `APOLLO-UST STAKING CONTRACT deployed successfully, address : ${network.psi_lp_staking_contract_address}`
  //   );
  //   writeArtifact(network, terra.config.chainID);
  // } else {
  //   console.log(`PSI-UST STAKING CONTRACT already deployed on bombay-12`);
  // }
}

function delay(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

main().catch(console.log);
