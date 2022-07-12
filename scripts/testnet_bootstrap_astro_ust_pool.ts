import "dotenv/config";
import {
  executeContract,
  newClient,
  readArtifact,
  writeArtifact,
} from "./helpers/helpers.js";
import { pisco_testnet, mainnet, Config } from "./deploy_configs.js";

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

  // Auction:::UpdateConfig :: SET ASTRO-UST pool in Auction if bombay-12
  if (
    !network.astro_ust_pool_set_in_auction &&
    network.astro_ust_astroport_pool
  ) {
    console.log(
      `${terra.config.chainID} :: Setting ASTRO-UST pool in Auction...`
    );
    let tx = await executeContract(
      terra,
      wallet,
      network.auction_address,
      {
        update_config: {
          new_config: {
            owner: undefined,
            astro_ust_pair_address: network.astro_ust_astroport_pool,
            generator_address: undefined,
          },
        },
      },
      [],
      "Auction ::: UpdateConfig ::: Setting ASTRO-UST pool"
    );
    console.log(
      `Auction :: Setting ASTRO-UST pool in Auction ==> ${tx.txhash}\n`
    );
    network.astro_ust_pool_set_in_auction = true;
    writeArtifact(network, terra.config.chainID);
  }

  // Auction:::InitPool :: Initialize ASTRO-UST pool if bombay-12
  if (
    !network.astro_ust_pool_initialized &&
    network.astro_ust_pool_set_in_auction
  ) {
    let out = await executeContract(
      terra,
      wallet,
      network.auction_address,
      {
        init_pool: {},
      },
      [],
      "Auction ::: InitPool ::: Initialize ASTRO-UST pool on Astroport"
    );
    console.log(
      `${terra.config.chainID} :: Initializing ASTRO-UST pool on Astroport ==>  ${out.txhash}`
    );
    network.auction_set_in_airdrop = true;
    writeArtifact(network, terra.config.chainID);
  }

  console.log("FINISH");
}

main().catch(console.log);
