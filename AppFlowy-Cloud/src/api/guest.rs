use access_control::act::Action;
use actix_web::{
  web::{self, Data, Json},
  Result, Scope,
};
use app_error::AppError;
use database_entity::dto::{AFAccessLevel, AFRole};
use shared_entity::{
  dto::guest_dto::{
    RevokeSharedViewAccessRequest, ShareViewWithGuestRequest, SharedUser, SharedViewDetails,
    SharedViewDetailsRequest, SharedViews,
  },
  response::{AppResponse, JsonAppResponse},
};
use uuid::Uuid;

use crate::biz::authentication::jwt::UserUuid;
use crate::state::AppState;

pub fn sharing_scope() -> Scope {
  web::scope("/api/sharing/workspace")
    .service(
      web::resource("{workspace_id}/view")
        .route(web::get().to(list_shared_views_handler))
        .route(web::put().to(put_shared_view_handler)),
    )
    .service(
      web::resource("{workspace_id}/view/{view_id}")
        .route(web::get().to(crate::api::workspace::get_page_view_handler)),
    )
    .service(
      web::resource("{workspace_id}/view/{view_id}/access-details")
        .route(web::post().to(shared_view_access_details_handler)),
    )
    .service(
      web::resource("{workspace_id}/view/{view_id}/revoke-access")
        .route(web::post().to(revoke_shared_view_access_handler)),
    )
}

async fn list_shared_views_handler(
  user_uuid: UserUuid,
  state: Data<AppState>,
  path: web::Path<Uuid>,
) -> Result<JsonAppResponse<SharedViews>> {
  let workspace_id = path.into_inner();
  let uid = state.user_cache.get_user_uid(&user_uuid).await?;
  state
    .workspace_access_control
    .enforce_action(&uid, &workspace_id, Action::Read)
    .await?;

  Ok(
    AppResponse::Ok()
      .with_data(SharedViews {
        shared_views: vec![],
        view_id_with_no_access: vec![],
      })
      .into(),
  )
}

async fn put_shared_view_handler(
  user_uuid: UserUuid,
  state: Data<AppState>,
  _payload: web::Json<ShareViewWithGuestRequest>,
  path: web::Path<Uuid>,
) -> Result<JsonAppResponse<()>> {
  let workspace_id = path.into_inner();
  let uid = state.user_cache.get_user_uid(&user_uuid).await?;
  state
    .workspace_access_control
    .enforce_action(&uid, &workspace_id, Action::Write)
    .await?;

  Ok(AppResponse::Ok().into())
}

async fn shared_view_access_details_handler(
  user_uuid: UserUuid,
  state: Data<AppState>,
  _json: Json<SharedViewDetailsRequest>,
  path: web::Path<(Uuid, Uuid)>,
) -> Result<JsonAppResponse<SharedViewDetails>> {
  let (workspace_id, view_id) = path.into_inner();
  let uid = state.user_cache.get_user_uid(&user_uuid).await?;
  state
    .workspace_access_control
    .enforce_action(&uid, &workspace_id, Action::Read)
    .await?;

  let members = database::workspace::select_workspace_member_list_exclude_guest(
    &state.pg_pool,
    &workspace_id,
  )
  .await
  .map_err(AppError::from)?;

  let shared_with = members
    .into_iter()
    .map(|row| {
      let role = AFRole::from(row.role);
      let access_level = AFAccessLevel::from(&role);
      SharedUser {
        view_id,
        email: row.email,
        name: row.name,
        access_level,
        role,
        avatar_url: row.avatar_url,
        pending_invitation: false,
      }
    })
    .collect();

  Ok(
    AppResponse::Ok()
      .with_data(SharedViewDetails {
        view_id,
        shared_with,
      })
      .into(),
  )
}

async fn revoke_shared_view_access_handler(
  user_uuid: UserUuid,
  state: Data<AppState>,
  _payload: web::Json<RevokeSharedViewAccessRequest>,
  path: web::Path<(Uuid, Uuid)>,
) -> Result<JsonAppResponse<()>> {
  let (workspace_id, _view_id) = path.into_inner();
  let uid = state.user_cache.get_user_uid(&user_uuid).await?;
  state
    .workspace_access_control
    .enforce_action(&uid, &workspace_id, Action::Write)
    .await?;

  Ok(AppResponse::Ok().into())
}
