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

  const INIT_BLOCK_HEIGHT = 6906883;
  const TILL_BLOCK_HEIGHT = INIT_BLOCK_HEIGHT + 365 * (86400 / 3);

  const INCENTIVES = 5000000000000;

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
    console.log(
      `anc_LP_staking id = ${network.anc_lp_staking_contract_code_id}`
    );
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
          [INIT_BLOCK_HEIGHT, TILL_BLOCK_HEIGHT, String(INCENTIVES)],
        ],
      }
    );
    console.log(
      `ANC-UST STAKING CONTRACT deployed successfully, address : ${network.anc_lp_staking_contract_address} `
    );
    writeArtifact(network, terra.config.chainID);
    await delay(300);
  } else {
    console.log(`ANC-UST STAKING CONTRACT already deployed on bombay-12`);
  }

  // Transfer :: Transfer ANC to ANC-UST STAKING CONTRACT for incentives
  if (!network.anc_sent_to_lp_staking_contract_) {
    let tx = await executeContract(terra, wallet, network.anc_token, {
      transfer: {
        recipient: network.anc_lp_staking_contract_address,
        amount: String(INCENTIVES),
      },
    });
    console.log(
      `ANC for incentives sent to ANC-UST LP staking contract :: ${tx.txhash}\n`
    );
    network.anc_sent_to_lp_staking_contract_ = true;
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
    console.log(
      `mir_LP_staking id = ${network.mir_lp_staking_contract_code_id}`
    );
    writeArtifact(network, terra.config.chainID);
  }

  // // Deploy :: MIR-UST STAKING CONTRACT
  if (!network.mir_lp_staking_contract_address) {
    console.log("Deploying mirror staking contract...");
    network.mir_lp_staking_contract_address = await instantiateContract(
      terra,
      wallet,
      network.mir_lp_staking_contract_code_id,
      {
        owner: wallet.key.accAddress,
        mirror_token: network.mir_token,
        mint_contract: wallet.key.accAddress, // mock value
        oracle_contract: wallet.key.accAddress, // mock value
        terraswap_factory: network.terraswap_factory_address, // mock value
        base_denom: "uusd",
        premium_min_update_interval: 0,
        short_reward_contract: wallet.key.accAddress, // mock value
      }
    );
    writeArtifact(network, terra.config.chainID);
    console.log(
      `MIR-UST STAKING CONTRACT deployed successfully, address : ${network.mir_lp_staking_contract_address}`
    );
  }

  // Register MIR-UST astroport pool with the LP staking contract
  if (
    network.mir_lp_staking_contract_address &&
    !network.mir_ust_pair_registered_with_staking
  ) {
    let pool_info = await queryContract(terra, network.mir_ust_astroport_pool, {
      pair: {},
    });

    console.log("Registering MIR-UST pair in mirror LP staking contract...");
    await executeContract(
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
    console.log("Registered successfully");
    network.mir_ust_pair_registered_with_staking = true;
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(
      `MIR-UST pair already registered in mirror LP staking contract.`
    );
  }

  // Send MIR to MIR LP staking contract for incentives
  if (
    network.mir_lp_staking_contract_address &&
    network.mir_ust_pair_registered_with_staking &&
    !network.mir_sent_to_lp_staking_contract
  ) {
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
      `MIR for incentives sent to MIR-UST LP staking contract :: ${tx.txhash} \n`
    );
    network.mir_sent_to_lp_staking_contract = true;
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
      `orion_LP_staking id = ${network.orion_lp_staking_contract_code_id}`
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
        staking_token: network.orion_ust_astroport_lp_token_address,
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

  // Send ORION to ORION LP staking contract for incentives
  if (
    network.orion_lp_staking_contract_address &&
    !network.orion_sent_to_lp_staking_contract
  ) {
    let tx = await executeContract(terra, wallet, network.orion_token, {
      transfer: {
        recipient: network.orion_lp_staking_contract_address,
        amount: String(INCENTIVES),
      },
    });
    console.log(
      `ORION for incentives sent to ORION-UST LP staking contract :: ${tx.txhash} \n`
    );
    network.orion_sent_to_lp_staking_contract = true;
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(
      `ORION already sent for incentives to ORION-UST LP staking contract bombay-12`
    );
  }

  // Set ORION rewards in ORION LP staking contract
  if (
    network.orion_lp_staking_contract_address &&
    network.orion_sent_to_lp_staking_contract &&
    !network.orion_lp_staking_contract_incentives_set
  ) {
    let tx = await executeContract(
      terra,
      wallet,
      network.orion_lp_staking_contract_address,
      {
        notify_rewards: {
          period_start: INIT_TIMETAMP,
          period_finish: TILL_TIMETAMP,
          amount: String(INCENTIVES),
        },
      }
    );
    console.log(
      `ORION incentives set in ORION-UST LP staking contract :: ${tx.txhash} \n`
    );
    network.orion_lp_staking_contract_incentives_set = true;
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(
      `ORION incentives already set in ORION-UST LP staking contract `
    );
  }

  // ##################### STT-UST STAKING CONTRACT #####################
  // ##################### STT-UST STAKING CONTRACT #####################
  // ##################### STT-UST STAKING CONTRACT #####################

  // STT-UST STAKING GATEWAY CONTRACT ID
  if (!network.stt_lp_staking_gateway_contract_code_id) {
    network.stt_lp_staking_gateway_contract_code_id = await uploadContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "starterra_staking_gateway.wasm")
    );
    console.log(
      `stt_lp_staking_gateway_ id = ${network.stt_lp_staking_gateway_contract_code_id}`
    );
    writeArtifact(network, terra.config.chainID);
  }

  // STT-UST STAKING CONTRACT ID
  if (!network.stt_lp_staking_contract_code_id) {
    network.stt_lp_staking_contract_code_id = await uploadContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "starterra_staking.wasm")
    );
    console.log(
      `stt_LP_staking id = ${network.stt_lp_staking_contract_code_id}`
    );
    writeArtifact(network, terra.config.chainID);
  }

  // Deploy :: STT-UST STAKING GATEWAY CONTRACT
  if (!network.stt_lp_staking_gateway_contract_address) {
    network.stt_lp_staking_gateway_contract_address = await instantiateContract(
      terra,
      wallet,
      network.stt_lp_staking_gateway_contract_code_id,
      {
        owner: wallet.key.accAddress,
        staking_contracts: [],
      }
    );
    console.log(
      `STT-UST STAKING GATEWAY CONTRACT deployed successfully, address : ${network.stt_lp_staking_gateway_contract_address}`
    );
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(
      `STT-UST STAKING GATEWAY CONTRACT already deployed on bombay-12`
    );
  }

  // Deploy :: STT-UST STAKING CONTRACT
  if (
    !network.stt_lp_staking_contract_address &&
    network.stt_lp_staking_gateway_contract_address
  ) {
    network.stt_lp_staking_contract_address = await instantiateContract(
      terra,
      wallet,
      network.stt_lp_staking_contract_code_id,
      {
        owner: wallet.key.accAddress,
        starterra_token: network.stt_token,
        staking_token: network.stt_ust_astroport_lp_token_address,
        burn_address: wallet.key.accAddress,
        gateway_address: network.stt_lp_staking_gateway_contract_address,
        distribution_schedule: [
          {
            start_time: INIT_TIMETAMP,
            end_time: TILL_TIMETAMP,
            amount: String(INCENTIVES),
          },
        ],
        unbond_config: [{ minimum_time: 0, percentage_loss: 0 }],
        faction_name: "testing",
        fee_configuration: [],
      }
    );
    console.log(
      `STT-UST STAKING CONTRACT deployed successfully, address : ${network.stt_lp_staking_contract_address}`
    );
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(`STT-UST STAKING CONTRACT already deployed on bombay-12`);
  }

  // Send STT to STT LP staking contract for incentives
  if (
    network.stt_lp_staking_contract_address &&
    !network.stt_sent_to_lp_staking_contract
  ) {
    let tx = await executeContract(terra, wallet, network.stt_token, {
      transfer: {
        recipient: network.stt_lp_staking_contract_address,
        amount: String(INCENTIVES),
      },
    });
    console.log(
      `STT for incentives sent to STT-UST LP staking contract :: ${tx.txhash} \n`
    );
    network.stt_sent_to_lp_staking_contract = true;
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(
      `STT already sent for incentives to STT-UST LP staking contract bombay-12`
    );
  }

  // UpdateConfig :: STT STAKING GATEWAY CONTRACT
  if (
    network.stt_lp_staking_contract_address &&
    network.stt_lp_staking_gateway_contract_address &&
    !network.stt_lp_staking_gateway_config_updated
  ) {
    let tx = await executeContract(
      terra,
      wallet,
      network.stt_lp_staking_gateway_contract_address,
      {
        update_config: {
          owner: undefined,
          staking_contracts: [network.stt_lp_staking_contract_address],
        },
      },
      [],
      `STT Staking gateway :: update config for testing`
    );
    console.log(
      `STT-UST STAKING GATEWAY CONTRACT config updated successfully, tx : ${tx.txhash}`
    );
    network.stt_lp_staking_gateway_config_updated = true;
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(`STT-UST STAKING GATEWAY CONTRACT  config already updated`);
  }

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
    console.log(
      `vkr_LP_staking id = ${network.vkr_lp_staking_contract_code_id}`
    );
    writeArtifact(network, terra.config.chainID);
  }

  // // Deploy :: VKR-UST STAKING CONTRACT
  if (!network.vkr_lp_staking_contract_address) {
    network.vkr_lp_staking_contract_address = await instantiateContract(
      terra,
      wallet,
      network.vkr_lp_staking_contract_code_id,
      {
        token: network.vkr_token,
        pair: network.vkr_ust_astroport_pool,
        lp_token: network.vkr_ust_astroport_lp_token_address,
        distribution_schedule: [
          [INIT_BLOCK_HEIGHT, TILL_BLOCK_HEIGHT, String(INCENTIVES)],
        ],
      }
    );
    console.log(
      `VKR-UST STAKING CONTRACT deployed successfully, address : ${network.vkr_lp_staking_contract_address}`
    );
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(`VKR-UST STAKING CONTRACT already deployed on bombay-12`);
  }

  // Send VKR to VKR LP staking contract for incentives
  if (
    network.vkr_lp_staking_contract_address &&
    !network.vkr_sent_to_lp_staking_contract
  ) {
    let tx = await executeContract(terra, wallet, network.vkr_token, {
      transfer: {
        recipient: network.vkr_lp_staking_contract_address,
        amount: String(INCENTIVES),
      },
    });
    console.log(
      `VKR for incentives sent to VKR-UST LP staking contract :: ${tx.txhash} \n`
    );
    network.vkr_sent_to_lp_staking_contract = true;
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(
      `VKR already sent for incentives to VKR-UST LP staking contract bombay-12`
    );
  }

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
      `mine_LP_staking id = ${network.mine_lp_staking_contract_code_id}`
    );
    writeArtifact(network, terra.config.chainID);
  }

  // Deploy :: MINE-UST STAKING CONTRACT
  if (!network.mine_lp_staking_contract_address) {
    network.mine_lp_staking_contract_address = await instantiateContract(
      terra,
      wallet,
      network.mine_lp_staking_contract_code_id,
      {
        pylon_token: network.mine_token,
        staking_token: network.mine_ust_astroport_lp_token_address,
        distribution_schedule: [
          [INIT_BLOCK_HEIGHT, TILL_BLOCK_HEIGHT, String(INCENTIVES)],
        ],
      }
    );
    console.log(
      `MINE-UST STAKING CONTRACT deployed successfully, address : ${network.mine_lp_staking_contract_address}`
    );
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(`MINE-UST STAKING CONTRACT already deployed on bombay-12`);
  }

  // Send MINE to MINE LP staking contract for incentives
  if (
    network.mine_lp_staking_contract_address &&
    !network.mine_sent_to_lp_staking_contract
  ) {
    let tx = await executeContract(terra, wallet, network.mine_token, {
      transfer: {
        recipient: network.mine_lp_staking_contract_address,
        amount: String(INCENTIVES),
      },
    });
    console.log(
      `MINE for incentives sent to MINE-UST LP staking contract :: ${tx.txhash} \n`
    );
    network.mine_sent_to_lp_staking_contract = true;
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(
      `MINE already sent for incentives to MINE-UST LP staking contract bombay-12`
    );
  }

  // ##################### PSI-UST STAKING CONTRACT #####################
  // ##################### PSI-UST STAKING CONTRACT #####################
  // ##################### PSI-UST STAKING CONTRACT #####################

  // PSI-UST STAKING CONTRACT ID
  if (!network.psi_lp_staking_contract_code_id) {
    network.psi_lp_staking_contract_code_id = await uploadContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "psi_staking.wasm")
    );
    console.log(
      `psi_LP_staking id = ${network.psi_lp_staking_contract_code_id}`
    );
    writeArtifact(network, terra.config.chainID);
  }

  // Deploy :: PSI-UST STAKING CONTRACT
  if (!network.psi_lp_staking_contract_address) {
    network.psi_lp_staking_contract_address = await instantiateContract(
      terra,
      wallet,
      network.psi_lp_staking_contract_code_id,
      {
        owner: wallet.key.accAddress,
        psi_token: network.psi_token,
        staking_token: network.psi_ust_astroport_lp_token_address,
        terraswap_factory: network.terraswap_factory_address,
        distribution_schedule: [
          {
            start_time: INIT_TIMETAMP,
            end_time: TILL_TIMETAMP,
            amount: String(INCENTIVES),
          },
        ],
      }
    );
    console.log(
      `PSI-UST STAKING CONTRACT deployed successfully, address : ${network.psi_lp_staking_contract_address}`
    );
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(`PSI-UST STAKING CONTRACT already deployed on bombay-12`);
  }

  // Send PSI to PSI LP staking contract for incentives
  if (
    network.psi_lp_staking_contract_address &&
    !network.psi_sent_to_lp_staking_contract
  ) {
    let tx = await executeContract(terra, wallet, network.psi_token, {
      transfer: {
        recipient: network.psi_lp_staking_contract_address,
        amount: String(INCENTIVES),
      },
    });
    console.log(
      `PSI for incentives sent to PSI-UST LP staking contract :: ${tx.txhash} \n`
    );
    network.psi_sent_to_lp_staking_contract = true;
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(
      `PSI already sent for incentives to PSI-UST LP staking contract bombay-12`
    );
  }

  // ##################### APOLLO-UST STAKING CONTRACT #####################
  // ##################### APOLLO-UST STAKING CONTRACT #####################
  // ##################### APOLLO-UST STAKING CONTRACT #####################

  // APOLLO-FACTORY  CONTRACT ID
  // if (!network.apollo_factory_contract_code_id) {
  //   network.apollo_factory_contract_code_id = await uploadContract(
  //     terra,
  //     wallet,
  //     join(ARTIFACTS_PATH, "apollo_factory.wasm")
  //   );
  //   console.log(
  //     `apollo_factory id = ${network.apollo_factory_contract_code_id}`
  //   );
  //   writeArtifact(network, terra.config.chainID);
  // }

  // // APOLLO-UST STAKING CONTRACT ID
  // if (!network.apollo_lp_staking_contract_code_id) {
  //   network.apollo_lp_staking_contract_code_id = await uploadContract(
  //     terra,
  //     wallet,
  //     join(ARTIFACTS_PATH, "apollo_staking.wasm")
  //   );
  //   console.log(
  //     `apollo_LP_staking id = ${network.apollo_lp_staking_contract_code_id}`
  //   );
  //   writeArtifact(network, terra.config.chainID);
  // }

  // Deploy :: APOLLO FACTORY CONTRACT
  // if (
  //   network.apollo_factory_contract_code_id &&
  //   !network.apollo_factory_address
  // ) {
  //   network.apollo_factory_address = await instantiateContract(
  //     terra,
  //     wallet,
  //     network.apollo_factory_contract_code_id,
  //     {
  //       warchest: wallet.key.accAddress,
  //       max_rewards: "3000000000000",
  //       distribution_schedule: [[]],
  //       oracle: wallet.key.accAddress,
  //       apollo_token: network.apollo_token,
  //       apollo_reward_percentage: "",
  //     }
  //   );
  //   console.log(
  //     `APOLLO FACTORY CONTRACT deployed successfully, address : ${network.apollo_factory_address}`
  //   );
  //   writeArtifact(network, terra.config.chainID);
  // } else {
  //   console.log(`APOLLO FACTORY CONTRACT already deployed on bombay-12`);
  // }

  // Deploy :: APOLLO-UST STAKING CONTRACT
  // if (!network.apollo_lp_staking_contract_address) {
  //   network.apollo_lp_staking_contract_address = await instantiateContract(
  //     terra,
  //     wallet,
  //     network.apollo_lp_staking_contract_code_id,
  //     {
  //       apollo_factory: network.apollo_factory_address,
  //       apollo_collector: wallet.key.accAddress,
  //       base_token: network.apollo_ust_astroport_lp_token_address,
  //       performance_fee: "",
  //       base_denom: "uusd",
  //       asset_token: network.apollo_token,
  //       asset_token_pair: network.apollo_ust_astroport_pool,
  //       max_spread: "",
  //       swap_commission: wallet.key.accAddress,
  //       oracle_contract: wallet.key.accAddress,
  //       apollo_strategy_id: { minimum_time: "", percentage_loss: "0" },
  //       reward_token: network.apollo_token,
  //     }
  //   );
  //   console.log(
  //     `APOLLO-UST STAKING CONTRACT deployed successfully, address : ${network.apollo_lp_staking_contract_address}`
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
