import { Merkle_Tree } from "./helpers/merkle_tree.js";
import { prepareDataForMerkleTree } from "./helpers/merkle_tree_utils.js";
import airdropdataTerra from "./helpers/airdrop_data/airdrop_recipients.json";
import fs from "fs";

const MERKLE_ROOTS = 1;

export async function create_json_with_proofs() {
  let n = MERKLE_ROOTS;
  let final_list_of_users_eligible_for_airdrop = [];

  // For each Merkle Tree, evaluate proofs for each user and enter it in the JSON
  for (let i = 0; i < n; i++) {
    // create merkle tree
    let terra_data = prepareDataForMerkleTree(
      airdropdataTerra.data,
      i * Math.round(airdropdataTerra.data.length / n),
      (i + 1) * Math.round(airdropdataTerra.data.length / n)
    );
    let terra_merkle_tree = new Merkle_Tree(terra_data);

    // PRINT MERKLE ROOT
    console.log(`Merkle Root : ${terra_merkle_tree.getMerkleRoot()}`);

    // calculate proof for each user
    for (let j = 0; j < terra_data.length; j++) {
      let user = { address: "", amount: "", merkle_proof: [""], index: 0 };
      user.address = terra_data[j]["address"];
      user.amount = terra_data[j]["amount"];
      user.merkle_proof = terra_merkle_tree.getMerkleProof({
        address: user.address,
        amount: user.amount,
      });
      user.index = i;
      final_list_of_users_eligible_for_airdrop.push(user);
    }
  }

  // Write final JSON to a .json file
  fs.writeFileSync(
    "list_of_users_eligible_for_airdrop.json",
    JSON.stringify(final_list_of_users_eligible_for_airdrop)
  );
}

create_json_with_proofs().catch(console.log);
