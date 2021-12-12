import "dotenv/config";
import {
  executeContract,
  newClient,
  readArtifact,
  writeArtifact,
} from "./helpers/helpers.js";
import { bombay_testnet, Config } from "./deploy_configs.js";

async function main() {
  // terra, wallet
  const { terra, wallet } = newClient();
  console.log(
    `chainID: ${terra.config.chainID} wallet: ${wallet.key.accAddress}`
  );

  // network : stores contract addresses
  const network = readArtifact(terra.config.chainID);
  console.log("network:", network);

  if (terra.config.chainID != "bombay-12") {
    console.log("Network is not testnet. Wrong script... terminating ... ");
    return;
  }

  // Astroport generator addresss should be set
  if (!network.astroport_generator_address) {
    console.log(
      `Please deploy the generator contract and then set this address in the <chain-id>.json before running this script...`
    );
    return;
  }

  //  LOCKDROP::UpdateConfig :: SET astroport generator address in Lockdrop if bombay-12
  if (!network.generator_set_in_lockdrop) {
    console.log(
      `${terra.config.chainID} :: Setting astroport generator address in Lockdrop...`
    );
    let tx = await executeContract(
      terra,
      wallet,
      network.lockdrop_address,
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
      network.auction_address,
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

main().catch(console.log);
