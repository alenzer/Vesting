#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use std::collections::HashMap;

use cosmwasm_std::{
    Addr, to_binary, DepsMut, Env, MessageInfo, Response,
    Uint128, CosmosMsg, WasmMsg, Storage
};
use cw2::set_contract_version;
use cw20::{Cw20ExecuteMsg, Cw20QueryMsg, BalanceResponse as Cw20BalanceResponse, TokenInfoResponse};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg};
use crate::state::{ProjectInfo, UserInfo, VestingParameter, PROJECT_SEQ, PROJECT_INFOS, OWNER, Config,
    };

// version info for migration info
const CONTRACT_NAME: &str = "Vesting";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    PROJECT_SEQ.save(deps.storage, &(Uint128::zero()))?;

    let owner = msg
        .admin
        .and_then(|s| deps.api.addr_validate(s.as_str()).ok()) 
        .unwrap_or(info.sender.clone());
    OWNER.save(deps.storage, &owner)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::SetConfig{ admin }
            => try_setconfig(deps, info, admin),

        ExecuteMsg::AddProject{ project_id, admin, token_addr, start_time }
            => try_addproject(deps, info, project_id, admin, token_addr, start_time ),

        ExecuteMsg::SetProjectInfo{ project_id, project_info }
            => try_setprojectinfo(deps, info, project_id, project_info ),

        ExecuteMsg::SetProjectConfig{ project_id, admin, token_addr , start_time} 
            => try_setprojectconfig(deps, info, project_id, admin, token_addr, start_time),

        ExecuteMsg::SetVestingParameters{ project_id, params }
            => try_setvestingparameters(deps, info, project_id, params),

        ExecuteMsg::SetSeedUsers { project_id, user_infos } 
            =>  try_setseedusers(deps, info, project_id, user_infos),

        ExecuteMsg::AddSeedUser { project_id, wallet, amount } 
            =>  try_addseeduser(deps, info, project_id, wallet, amount),

        ExecuteMsg::SetPresaleUsers { project_id, user_infos } 
            =>  try_setpresaleusers(deps, info, project_id, user_infos),

        ExecuteMsg::AddPresaleUser { project_id, wallet, amount } 
            =>  try_addpresaleuser(deps, info, project_id, wallet, amount),

        ExecuteMsg::SetIDOUsers { project_id, user_infos } 
            =>  try_setidousers(deps, info, project_id, user_infos),

        ExecuteMsg::AddIDOUser { project_id, wallet, amount } 
            =>  try_addidouser(deps, info, project_id, wallet, amount),

        ExecuteMsg::ClaimPendingTokens { project_id, }
            =>  try_claimpendingtokens(deps, _env, info, project_id )
    }
}
pub fn try_setprojectinfo(deps: DepsMut, info: MessageInfo, project_id: Uint128, project_info: ProjectInfo)
    ->Result<Response, ContractError>
{
    let mut x = PROJECT_INFOS.load(deps.storage, project_id.u128().into())?;
    if x.config.owner != info.sender {
        return Err(ContractError::Unauthorized{ });
    }
    x = project_info;
    PROJECT_INFOS.save(deps.storage, project_id.u128().into(), &x)?;
    Ok(Response::new()
    .add_attribute("action", "set Project Info"))    
}
pub fn try_setvestingparameters(deps: DepsMut, info: MessageInfo, project_id: Uint128, params: Vec<VestingParameter>)
    ->Result<Response, ContractError>
{
    let mut x = PROJECT_INFOS.load(deps.storage, project_id.u128().into())?;
    if x.config.owner != info.sender {
        return Err(ContractError::Unauthorized{ });
    }

    x.vest_param.insert("seed".to_string(), params[0]);
    x.vest_param.insert("presale".to_string(), params[1]);
    x.vest_param.insert("ido".to_string(), params[2]);

    PROJECT_INFOS.save(deps.storage, project_id.u128().into(), &x)?;
    Ok(Response::new()
    .add_attribute("action", "Set Vesting parameters"))
}

pub fn calc_pending(store: &dyn Storage, _env: Env, project_id: Uint128, user: UserInfo, stage: String)
    -> Uint128
{
    let x = PROJECT_INFOS.load(store, project_id.u128().into()).unwrap();
    let param = x.vest_param.get(&stage).unwrap();

    let past_time =Uint128::new(_env.block.time.seconds() as u128) - x.config.start_time;

    let mut unlocked = Uint128::zero();
    if past_time > Uint128::zero() {
        unlocked = user.total_amount * param.soon / Uint128::new(100);
    }
    let locked = user.total_amount - unlocked;
    if past_time > param.after {
        unlocked += (past_time - param.after) * locked / param.period;
        if unlocked >= user.total_amount{
            unlocked = user.total_amount;
        }
    }

    return unlocked - user.released_amount;
}

pub fn try_claimpendingtokens(deps: DepsMut, _env: Env, info: MessageInfo, project_id: Uint128 )
    ->Result<Response, ContractError>
{
    let mut x = PROJECT_INFOS.load(deps.storage, project_id.u128().into())?;
    let mut index = x.seed_users.iter().position(|x| x.wallet_address == info.sender);
    let mut amount = Uint128::zero();
    if index != None {
        let pending_amount = calc_pending(
            deps.storage, _env.clone(), project_id, x.seed_users[index.unwrap()].clone(), "seed".to_string()
        );
        x.seed_users[index.unwrap()].released_amount += pending_amount;
        amount += pending_amount;
    }

    index = x.presale_users.iter().position(|x| x.wallet_address == info.sender);
    if index != None {
        let pending_amount = calc_pending(
            deps.storage, _env.clone(), project_id, x.presale_users[index.unwrap()].clone(), "presale".to_string()
        );
        x.presale_users[index.unwrap()].released_amount += pending_amount;
        amount += pending_amount;
    }

    index = x.ido_users.iter().position(|x| x.wallet_address == info.sender);
    if index != None {
        let pending_amount = calc_pending(
            deps.storage, _env.clone(), project_id, x.ido_users[index.unwrap()].clone(), "ido".to_string()
        );
        x.ido_users[index.unwrap()].released_amount += pending_amount;
        amount += pending_amount;
    }
    if amount == Uint128::zero() {
        return Err(ContractError::NoPendingTokens{});
    }

    let token_info: TokenInfoResponse = deps.querier.query_wasm_smart(
        x.config.token_addr.clone(),
        &Cw20QueryMsg::TokenInfo{}
    )?;
    amount = amount * Uint128::new((10 as u128).pow(token_info.decimals as u32)); //for decimals

    let token_balance: Cw20BalanceResponse = deps.querier.query_wasm_smart(
        x.config.token_addr.clone(),
        &Cw20QueryMsg::Balance{
            address: _env.contract.address.to_string(),
        }
    )?;
    if token_balance.balance < amount {
        return Err(ContractError::NotEnoughBalance{})
    }

    let bank_cw20 = WasmMsg::Execute {
        contract_addr: String::from(x.config.token_addr),
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: info.sender.to_string(),
            amount: amount,
        }).unwrap(),
        funds: Vec::new()
    };

    Ok(Response::new()
    .add_message(CosmosMsg::Wasm(bank_cw20))
    .add_attribute("action", "Claim pending tokens"))
}

pub fn check_add_userinfo( users: &mut Vec<UserInfo>, wallet:Addr, amount: Uint128)
{
    let index =users.iter().position(|x| x.wallet_address == wallet);
    if index == None {
        users.push(UserInfo { 
            wallet_address: wallet, 
            total_amount: amount, 
            released_amount: Uint128::zero(), 
            pending_amount: Uint128::zero() 
        });
    }
    else{
        users[index.unwrap()].total_amount += amount;
    }
}
pub fn try_addseeduser(deps: DepsMut, info: MessageInfo, project_id: Uint128, wallet:Addr, amount: Uint128)
    ->Result<Response, ContractError>
{
    let mut x = PROJECT_INFOS.load(deps.storage, project_id.u128().into())?;
    if x.config.owner != info.sender {
        return Err(ContractError::Unauthorized{ });
    }

    check_add_userinfo(&mut x.seed_users, wallet, amount);
    PROJECT_INFOS.save(deps.storage, project_id.u128().into(), &x)?;

    Ok(Response::new()
    .add_attribute("action", "Add  User info for Seed stage"))
}
pub fn try_addpresaleuser(deps: DepsMut, info: MessageInfo, project_id: Uint128, wallet: Addr, amount:Uint128)
    ->Result<Response, ContractError>
{
    let mut x = PROJECT_INFOS.load(deps.storage, project_id.u128().into())?;
    if x.config.owner != info.sender {
        return Err(ContractError::Unauthorized{ });
    }

    check_add_userinfo(&mut x.presale_users, wallet, amount);
    PROJECT_INFOS.save(deps.storage, project_id.u128().into(), &x)?;

    Ok(Response::new()
    .add_attribute("action", "Add  User info for Presale stage"))
}
pub fn try_addidouser(deps: DepsMut, info: MessageInfo, project_id: Uint128, wallet:Addr, amount:Uint128)
    ->Result<Response, ContractError>
{
    let mut x = PROJECT_INFOS.load(deps.storage, project_id.u128().into())?;
    if x.config.owner != info.sender {
        return Err(ContractError::Unauthorized{ });
    }

    check_add_userinfo(&mut x.ido_users, wallet, amount);
    PROJECT_INFOS.save(deps.storage, project_id.u128().into(), &x)?;

    Ok(Response::new()
    .add_attribute("action", "Add  User info for IDO stage"))
}
pub fn try_setseedusers(deps: DepsMut, info: MessageInfo, project_id: Uint128, user_infos: Vec<UserInfo>)
    ->Result<Response, ContractError>
{
    let mut x = PROJECT_INFOS.load(deps.storage, project_id.u128().into())?;
    if x.config.owner != info.sender {
        return Err(ContractError::Unauthorized{ });
    }

    x.seed_users = user_infos;

    PROJECT_INFOS.save(deps.storage, project_id.u128().into(), &x)?;

    Ok(Response::new()
    .add_attribute("action", "Set User infos for Seed stage"))
}
pub fn try_setpresaleusers(deps: DepsMut, info: MessageInfo, project_id: Uint128, user_infos: Vec<UserInfo>)
    ->Result<Response, ContractError>
{
    let mut x = PROJECT_INFOS.load(deps.storage, project_id.u128().into())?;
    if x.config.owner != info.sender {
        return Err(ContractError::Unauthorized{ });
    }

    x.presale_users = user_infos;

    PROJECT_INFOS.save(deps.storage, project_id.u128().into(), &x)?;

    Ok(Response::new()
    .add_attribute("action", "Set User infos for Presale stage"))
}
pub fn try_setidousers(deps: DepsMut, info: MessageInfo, project_id: Uint128, user_infos: Vec<UserInfo>)
    ->Result<Response, ContractError>
{
    let mut x = PROJECT_INFOS.load(deps.storage, project_id.u128().into())?;
    if x.config.owner != info.sender {
        return Err(ContractError::Unauthorized{ });
    }

    x.ido_users = user_infos;

    PROJECT_INFOS.save(deps.storage, project_id.u128().into(), &x)?;

    Ok(Response::new()
    .add_attribute("action", "Set User infos for IDO stage"))
}

pub fn try_setprojectconfig(deps:DepsMut, info:MessageInfo,
    project_id: Uint128,
    admin: String, 
    token_addr: String,
    start_time: Uint128
) -> Result<Response, ContractError>
{
    //-----------check owner--------------------------
    let owner = OWNER.load(deps.storage).unwrap();
    if info.sender != owner {
        return Err(ContractError::Unauthorized{});
    }

    let mut x = PROJECT_INFOS.load(deps.storage, project_id.u128().into())?;
    x.config.owner = deps.api.addr_validate(admin.as_str())?;
    x.config.token_addr = deps.api.addr_validate(token_addr.as_str())?;
    x.config.start_time = start_time;

    PROJECT_INFOS.save(deps.storage, project_id.u128().into(), &x)?;
    Ok(Response::new()
        .add_attribute("action", "SetConfig"))                                
}

pub fn try_addproject(deps:DepsMut, info:MessageInfo,
    project_id: Uint128,
    admin: String, 
    token_addr: String,
    start_time: Uint128
) -> Result<Response, ContractError>
{
    //-----------check owner--------------------------
    let owner = OWNER.load(deps.storage).unwrap();
    if info.sender != owner {
        return Err(ContractError::Unauthorized{});
    }

    let config: Config = Config{
        owner: deps.api.addr_validate(admin.as_str())?,
        token_addr : deps.api.addr_validate(token_addr.as_str())?,
        start_time : start_time,
    };

    let project_info: ProjectInfo = ProjectInfo{
        project_id: Uint128::zero(),
        config: config,
        vest_param: HashMap::new(),
        seed_users: Vec::new(),
        presale_users: Vec::new(),
        ido_users: Vec::new()
    };

    PROJECT_INFOS.save(deps.storage, project_id.u128().into(), &project_info)?;

    let sec_per_month = 60 * 60 * 24 * 30;
    let seed_param = VestingParameter {
        soon: Uint128::new(15), //15% unlock at tge
        after: Uint128::new(sec_per_month), //after 1 month
        period: Uint128::new(sec_per_month * 6) //release over 6 month
    };
    let presale_param = VestingParameter {
        soon: Uint128::new(20), //20% unlock at tge
        after: Uint128::new(sec_per_month), //ater 1 month
        period: Uint128::new(sec_per_month * 5) //release over 5 month
    };
    let ido_param = VestingParameter {
        soon: Uint128::new(25), //25% unlock at tge
        after: Uint128::new(sec_per_month), //after 1 month
        period: Uint128::new(sec_per_month * 4) //release over 4 month
    };

    try_setvestingparameters(deps, info, project_info.project_id, vec![seed_param, presale_param, ido_param])?;

    Ok(Response::new()
        .add_attribute("action", "SetConfig"))                                
}
pub fn try_setconfig(deps:DepsMut, info:MessageInfo, admin: String) 
    -> Result<Response, ContractError>
{
    //-----------check owner--------------------------
    let owner = OWNER.load(deps.storage).unwrap();
    if info.sender != owner {
        return Err(ContractError::Unauthorized{});
    }

    let admin_addr = deps.api.addr_validate(&admin).unwrap();
    OWNER.save(deps.storage, &admin_addr)?;

    Ok(Response::new()
        .add_attribute("action", "SetConfig"))                                
}
