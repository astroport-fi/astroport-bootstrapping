# Airdrop

The Airdrop contract facilitates direct claiming of ASTRO tokens airdropped to among initially unaccounted bLUNA collateral depositors into Anchor at block #4451515

## Contract Design

### Handle Messages

| Message                                | Description                                                                                                                         |
| -------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| `ExecuteMsg::Claim`                    | Executes an airdrop claim for Users.                                                                                                |
| `ExecuteMsg::TransferUnclaimedTokens`  | Admin function. Transfers unclaimed ASTRO tokens available with the contract to the recipient address once the claim window is over |
| `ExecuteMsg::UpdateConfig`             | Admin function to update any of the configuration parameters.                                                                       |
| `Cw20HookMsg::IncreaseAstroIncentives` | Admin Function to increase ASTRO incentives to be used for the airdrop                                                              |

### Query Messages

| Message                    | Description                                                                                           |
| -------------------------- | ----------------------------------------------------------------------------------------------------- |
| `QueryMsg::Config`         | Returns the config info                                                                               |
| `QueryMsg::State`          | Returns the contract's state info                                                                     |
| `QueryMsg::HasUserClaimed` | Returns a boolean value indicating if the corresponding address have yet claimed their airdrop or not |
| `QueryMsg::UserInfo`       | Returns user's airdrop claim state (total airdrop amount)                                             |

## How to Guide :: Get merkle proofs

### Create distribution lists for terra and evm users

claimees_data.json

```
{[ { address: 'terra1k0jntykt7e4g3y88ltc60czgjuqdy4c9ax8tx2',
    amount: '43454523323'
  },
  { address: 'terra1xzlgeyuuyqje79ma6vllregprkmgwgavjx2h6m',
    amount: '1343252443'
  }
]}
```

### Get proof with user input

```
    import  {Terra_Merkle_Tree}  from "./helpers/terra_merkle_tree.js";

    const terra_merkle_tree = new Terra_Merkle_Tree(terra_claimees_data);
    const terra_tree_root = terra_merkle_tree.getMerkleRoot();

    let merkle_proof_for_terra_user_ = terra_merkle_tree.getMerkleProof({  "address":"terra1k0jntykt7e4g3y88ltc60czgjuqdy4c9ax8tx2",
                                                                            "amount": (43454523323).toString()
                                                                        } );

    console.log("Terra Merkle Root ", terra_tree_root)
    console.log("Terra Merkle Proof ", merkle_proof_for_terra_user_)
    console.log("Verify Terra Merkle Proof ", terra_merkle_tree.verify({  "address":"terra1k0jntykt7e4g3y88ltc60czgjuqdy4c9ax8tx2",
                                                                            "amount": (43454523323).toString()
                                                                        }) )

```

## Build schema and run unit-tests

```
cargo schema
cargo test
```
