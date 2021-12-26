import "dotenv/config";
import {
  executeContract,
  newClient,
  readArtifact,
  writeArtifact,
  extract_astroport_pool_info,
} from "./helpers/helpers.js";
import { join } from "path";

const ARTIFACTS_PATH = "../artifacts";

// ########### ASTROPORT DEX :: INITIALIZES ASTRO-UST POOL ###########

async function main() {
  const { terra, wallet } = newClient();
  console.log(
    `chainID: ${terra.config.chainID} wallet: ${wallet.key.accAddress}`
  );

  const network = readArtifact(terra.config.chainID);
  console.log("network:", network);

  // Astroport factory address should be set
  if (!network.astroport_factory_address) {
    console.log(
      `Please set Astroport factory address in the deploy config before running this script...`
    );
    return;
  }

  // ASTROPORT :: CREATE PAIR :: ASTRO/UST
  if (!network.astro_ust_astroport_pool) {
    console.log(
      `${terra.config.chainID} :: Creating ASTRO/UST pool on Astroport`
    );
    // create pair tx
    let tx = await executeContract(
      terra,
      wallet,
      network.astroport_factory_address,
      {
        create_pair: {
          pair_type: { xyk: {} },
          asset_infos: [
            { token: { contract_addr: network.astro_token_address } },
            { native_token: { denom: "uusd" } },
          ],
          init_params: null,
        },
      },
      [],
      "Astroport :: Initializing ASTRO/UST Pool"
    );
    let tx_resp = extract_astroport_pool_info(tx);
    network.astro_ust_astroport_pool = tx_resp.pool_address;
    network.astro_ust_astroport_lp_token_address = tx_resp.lp_token_address;
    writeArtifact(network, terra.config.chainID);
    console.log(
      `ASTRO/UST pool on Astroport successfully initialized ${tx.txhash}:: ${terra.config.chainID}\n`
    );
  } else {
    console.log(
      `ASTRO/UST pool on already exists on Astroport :: ${terra.config.chainID}`
    );
  }
}

main().catch(console.log);
