#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Deps, Env, StdResult,
    Uint128, QueryRequest, BankQuery,
    Coin, AllBalanceResponse,
};

use cw20::{ Cw20QueryMsg, BalanceResponse as Cw20BalanceResponse, TokenInfoResponse };

use crate::msg::{QueryMsg};
use crate::state::{Config, PROJECT_INFOS, ProjectInfo};
use crate::contract::{ calc_pending };


#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetBalance{ project_id, wallet } => 
            to_binary(&query_balance(deps, _env, project_id, wallet)?),
            
        QueryMsg::GetConfig{ project_id } => 
            to_binary(&query_getconfig(deps, project_id)?),

        QueryMsg::GetProjectInfo{ project_id } => 
            to_binary(&query_getprojectinfo(deps, project_id)?),

        QueryMsg::GetPendingTokens{ project_id, wallet } => 
            to_binary(&query_pendingtokens(deps, _env, project_id, wallet)?),
    }
}
fn query_pendingtokens(deps:Deps, _env:Env, project_id:u32, wallet: String) 
    -> StdResult<Uint128> 
{
    let x = PROJECT_INFOS.load(deps.storage, project_id)?;
    let mut index = x.seed_users.iter().position(|x| x.wallet_address == wallet);
    let mut amount = Uint128::zero();
    if index != None {
        let pending_amount = calc_pending(
            deps.storage, _env.clone(), project_id, x.seed_users[index.unwrap()].clone(), "seed".to_string()
        );
        amount += pending_amount;
    }

    index = x.presale_users.iter().position(|x| x.wallet_address == wallet);
    if index != None {
        let pending_amount = calc_pending(
            deps.storage, _env.clone(), project_id, x.presale_users[index.unwrap()].clone(), "presale".to_string()
        );
        amount += pending_amount;
    }

    index = x.ido_users.iter().position(|x| x.wallet_address == wallet);
    if index != None {
        let pending_amount = calc_pending(
            deps.storage, _env.clone(), project_id, x.ido_users[index.unwrap()].clone(), "ido".to_string()
        );
        amount += pending_amount;
    }

    Ok(amount)
}
fn query_getprojectinfo(deps:Deps, project_id:u32) -> StdResult<ProjectInfo>{
    let x = PROJECT_INFOS.load(deps.storage, project_id)?;
    Ok(x)
}

fn query_balance(deps:Deps, _env:Env, project_id:u32, wallet:String) -> StdResult<AllBalanceResponse>{

    // let uusd_denom = String::from("uusd");
    let mut balance: AllBalanceResponse = deps.querier.query(
        &QueryRequest::Bank(BankQuery::AllBalances {
            address: wallet.clone(),
        }
    ))?;

    let x = PROJECT_INFOS.load(deps.storage, project_id)?;

    let token_balance: Cw20BalanceResponse = deps.querier.query_wasm_smart(
        x.clone().config.token_addr,
        &Cw20QueryMsg::Balance{
            address: wallet,
        }
    )?;
    let token_info: TokenInfoResponse = deps.querier.query_wasm_smart(
        x.config.token_addr.clone(),
        &Cw20QueryMsg::TokenInfo{}
    )?;
    balance.amount.push(Coin::new(token_balance.balance.u128(), token_info.name));

    Ok(balance)
}
fn query_getconfig(deps:Deps, project_id:u32) -> StdResult<Config> {
    let x = PROJECT_INFOS.load(deps.storage, project_id)?;
    Ok(x.config)
}
