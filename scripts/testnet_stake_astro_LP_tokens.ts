import "dotenv/config";
import { Coin, LCDClient, LocalTerra, Wallet } from "@terra-money/terra.js";
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
import { bombay_testnet, mainnet, Config } from "./deploy_configs.js";
import { join } from "path";

let MULTI_SIG_TO_USE = "";

async function stake_astro_lp_tokens_lockdrop(
  terra: LocalTerra | LCDClient,
  wallet: Wallet,
  lockdrop_address: string,
  terraswap_lp_token: string,
  pair_name: string
) {
  let tx = await executeContract(
    terra,
    wallet,
    lockdrop_address,
    {
      stake_lp_tokens: {
        terraswap_lp_token: terraswap_lp_token,
      },
    },
    [],
    `Lockdrop :: Staking ${pair_name} LP tokens with the astroport generator`
  );
  return tx;
}

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
    CONFIGURATION = bombay_testnet;
  } else if (terra.config.chainID == "columbus-5") {
    CONFIGURATION = mainnet;
  }

  // ASTRO Token addresss should be set
  if (!network.lockdrop_address) {
    console.log(
      `Please deploy the Lockdrop Contract in the deploy config before running this script...`
    );
    return;
  }

  /*************************************** LOCKDROP :: IF NETWORK IS BOMBAY-12  *****************************************/
  /*************************************** LOCKDROP :: IF NETWORK IS BOMBAY-12  *****************************************/
  /*************************************** LOCKDROP :: IF NETWORK IS BOMBAY-12  *****************************************/
  /*************************************** LOCKDROP :: IF NETWORK IS BOMBAY-12  *****************************************/

  if (terra.config.chainID == "bombay-12") {
    // Staking LP tokens with astroport generator :: ANC/UST
    // if (
    //   !network.anc_lp_tokens_staked_with_generator &&
    //   // network.anc_proxy_rewards_set_in_generator &&
    //   network.anc_ust_liquidity_migrated
    // ) {
    //   console.log(
    //     `${terra.config.chainID} :: Lockdrop :: Staking LP tokens with astroport generator :: ANC/UST`
    //   );
    //   let tx = await stake_astro_lp_tokens_lockdrop(
    //     terra,
    //     wallet,
    //     network.lockdrop_address,
    //     network.anc_ust_terraswap_lp_token_address,
    //     "ANC-UST"
    //   );
    //   console.log(
    //     `Lockdrop :: LP Tokens successfully staked:: ANC/UST :: ${tx.txhash}\n`
    //   );
    //   network.anc_lp_tokens_staked_with_generator = true;
    //   writeArtifact(network, terra.config.chainID);
    // }

    // Staking LP tokens with astroport generator :: MIR/UST
    if (
      !network.mir_lp_tokens_staked_with_generator &&
      network.mir_proxy_rewards_set_in_generator &&
      network.mir_ust_liquidity_migrated
    ) {
      console.log(
        `${terra.config.chainID} :: Lockdrop :: Staking LP tokens with astroport generator :: MIR/UST`
      );
      let tx = await stake_astro_lp_tokens_lockdrop(
        terra,
        wallet,
        network.lockdrop_address,
        network.mir_ust_terraswap_lp_token_address,
        "MIR-UST"
      );
      console.log(
        `Lockdrop :: LP Tokens successfully staked:: MIR/UST :: ${tx.txhash}\n`
      );
      network.mir_lp_tokens_staked_with_generator = true;
      writeArtifact(network, terra.config.chainID);
    }

    // Staking LP tokens with astroport generator :: ORION/UST
    if (
      !network.orion_lp_tokens_staked_with_generator &&
      network.orion_proxy_rewards_set_in_generator &&
      network.orion_ust_liquidity_migrated
    ) {
      console.log(
        `${terra.config.chainID} :: Lockdrop :: Staking LP tokens with astroport generator :: ORION/UST`
      );
      let tx = await stake_astro_lp_tokens_lockdrop(
        terra,
        wallet,
        network.lockdrop_address,
        network.orion_ust_terraswap_lp_token_address,
        "ORION-UST"
      );
      console.log(
        `Lockdrop :: LP Tokens successfully staked:: ORION/UST :: ${tx.txhash}\n`
      );
      network.orion_lp_tokens_staked_with_generator = true;
      writeArtifact(network, terra.config.chainID);
    }

    // Staking LP tokens with astroport generator :: STT/UST
    if (
      !network.stt_lp_tokens_staked_with_generator &&
      network.stt_proxy_rewards_set_in_generator &&
      network.stt_ust_liquidity_migrated
    ) {
      console.log(
        `${terra.config.chainID} :: Lockdrop :: Staking LP tokens with astroport generator :: STT/UST`
      );
      let tx = await stake_astro_lp_tokens_lockdrop(
        terra,
        wallet,
        network.lockdrop_address,
        network.stt_ust_terraswap_lp_token_address,
        "STT-UST"
      );
      console.log(
        `Lockdrop :: LP Tokens successfully staked:: STT/UST :: ${tx.txhash}\n`
      );
      network.stt_lp_tokens_staked_with_generator = true;
      writeArtifact(network, terra.config.chainID);
    }

    // Staking LP tokens with astroport generator :: VKR/UST
    if (
      !network.vkr_lp_tokens_staked_with_generator &&
      network.vkr_proxy_rewards_set_in_generator &&
      network.vkr_ust_liquidity_migrated
    ) {
      console.log(
        `${terra.config.chainID} :: Lockdrop :: Staking LP tokens with astroport generator :: VKR/UST`
      );
      let tx = await stake_astro_lp_tokens_lockdrop(
        terra,
        wallet,
        network.lockdrop_address,
        network.vkr_ust_terraswap_lp_token_address,
        "VKR-UST"
      );
      console.log(
        `Lockdrop :: LP Tokens successfully staked:: VKR/UST :: ${tx.txhash}\n`
      );
      network.vkr_lp_tokens_staked_with_generator = true;
      writeArtifact(network, terra.config.chainID);
    }

    // Staking LP tokens with astroport generator :: MINE/UST
    if (
      !network.mine_lp_tokens_staked_with_generator &&
      network.mine_proxy_rewards_set_in_generator &&
      network.mine_ust_liquidity_migrated
    ) {
      console.log(
        `${terra.config.chainID} :: Lockdrop :: Staking LP tokens with astroport generator :: MINE/UST`
      );
      let tx = await stake_astro_lp_tokens_lockdrop(
        terra,
        wallet,
        network.lockdrop_address,
        network.mine_ust_terraswap_lp_token_address,
        "MINE-UST"
      );
      console.log(
        `Lockdrop :: LP Tokens successfully staked:: MINE/UST :: ${tx.txhash}\n`
      );
      network.mine_lp_tokens_staked_with_generator = true;
      writeArtifact(network, terra.config.chainID);
    }

    // Staking LP tokens with astroport generator :: PSI/UST
    if (
      !network.psi_lp_tokens_staked_with_generator &&
      network.psi_proxy_rewards_set_in_generator &&
      network.psi_ust_liquidity_migrated
    ) {
      console.log(
        `${terra.config.chainID} :: Lockdrop :: Staking LP tokens with astroport generator :: PSI/UST`
      );
      let tx = await stake_astro_lp_tokens_lockdrop(
        terra,
        wallet,
        network.lockdrop_address,
        network.psi_ust_terraswap_lp_token_address,
        "PSI-UST"
      );
      console.log(
        `Lockdrop :: LP Tokens successfully staked:: PSI/UST :: ${tx.txhash}\n`
      );
      network.psi_lp_tokens_staked_with_generator = true;
      writeArtifact(network, terra.config.chainID);
    }
  }

  console.log("FINISH");
}

main().catch(console.log);
