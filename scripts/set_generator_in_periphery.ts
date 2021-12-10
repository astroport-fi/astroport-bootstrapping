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
import { bombay_testnet, mainnet, Config } from "./deploy_configs.js";
import { join } from "path";

let MULTI_SIG_TO_USE = "";

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

  // Astroport generator addresss should be set
  if (!network.astroport_generator_address) {
    console.log(
      `Please deploy the generator contract and then set this address in the <chain-id>.json before running this script...`
    );
    return;
  }

  /*************************************** PERIPHERY :: IF NETWORK IS BOMBAY-12  *****************************************/

  if (terra.config.chainID == "bombay-12") {
    //  LOCKDROP::UpdateConfig :: SET astroport generator address in Lockdrop if bombay-12
    if (!network.generator_set_in_lockdrop) {
      console.log(
        `${terra.config.chainID} :: Setting astroport generator address in Lockdrop...`
      );
      let tx = await executeContract(
        terra,
        wallet,
        network.lockdropAddress,
        {
          update_config: {
            new_config: {
              owner: undefined,
              astro_token_address: undefined,
              auction_contract_address: undefined,
              generator_address: network.astroport_generator_address,
            },
          },
        },
        [],
        "Lockdrop : Setting astroport generator"
      );
      console.log(
        `Lockdrop ::: Astroport generator set successfully ::: ${tx.txhash}\n`
      );
      network.generator_set_in_lockdrop = true;
      writeArtifact(network, terra.config.chainID);
    } else {
      console.log(`Lockdrop ::: Astroport generator already set \n`);
    }

    //  Auction::UpdateConfig :: SET astroport generator address in Auction if bombay-12
    if (!network.generator_set_in_auction) {
      console.log(
        `${terra.config.chainID} :: Setting astroport generator address in Auction...`
      );
      let tx = await executeContract(
        terra,
        wallet,
        network.auction_Address,
        {
          update_config: {
            new_config: {
              owner: undefined,
              astro_ust_pair_address: undefined,
              generator_contract: network.astroport_generator_address,
            },
          },
        },
        [],
        "Auction : Setting astroport generator"
      );
      console.log(
        `Auction ::: Astroport generator set successfully ::: ${tx.txhash}\n`
      );
      network.generator_set_in_auction = true;
      writeArtifact(network, terra.config.chainID);
    } else {
      console.log(`Auction ::: Astroport generator already set \n`);
    }

    writeArtifact(network, terra.config.chainID);
    console.log("FINISH");
  }
}

main().catch(console.log);
