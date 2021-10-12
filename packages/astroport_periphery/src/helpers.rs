use crate::tax::deduct_tax;
use cosmwasm_bignumber::Uint256;
use cosmwasm_std::{
    to_binary, Addr, Api, BalanceResponse, BankMsg, BankQuery, Binary, Coin, CosmosMsg, Deps,
    QuerierWrapper, QueryRequest, StdResult, Uint128, WasmMsg, WasmQuery,
};
use cw20::BalanceResponse as CW20BalanceResponse;
use cw20_base::msg::{ExecuteMsg as CW20ExecuteMsg, QueryMsg as Cw20QueryMsg};

/// @dev Helper function which returns a cosmos wasm msg to transfer cw20 tokens to a recepient address
/// @param recipient : Address to be transferred cw20 tokens to
/// @param token_contract_address : Contract address of the cw20 token to transfer
/// @param amount : Number of tokens to transfer
pub fn build_transfer_cw20_token_msg(
    recipient: Addr,
    token_contract_address: String,
    amount: Uint128,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: token_contract_address,
        msg: to_binary(&CW20ExecuteMsg::Transfer {
            recipient: recipient.into(),
            amount,
        })?,
        funds: vec![],
    }))
}

/// Helper Function. Returns CosmosMsg which transfers CW20 Tokens from owner to recepient. (Transfers ASTRO from user to itself )
pub fn build_transfer_cw20_from_user_msg(
    cw20_token_address: String,
    owner: String,
    recepient: String,
    amount: Uint256,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: cw20_token_address,
        funds: vec![],
        msg: to_binary(&cw20::Cw20ExecuteMsg::TransferFrom {
            owner,
            recipient: recepient,
            amount: amount.into(),
        })?,
    }))
}

/// @dev Helper function which returns a cosmos wasm msg to send cw20 tokens to another contract which implements the ReceiveCW20 Hook
/// @param recipient_contract_addr : Contract Address to be transferred cw20 tokens to
/// @param token_contract_address : Contract address of the cw20 token to transfer
/// @param amount : Number of tokens to transfer
/// @param msg_ : ExecuteMsg coded into binary which needs to be handled by the recepient contract
pub fn build_send_cw20_token_msg(
    recipient_contract_addr: String,
    token_contract_address: String,
    amount: Uint128,
    msg_: Binary,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: token_contract_address,
        msg: to_binary(&CW20ExecuteMsg::Send {
            contract: recipient_contract_addr,
            amount,
            msg: msg_,
        })?,
        funds: vec![],
    }))
}

/// @dev Helper function which returns a cosmos wasm msg to send native tokens to recepient
/// @param recipient : Contract Address to be transferred native tokens to
/// @param denom : Native token to transfer
/// @param amount : Number of tokens to transfer
pub fn build_send_native_asset_msg(
    deps: Deps,
    recipient: Addr,
    denom: &str,
    amount: Uint256,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Bank(BankMsg::Send {
        to_address: recipient.into(),
        amount: vec![deduct_tax(
            deps,
            Coin {
                denom: denom.to_string(),
                amount: amount.into(),
            },
        )?],
    }))
}

/// Used when unwrapping an optional address sent in a contract call by a user.
/// Validates addreess if present, otherwise uses a given default value.
pub fn option_string_to_addr(
    api: &dyn Api,
    option_string: Option<String>,
    default: Addr,
) -> StdResult<Addr> {
    match option_string {
        Some(input_addr) => api.addr_validate(&input_addr),
        None => Ok(default),
    }
}

// native coins
pub fn get_denom_amount_from_coins(coins: &[Coin], denom: &str) -> Uint256 {
    coins
        .iter()
        .find(|c| c.denom == denom)
        .map(|c| Uint256::from(c.amount))
        .unwrap_or_else(Uint256::zero)
}

// CW20
pub fn cw20_get_balance(
    querier: &QuerierWrapper,
    token_address: Addr,
    account_addr: Addr,
) -> StdResult<Uint128> {
    let query: CW20BalanceResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: token_address.into(),
        msg: to_binary(&Cw20QueryMsg::Balance {
            address: account_addr.into(),
        })?,
    }))?;

    Ok(query.balance)
}

/// @dev Helper function which returns a cosmos wasm msg to approve held cw20 tokens to be transferrable by beneficiary address
/// @param token_contract_address : Token contract address
/// @param spender_address : Address to which allowance is being provided to, to allow it to transfer the tokens held by the contract
/// @param allowance_amount : Allowance amount
pub fn build_approve_cw20_msg(
    token_contract_address: String,
    spender_address: String,
    allowance_amount: Uint128,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: token_contract_address,
        msg: to_binary(&CW20ExecuteMsg::IncreaseAllowance {
            spender: spender_address,
            amount: allowance_amount,
            expires: None,
        })?,
        funds: vec![],
    }))
}

pub fn zero_address() -> Addr {
    Addr::unchecked("")
}

pub fn query_balance(
    querier: &QuerierWrapper,
    account_addr: Addr,
    denom: String,
) -> StdResult<Uint128> {
    let balance: BalanceResponse = querier.query(&QueryRequest::Bank(BankQuery::Balance {
        address: String::from(account_addr),
        denom,
    }))?;
    Ok(balance.amount.amount)
}

// Returns true if the user_info stuct's lockup_positions vector contains the string_
pub fn is_str_present_in_vec(vector_struct: Vec<String>, string_: String) -> bool {
    if vector_struct.iter().any(|id| id == &string_) {
        return true;
    }
    false
}
