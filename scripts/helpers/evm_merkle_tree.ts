import keccak256 from 'keccak256';
import { MerkleTree } from 'merkletreejs';

export class EVM_Merkle_Tree {

  private tree: MerkleTree;

  constructor(accounts: Array<{ address: string; amount: string }>) {
    let leaves = accounts.map((a) => keccak256( (a.address.substr(2,42).toLowerCase() + a.amount).replace('0x', '') ) );
    leaves.sort();
    this.tree = new MerkleTree(leaves, keccak256, { sort: true });
  }

  getMerkleTree() {
    return this.tree;
  }

  getMerkleRoot() {
    return this.tree.getHexRoot().replace('0x', '');
  }

  getMerkleProof(account : {address: string; amount: string;}) : string[] {
    return this.tree.getHexProof(keccak256(  (account.address.substr(2,42).toLowerCase() + account.amount).replace('0x', '')  )).map((v) => v.replace('0x', ''));
  }

  verify( proof: string[], account: { address: string; amount: string }) {
    let leaf_evm = keccak256((account.address.substr(2,42).toLowerCase() + account.amount).replace('0x', ''))
    let is_valid = this.tree.verify(proof, leaf_evm ,this.tree.getHexRoot());  
    return is_valid;
  }
}