use cosmwasm_std::{from_slice, Addr, Empty, QuerierWrapper, StdResult, Uint128};
use cw_storage_plus::Path;
use serde::Deserialize;

/// @dev Returns generator deposit of tokens for the specified address
pub fn raw_generator_deposit(
    querier: QuerierWrapper,
    generator: &Addr,
    lp_token: &[u8],
    address: &[u8],
) -> StdResult<Uint128> {
    #[derive(Deserialize)]
    struct UserInfo {
        amount: Uint128,
    }

    let key: Path<Empty> = Path::new(b"user_info", &[lp_token, address]);
    if let Some(res) = &querier.query_wasm_raw(generator, key.to_vec())? {
        let UserInfo { amount } = from_slice(res)?;
        Ok(amount)
    } else {
        Ok(Uint128::zero())
    }
}

/// @dev Returns balance of tokens for the specified address
pub fn raw_balance(querier: QuerierWrapper, token: &Addr, address: &[u8]) -> StdResult<Uint128> {
    let key: Path<Empty> = Path::new(b"balance", &[address]);
    if let Some(res) = &querier.query_wasm_raw(token, key.to_vec())? {
        let res: Uint128 = from_slice(res)?;
        Ok(res)
    } else {
        Ok(Uint128::zero())
    }
}
