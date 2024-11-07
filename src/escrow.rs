use cosmwasm_std::{
    entry_point, to_json_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult, Uint128, BankMsg, Coin, WasmMsg, SubMsg, QueryRequest, WasmQuery,
};
use serde::{Deserialize, Serialize};
use cw_storage_plus::{Item, Map};

// Contract State
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct State {
    pub buyer: Addr,
    pub seller: Addr,
    pub sale_price: Uint128,
    pub state_percent: u64,
    pub seller_percent: u64,
    pub title: String,
    pub description: String,
    pub is_active: bool,
    pub is_cancelled: bool,
}

// Contract Status for queries
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ContractStatus {
    pub buyer: Addr,
    pub seller: Addr,
    pub sale_price: Uint128,
    pub state_percent: u64,
    pub seller_percent: u64,
    pub title: String,
    pub description: String,
    pub buyer_stake: bool,
    pub seller_stake: bool,
    pub buyer_cancel: bool,
    pub seller_cancel: bool,
    pub active: bool,
    pub cancelled: bool,
    pub agreement_address: Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InstantiateMsg {
    pub buyer: String,
    pub seller: String,
    pub sale_price: Uint128,
    pub state_percent: u64,
    pub seller_percent: u64,
    pub title: String,
    pub description: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ExecuteMsg {
    Stake {},
    RevokeStake {},
    Cancel {},
    RevokeCancellation {},
    Confirm {},
    StakeWithBabylon {
        babylon_stake_token: String,  // The staked token address from Babylon
        amount: Uint128,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum QueryMsg {
    GetStatus {},
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum BabylonMsg {
    VerifyStake {
        user: String,
        amount: Uint128,
    },
}

// State storage
const STATE: Item<State> = Item::new("state");
const STAKE_STATUS: Map<&Addr, bool> = Map::new("stake_status");
const CANCEL_STATUS: Map<&Addr, bool> = Map::new("cancel_status");
const STAKE_AMOUNTS: Map<&Addr, Uint128> = Map::new("stake_amounts");

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let buyer = deps.api.addr_validate(&msg.buyer)?;
    let seller = deps.api.addr_validate(&msg.seller)?;

    if buyer == seller {
        return Err(StdError::generic_err(
            "Buyer and seller addresses cannot be the same",
        ));
    }

    let state = State {
        buyer: buyer.clone(),
        seller: seller.clone(),
        sale_price: msg.sale_price,
        state_percent: msg.state_percent,
        seller_percent: msg.seller_percent,
        title: msg.title,
        description: msg.description,
        is_active: true,
        is_cancelled: false,
    };

    STATE.save(deps.storage, &state)?;

    let buyer_stake = msg.sale_price.multiply_ratio(msg.state_percent, 100u64);
    let seller_stake = msg.sale_price.multiply_ratio(msg.seller_percent, 100u64);

    STAKE_AMOUNTS.save(deps.storage, &buyer, &buyer_stake)?;
    STAKE_AMOUNTS.save(deps.storage, &seller, &seller_stake)?;

    Ok(Response::new().add_attribute("action", "instantiate"))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Stake {} => execute_stake(deps, env, info),
        ExecuteMsg::RevokeStake {} => execute_revoke_stake(deps, env, info),
        ExecuteMsg::Cancel {} => execute_cancel(deps, env, info),
        ExecuteMsg::RevokeCancellation {} => execute_revoke_cancellation(deps, env, info),
        ExecuteMsg::Confirm {} => execute_confirm(deps, env, info),
        ExecuteMsg::StakeWithBabylon { babylon_stake_token, amount } => {
            execute_stake_with_babylon(deps, env, info, babylon_stake_token, amount)
        },
    }
}

pub fn execute_stake_with_babylon(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    babylon_stake_token: String,
    amount: Uint128,
) -> StdResult<Response> {
    let state = STATE.load(deps.storage)?;

    if info.sender != state.buyer && info.sender != state.seller {
        return Err(StdError::generic_err("Unauthorized"));
    }

    let babylon_stake_msg = WasmMsg::Execute {
        contract_addr: "babylon_staking_contract_address".to_string(), // Replace with actual Babylon staking contract address
        msg: to_json_binary(&BabylonMsg::VerifyStake {
            user: info.sender.to_string(),
            amount,
        })?,
        funds: vec![],
    };

    STAKE_STATUS.save(deps.storage, &info.sender, &true)?;

    Ok(Response::new()
        .add_submessage(SubMsg::new(babylon_stake_msg))
        .add_attribute("action", "stake_with_babylon")
        .add_attribute("amount", amount.to_string()))
}

pub fn execute_cancel(deps: DepsMut, env: Env, info: MessageInfo) -> StdResult<Response> {
    let state = STATE.load(deps.storage)?;
    
    if info.sender != state.buyer && info.sender != state.seller {
        return Err(StdError::generic_err("Unauthorized"));
    }

    CANCEL_STATUS.save(deps.storage, &info.sender, &true)?;

    Ok(Response::new().add_attribute("action", "cancel"))
}

pub fn execute_revoke_cancellation(deps: DepsMut, env: Env, info: MessageInfo) -> StdResult<Response> {
    let state = STATE.load(deps.storage)?;
    
    if info.sender != state.buyer && info.sender != state.seller {
        return Err(StdError::generic_err("Unauthorized"));
    }

    let is_cancelled = CANCEL_STATUS.load(deps.storage, &info.sender)?;
    if !is_cancelled {
        return Err(StdError::generic_err("No cancellation found to revoke"));
    }

    CANCEL_STATUS.save(deps.storage, &info.sender, &false)?;

    Ok(Response::new().add_attribute("action", "revoke_cancellation"))
}

pub fn execute_confirm(deps: DepsMut, env: Env, info: MessageInfo) -> StdResult<Response> {
    let state = STATE.load(deps.storage)?;
    
    if info.sender != state.buyer && info.sender != state.seller {
        return Err(StdError::generic_err("Unauthorized"));
    }

    let buyer_staked = STAKE_STATUS.load(deps.storage, &state.buyer)?;
    let seller_staked = STAKE_STATUS.load(deps.storage, &state.seller)?;
    
    if !buyer_staked || !seller_staked {
        return Err(StdError::generic_err("Both parties must stake before confirmation"));
    }

    Ok(Response::new().add_attribute("action", "confirm"))
}

pub fn execute_revoke_stake(deps: DepsMut, env: Env, info: MessageInfo) -> StdResult<Response> {
    let state = STATE.load(deps.storage)?;
    
    if info.sender != state.buyer && info.sender != state.seller {
        return Err(StdError::generic_err("Unauthorized"));
    }

    let is_staked = STAKE_STATUS.load(deps.storage, &info.sender)?;
    if !is_staked {
        return Err(StdError::generic_err("No stake found to revoke"));
    }

    let stake_amount = STAKE_AMOUNTS.load(deps.storage, &info.sender)?;

    STAKE_STATUS.save(deps.storage, &info.sender, &false)?;

    let refund_msg = BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: vec![Coin {
            denom: "ujuno".to_string(),
            amount: stake_amount,
        }],
    };

    Ok(Response::new()
        .add_message(refund_msg)
        .add_attribute("action", "revoke_stake"))
}

pub fn query_status(deps: Deps, env: Env) -> StdResult<ContractStatus> {
    let state = STATE.load(deps.storage)?;
    let buyer_stake = STAKE_STATUS.load(deps.storage, &state.buyer).unwrap_or(false);
    let seller_stake = STAKE_STATUS.load(deps.storage, &state.seller).unwrap_or(false);
    let buyer_cancel = CANCEL_STATUS.load(deps.storage, &state.buyer).unwrap_or(false);
    let seller_cancel = CANCEL_STATUS.load(deps.storage, &state.seller).unwrap_or(false);

    Ok(ContractStatus {
        buyer: state.buyer,
        seller: state.seller,
        sale_price: state.sale_price,
        state_percent: state.state_percent,
        seller_percent: state.seller_percent,
        title: state.title,
        description: state.description,
        buyer_stake,
        seller_stake,
        buyer_cancel,
        seller_cancel,
        active: state.is_active,
        cancelled: state.is_cancelled,
        agreement_address: env.contract.address,
    })
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetStatus {} => to_json_binary(&query_status(deps, env)?),
    }
}
