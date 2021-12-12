import "dotenv/config";
import { Coin, LCDClient, LocalTerra, Wallet } from "@terra-money/terra.js";
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

const ANC_ALLOC_POINT = 10;
const MIR_ALLOC_POINT = 10;
const ORION_ALLOC_POINT = 10;
const STT_ALLOC_POINT = 10;
const VKR_ALLOC_POINT = 10;
const MINE_ALLOC_POINT = 10;
const PSI_ALLOC_POINT = 10;

async function query_generator_config(
  terra: LocalTerra | LCDClient,
  generator_address: string
) {
  let config = await queryContract(terra, generator_address, {
    config: {},
  });
  return config;
}

async function add_proxy_as_allowed_generator(
  terra: LocalTerra | LCDClient,
  wallet: Wallet,
  generator_address: string,
  new_allowed_proxies: Array<String>,
  pair_name: string
) {
  let tx = await executeContract(
    terra,
    wallet,
    generator_address,
    {
      set_allowed_reward_proxies: {
        proxies: new_allowed_proxies,
      },
    },
    [],
    `Generator :: Setting the ${pair_name} proxy contract as allowed in generator... `
  );
  return tx;
}

async function register_lp_tokens_in_generator(
  terra: LocalTerra | LCDClient,
  wallet: Wallet,
  generator_address: string,
  lp_token_address: string,
  proxy_contract_address: string,
  alloc_point: string,
  pair_name: string
) {
  let tx = await executeContract(
    terra,
    wallet,
    generator_address,
    {
      add: {
        lp_token: lp_token_address,
        alloc_point: alloc_point,
        reward_proxy: proxy_contract_address,
      },
    },
    [],
    `Generator :: Adding ${pair_name} LP token for handling rewards`
  );
  return tx;
}

async function main() {
  const { terra, wallet } = newClient();

  console.log(
    `chainID: ${terra.config.chainID} wallet: ${wallet.key.accAddress}`
  );

  const network = readArtifact(terra.config.chainID);
  console.log("network:", network);

  // ##################### ASTROPORT GENERATOR :: SET & ADD ANC REWARD PROXY #####################

  // if (
  //   !network.anc_proxy_rewards_set_in_generator &&
  //   network.anc_generator_proxy_contract_address
  // ) {
  //   // Query config ==> add proxy to the list of allowed proxies ==> set proxies as allowed in generator
  //   if (!network.anc_generator_proxy_set_in_generator) {
  //     let config = await query_generator_config(
  //       terra,
  //       network.astroport_generator_address
  //     );
  //     let new_allowed_proxies: Array<String> = config.allowed_reward_proxies;
  //     new_allowed_proxies.push(
  //       network.anc_generator_proxy_contract_address as String
  //     );
  //     console.log(
  //       `Set the ANC proxy as allowed in generator... Allowed proxies with new one: ${new_allowed_proxies}`
  //     );
  //     let tx_set_proxy = await add_proxy_as_allowed_generator(
  //       terra,
  //       wallet,
  //       network.astroport_generator_address,
  //       new_allowed_proxies,
  //       "ANC-UST"
  //     );
  //     network.anc_generator_proxy_set_in_generator = true;
  //     console.log(
  //       `ANC proxy set successfully as allowed in generator... ${tx_set_proxy.txhash}`
  //     );
  //     writeArtifact(network, terra.config.chainID);
  //     await delay(900);
  //   }
  //   // Register ANC-UST LP token with the astroport generator
  //   let tx_set_reward = await register_lp_tokens_in_generator(
  //     terra,
  //     wallet,
  //     network.astroport_generator_address,
  //     network.anc_ust_astroport_lp_token_address,
  //     network.anc_generator_proxy_contract_address,
  //     String(ANC_ALLOC_POINT),
  //     "ANC-UST"
  //   );
  //   console.log(
  //     `Adding ANC-UST LP rewards with the proxy contract to the astroport generator = ${tx_set_reward.txhash}`
  //   );

  //   network.anc_proxy_rewards_set_in_generator = true;
  //   writeArtifact(network, terra.config.chainID);
  // } else {
  //   console.log(
  //     `ANC-UST PROXY both added and set with Astroport generator on bombay-12\n`
  //   );
  // }

  // ##################### ASTROPORT GENERATOR :: SET & ADD MIR REWARD PROXY #####################

  if (
    !network.mir_proxy_rewards_set_in_generator &&
    network.mir_generator_proxy_contract_address
  ) {
    // Query config ==> add proxy to the list of allowed proxies ==> set proxies as allowed in generator
    if (!network.mir_generator_proxy_set_in_generator) {
      let config = await query_generator_config(
        terra,
        network.astroport_generator_address
      );
      let new_allowed_proxies: Array<String> = config.allowed_reward_proxies;
      new_allowed_proxies.push(
        network.mir_generator_proxy_contract_address as String
      );
      console.log(
        `Set the MIR proxy as allowed in generator... Allowed proxies with new one: ${new_allowed_proxies}`
      );
      let tx_set_proxy = await add_proxy_as_allowed_generator(
        terra,
        wallet,
        network.astroport_generator_address,
        new_allowed_proxies,
        "MIR-UST"
      );
      network.mir_generator_proxy_set_in_generator = true;
      console.log(
        `MIR proxy set successfully as allowed in generator... ${tx_set_proxy.txhash}`
      );
      writeArtifact(network, terra.config.chainID);
      await delay(900);
    }
    // Register MIR-UST LP token with the astroport generator
    let tx_set_reward = await register_lp_tokens_in_generator(
      terra,
      wallet,
      network.astroport_generator_address,
      network.mir_ust_astroport_lp_token_address,
      network.mir_generator_proxy_contract_address,
      String(MIR_ALLOC_POINT),
      "MIR-UST"
    );
    console.log(
      `Adding MIR-UST LP rewards with the proxy contract to the astroport generator = ${tx_set_reward.txhash}`
    );

    network.mir_proxy_rewards_set_in_generator = true;
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(
      `MIR-UST PROXY both added and set with Astroport generator on bombay-12`
    );
  }

  // ##################### ASTROPORT GENERATOR :: SET & ADD ORION REWARD PROXY #####################

  if (
    !network.orion_proxy_rewards_set_in_generator &&
    network.orion_generator_proxy_contract_address
  ) {
    // Query config ==> add proxy to the list of allowed proxies ==> set proxies as allowed in generator
    if (!network.orion_generator_proxy_set_in_generator) {
      let config = await query_generator_config(
        terra,
        network.astroport_generator_address
      );
      let new_allowed_proxies: Array<String> = config.allowed_reward_proxies;
      new_allowed_proxies.push(
        network.orion_generator_proxy_contract_address as String
      );
      console.log(
        `Set the ORION proxy as allowed in generator... Allowed proxies with new one: ${new_allowed_proxies}`
      );
      let tx_set_proxy = await add_proxy_as_allowed_generator(
        terra,
        wallet,
        network.astroport_generator_address,
        new_allowed_proxies,
        "ORION-UST"
      );
      network.orion_generator_proxy_set_in_generator = true;
      console.log(
        `ORION proxy set successfully as allowed in generator... ${tx_set_proxy.txhash}`
      );
      writeArtifact(network, terra.config.chainID);
      await delay(900);
    }
    // Register ORION-UST LP token with the astroport generator
    let tx_set_reward = await register_lp_tokens_in_generator(
      terra,
      wallet,
      network.astroport_generator_address,
      network.orion_ust_astroport_lp_token_address,
      network.orion_generator_proxy_contract_address,
      String(ORION_ALLOC_POINT),
      "ORION-UST"
    );
    console.log(
      `Adding ORION-UST LP rewards with the proxy contract to the astroport generator = ${tx_set_reward.txhash}`
    );

    network.orion_proxy_rewards_set_in_generator = true;
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(
      `ORION-UST PROXY both added and set with Astroport generator on bombay-12`
    );
  }

  // ##################### ASTROPORT GENERATOR :: SET & ADD STT REWARD PROXY #####################

  if (
    !network.stt_proxy_rewards_set_in_generator &&
    network.stt_generator_proxy_contract_address
  ) {
    // Query config ==> add proxy to the list of allowed proxies ==> set proxies as allowed in generator
    if (!network.stt_generator_proxy_set_in_generator) {
      let config = await query_generator_config(
        terra,
        network.astroport_generator_address
      );
      let new_allowed_proxies: Array<String> = config.allowed_reward_proxies;
      new_allowed_proxies.push(
        network.stt_generator_proxy_contract_address as String
      );

      console.log(
        `Set the STT proxy as allowed in generator... Allowed proxies with new one: ${new_allowed_proxies}`
      );
      let tx_set_proxy = await add_proxy_as_allowed_generator(
        terra,
        wallet,
        network.astroport_generator_address,
        new_allowed_proxies,
        "STT-UST"
      );
      network.stt_generator_proxy_set_in_generator = true;
      console.log(
        `STT proxy set successfully as allowed in generator... ${tx_set_proxy.txhash}`
      );
      writeArtifact(network, terra.config.chainID);
      await delay(900);
    }
    // Register STT-UST LP token with the astroport generator
    let tx_set_reward = await register_lp_tokens_in_generator(
      terra,
      wallet,
      network.astroport_generator_address,
      network.stt_ust_astroport_lp_token_address,
      network.stt_generator_proxy_contract_address,
      String(STT_ALLOC_POINT),
      "STT-UST"
    );
    console.log(
      `Adding STT-UST LP rewards with the proxy contract to the astroport generator = ${tx_set_reward.txhash}`
    );

    network.stt_proxy_rewards_set_in_generator = true;
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(
      `STT-UST PROXY both added and set with Astroport generator on bombay-12`
    );
  }

  // ##################### ASTROPORT GENERATOR :: SET & ADD VKR REWARD PROXY #####################

  if (
    !network.vkr_proxy_rewards_set_in_generator &&
    network.vkr_generator_proxy_contract_address
  ) {
    // Query config ==> add proxy to the list of allowed proxies ==> set proxies as allowed in generator
    if (!network.vkr_generator_proxy_set_in_generator) {
      let config = await query_generator_config(
        terra,
        network.astroport_generator_address
      );
      let new_allowed_proxies: Array<String> = config.allowed_reward_proxies;
      new_allowed_proxies.push(
        network.vkr_generator_proxy_contract_address as String
      );

      console.log(
        `Set the VKR proxy as allowed in generator... Allowed proxies with new one: ${new_allowed_proxies}`
      );
      let tx_set_proxy = await add_proxy_as_allowed_generator(
        terra,
        wallet,
        network.astroport_generator_address,
        new_allowed_proxies,
        "VKR-UST"
      );
      network.vkr_generator_proxy_set_in_generator = true;
      console.log(
        `VKR proxy set successfully as allowed in generator... ${tx_set_proxy.txhash}`
      );
      writeArtifact(network, terra.config.chainID);
      await delay(900);
    }
    // Register VKR-UST LP token with the astroport generator
    let tx_set_reward = await register_lp_tokens_in_generator(
      terra,
      wallet,
      network.astroport_generator_address,
      network.vkr_ust_astroport_lp_token_address,
      network.vkr_generator_proxy_contract_address,
      String(VKR_ALLOC_POINT),
      "VKR-UST"
    );
    console.log(
      `Adding VKR-UST LP rewards with the proxy contract to the astroport generator = ${tx_set_reward.txhash}`
    );

    network.vkr_proxy_rewards_set_in_generator = true;
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(
      `VKR-UST PROXY both added and set with Astroport generator on bombay-12`
    );
  }

  // ##################### ASTROPORT GENERATOR :: SET & ADD MINE REWARD PROXY #####################

  if (
    !network.mine_proxy_rewards_set_in_generator &&
    network.mine_generator_proxy_contract_address
  ) {
    // Query config ==> add proxy to the list of allowed proxies ==> set proxies as allowed in generator
    if (!network.mine_generator_proxy_set_in_generator) {
      let config = await query_generator_config(
        terra,
        network.astroport_generator_address
      );
      let new_allowed_proxies: Array<String> = config.allowed_reward_proxies;
      new_allowed_proxies.push(
        network.mine_generator_proxy_contract_address as String
      );

      console.log(
        `Set the MINE proxy as allowed in generator... Allowed proxies with new one: ${new_allowed_proxies}`
      );
      let tx_set_proxy = await add_proxy_as_allowed_generator(
        terra,
        wallet,
        network.astroport_generator_address,
        new_allowed_proxies,
        "MINE-UST"
      );
      network.mine_generator_proxy_set_in_generator = true;
      console.log(
        `MINE proxy set successfully as allowed in generator... ${tx_set_proxy.txhash}`
      );
      writeArtifact(network, terra.config.chainID);
      await delay(900);
    }
    // Register MINE-UST LP token with the astroport generator
    let tx_set_reward = await register_lp_tokens_in_generator(
      terra,
      wallet,
      network.astroport_generator_address,
      network.mine_ust_astroport_lp_token_address,
      network.mine_generator_proxy_contract_address,
      String(MINE_ALLOC_POINT),
      "MINE-UST"
    );
    console.log(
      `Adding MINE-UST LP rewards with the proxy contract to the astroport generator = ${tx_set_reward.txhash}`
    );

    network.mine_proxy_rewards_set_in_generator = true;
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(
      `MINE-UST PROXY both added and set with Astroport generator on bombay-12`
    );
  }

  // ##################### ASTROPORT GENERATOR :: SET & ADD PSI REWARD PROXY #####################

  if (
    !network.psi_proxy_rewards_set_in_generator &&
    network.psi_generator_proxy_contract_address
  ) {
    // Query config ==> add proxy to the list of allowed proxies ==> set proxies as allowed in generator
    if (!network.psi_generator_proxy_set_in_generator) {
      let config = await query_generator_config(
        terra,
        network.astroport_generator_address
      );
      let new_allowed_proxies: Array<String> = config.allowed_reward_proxies;
      new_allowed_proxies.push(
        network.psi_generator_proxy_contract_address as String
      );

      console.log(
        `Set the PSI proxy as allowed in generator... Allowed proxies with new one: ${new_allowed_proxies}`
      );
      let tx_set_proxy = await add_proxy_as_allowed_generator(
        terra,
        wallet,
        network.astroport_generator_address,
        new_allowed_proxies,
        "PSI-UST"
      );
      network.psi_generator_proxy_set_in_generator = true;
      console.log(
        `PSI proxy set successfully as allowed in generator... ${tx_set_proxy.txhash}`
      );
      writeArtifact(network, terra.config.chainID);
      await delay(900);
    }
    // Register PSI-UST LP token with the astroport generator
    let tx_set_reward = await register_lp_tokens_in_generator(
      terra,
      wallet,
      network.astroport_generator_address,
      network.psi_ust_astroport_lp_token_address,
      network.psi_generator_proxy_contract_address,
      String(PSI_ALLOC_POINT),
      "PSI-UST"
    );
    console.log(
      `Adding PSI-UST LP rewards with the proxy contract to the astroport generator = ${tx_set_reward.txhash}`
    );

    network.psi_proxy_rewards_set_in_generator = true;
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(
      `PSI-UST PROXY both added and set with Astroport generator on bombay-12`
    );
  }
}

function delay(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

main().catch(console.log);
