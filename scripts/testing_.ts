import "dotenv/config";
import {
  executeContract,
  newClient,
  executeContractJsonForMultiSig,
  readArtifact,
  writeArtifact,
  Client,
} from "./helpers/helpers.js";

async function main() {
  // terra, wallet
  const { terra, wallet } = newClient();
  console.log(
    `chainID: ${terra.config.chainID} wallet: ${wallet.key.accAddress}`
  );

  // network : stores contract addresses
  const network = readArtifact(terra.config.chainID);

  // TX
  let tx = await executeContract(
    terra,
    wallet,
    "terra1hq5yf80uk0pe49qe6yl9wjm7rjk6hnx94ugs9u",
    {
      claim: {
        claim_amount: "2503516700",
        merkle_proof: [
          "1078208148e42f2bccf929e8d2992936a7b2eb04eb6f9ba6f15e84a2ceaa3737",
          "1d6e1446388a3b1e3980eb40bd94e5fbbd5a92885501dd54594d694d6c030ae8",
          "ee3493cbec4244a447783cf4958d2341c2587c2923e4fc07024bface10efbd61",
        ],
        root_index: 0,
      },
    },
    [],
    "Claiming airdrop"
  );

  console.log(` ${tx.txhash}\n`);

  writeArtifact(network, terra.config.chainID);
}

main().catch(console.log);

// {"lock_up_info":{"user_address":"terra1yskm9s4r0h0egg3lxe5wmmppr9s6lfau4j8yhc","terraswap_lp_token":"terra18y7dnplnh8kncxmkuhnr3mdjc8wny0zrxreynv","duration":3}}
