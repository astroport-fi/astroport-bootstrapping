# Lockdrop

The lockdrop contract allows users to lock any of the supported Terraswap LP tokens locked for a selected duration against which they will receive ASTRO tokens pro-rata to their wighted share of the LP tokens to the total deposited LP tokens for that particular pool in the contract.

- Upon lockup expiration, users will receive Astroport LP tokens on an equivalent weight basis as per their initial Terraswap LP token deposits.

- Users can optionally unlock their lockup positions before the lockup duration completion by returning the ASTRO tokens which they received for participation in the lockup.

Note - Users can open muliple lockup positions with different lockup duration for each LP Token pool

## Contract Design

### Handle Messages

| Message                                       | Description                                                                                                                                                                                                                                                                                                                |
| --------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ExecuteMsg::UpdateConfig`                    | Can only be called by the admin. Facilitates updating configuration parameters                                                                                                                                                                                                                                             |
| `ExecuteMsg::EnableClaims`                    | Executed by the Bootstrap auction contract when liquidity is added to the ASTRO-UST pool. Enables ASTRO withdrawals by the lockdrop recepients.                                                                                                                                                                            |
| `ExecuteMsg::InitializePool`                  | Admin function. Facilitates updating ASTRO incentives % to be distributed among a particular LP token depositors                                                                                                                                                                                                           |
| `ExecuteMsg::UpdatePool`                      | Admin function to deposit net total locked UST into the Red Bank. Called after the deposit window is over.                                                                                                                                                                                                                 |
| `ExecuteMsg::IncreaseLockup`                  | Facilitates opening a new user position or adding to an existing position                                                                                                                                                                                                                                                  |
| `ExecuteMsg::WithdrawFromLockup`              | Facilitates LP token withdrawals from lockup positions by users. 100% amount can be withdrawn during deposit window, which is then limited to 50% during 1st half of deposit window which then decreases linearly during 2nd half of deposit window. Only 1 withdrawal can be made by a user during the withdrawal windows |
| `ExecuteMsg::MigrateLiquidity`                | Admin function. Facilitates migration of liquidity (locked LP tokens) from Terraswap to Astroport                                                                                                                                                                                                                          |
| `ExecuteMsg::StakeLpTokens`                   | Admin function. Facilitates staking of Astroport LP tokens for a particular LP pool with the generator contract                                                                                                                                                                                                            |
| `ExecuteMsg::DelegateAstroToAuction`          | This function facilitates ASTRO tokens delegation to the Bootstrap auction contract during the bootstrap auction phase. Delegated ASTRO tokens are added to the user's position in the bootstrap auction contract                                                                                                          |
| `ExecuteMsg::ClaimRewardsAndOptionallyUnlock` | Facilitates rewards claim by users for a particular lockup position along with unlock when possible                                                                                                                                                                                                                        |

### Handle Messages :: Callback

| Message                                               | Description                                                                                                                                                                |
| ----------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `CallbackMsg::UpdatePoolOnDualRewardsClaim`           | Callback function to update contract state after pending dual staking rewards are claimed from the generator contract                                                      |
| `CallbackMsg::WithdrawUserLockupRewardsCallback`      | Callback function to withdraw user rewards for a particular lokcup position along with optional LP tokens withdrawal, either forcefully or upon lockup duration expiration |
| `CallbackMsg::WithdrawLiquidityFromTerraswapCallback` | Callback function used during liquidity migration to update state after liquidity is removed from terraswap                                                                |

### Query Messages

| Message                | Description                                                                                                      |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------- |
| `QueryMsg::Config`     | Returns the config info                                                                                          |
| `QueryMsg::State`      | Returns the contract's global state                                                                              |
| `QueryMsg::Pool`       | Returns info regarding a certain supported LP token pool                                                         |
| `QueryMsg::UserInfo`   | Returns info regarding a user (total ASTRO rewards, list of lockup positions)                                    |
| `QueryMsg::LockUpInfo` | Returns info regarding a particular lockup position with a given duration and identifer for the LP tokens locked |

## Build schema and run unit-tests

```
cargo schema
cargo test
```

## License

TBD
