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

  let ADDRESSES = [
    "terra17mpuq65hw5kt7d44kpw4nk7x339xhznaa0duzv", // Ramon
    "terra1lv845g7szf9m3082qn3eehv9ewkjjr2kdyz0t6", // Arthur
    "terra1mthww38ea56sjmwtswlvy0g4zzjspjwaw3e6t8", // Sage
    "terra1jdd392vxlx5u23yvekvvjzuna0gszttgtngfnt", // Stefan
  ];

  for (let i = 0; i < ADDRESSES.length; i++) {
    console.log(`\n \n \n Sending to ${ADDRESSES[i]}`);
    // Send LUNA-UST LP Tokens
    // let transfer_tx = await executeContract(
    //   terra,
    //   wallet,
    //   network.luna_ust_terraswap_lp_token_address,
    //   {
    //     transfer: {
    //       recipient: ADDRESSES[i],
    //       amount: "100000",
    //     },
    //   }
    // );
    // console.log(`Transferring  LUNA-UST LP Tokens : ${transfer_tx.txhash}`);
    // Send LUNA-BLUNA LP Tokens
    await delay(300);
    let transfer_tx = await executeContract(
      terra,
      wallet,
      network.bluna_luna_terraswap_lp_token_address,
      {
        transfer: {
          recipient: ADDRESSES[i],
          amount: "1000000000",
        },
      }
    );
    console.log(`Transferring LUNA-BLUNA LP Tokens : ${transfer_tx.txhash}`);
    // Send ANC-UST LP Tokens
    transfer_tx = await executeContract(
      terra,
      wallet,
      network.anc_ust_terraswap_lp_token_address,
      {
        transfer: {
          recipient: ADDRESSES[i],
          amount: "1000000000",
        },
      }
    );
    console.log(`Transferring ANC-UST LP Tokens : ${transfer_tx.txhash}`);
    await delay(300);
    // Send MIR-UST LP Tokens
    transfer_tx = await executeContract(
      terra,
      wallet,
      network.mir_ust_terraswap_lp_token_address,
      {
        transfer: {
          recipient: ADDRESSES[i],
          amount: "1000000000",
        },
      }
    );
    console.log(`Transferring MIR-UST LP Tokens : ${transfer_tx.txhash}`);
    await delay(300);
    await delay(300);
    await delay(300);
    // Send PSI-UST LP Tokens
    transfer_tx = await executeContract(
      terra,
      wallet,
      network.psi_ust_terraswap_lp_token_address,
      {
        transfer: {
          recipient: ADDRESSES[i],
          amount: "1000000000",
        },
      }
    );
    console.log(`Transferring PSI-UST LP Tokens : ${transfer_tx.txhash}`);
    // Send ORION-UST LP Tokens
    transfer_tx = await executeContract(
      terra,
      wallet,
      network.orion_ust_terraswap_lp_token_address,
      {
        transfer: {
          recipient: ADDRESSES[i],
          amount: "1000000000",
        },
      }
    );
    await delay(300);
    await delay(300);
    console.log(`Transferring ORION-UST LP Tokens : ${transfer_tx.txhash}`);
    // Send STT-UST LP Tokens

    transfer_tx = await executeContract(
      terra,
      wallet,
      network.stt_ust_terraswap_lp_token_address,
      {
        transfer: {
          recipient: ADDRESSES[i],
          amount: "1000000000",
        },
      }
    );
    await delay(300);

    console.log(`Transferring STT-UST LP Tokens : ${transfer_tx.txhash}`);
    // Send VKR-UST LP Tokens

    transfer_tx = await executeContract(
      terra,
      wallet,
      network.vkr_ust_terraswap_lp_token_address,
      {
        transfer: {
          recipient: ADDRESSES[i],
          amount: "1000000000",
        },
      }
    );
    await delay(300);
    console.log(`Transferring VKR-UST LP Tokens : ${transfer_tx.txhash}`);
    // Send MINE-UST LP Tokens
    await delay(300);
    transfer_tx = await executeContract(
      terra,
      wallet,
      network.mine_ust_terraswap_lp_token_address,
      {
        transfer: {
          recipient: ADDRESSES[i],
          amount: "1000000000",
        },
      }
    );
    await delay(300);
    console.log(`Transferring MINE-UST LP Tokens : ${transfer_tx.txhash}`);
    // Send APOLLO-UST LP Tokens
    transfer_tx = await executeContract(
      terra,
      wallet,
      network.apollo_ust_terraswap_lp_token_address,
      {
        transfer: {
          recipient: ADDRESSES[i],
          amount: "1000000000",
        },
      }
    );
    console.log(`Transferring APOLLO-UST LP Tokens : ${transfer_tx.txhash}`);
  }

  await delay(300);
  await delay(300);
}

function delay(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

main().catch(console.log);
