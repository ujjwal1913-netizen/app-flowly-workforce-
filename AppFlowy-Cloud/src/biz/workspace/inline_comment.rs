use access_control::act::Action;
use actix_web::web::{Data, Json, Path, Query};
use actix_web::Result;
use app_error::AppError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared_entity::response::{AppResponse, JsonAppResponse};
use sqlx::Row;
use std::collections::HashMap;
use uuid::Uuid;

use crate::biz::authentication::jwt::UserUuid;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineCommentUserResponse {
  pub uuid: String,
  pub name: String,
  pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineCommentResponse {
  pub user: Option<InlineCommentUserResponse>,
  pub comment_id: Uuid,
  pub view_id: Uuid,
  pub block_id: Option<String>,
  pub content: String,
  pub reply_comment_id: Option<Uuid>,
  pub is_resolved: bool,
  pub is_deleted: bool,
  pub can_be_deleted: bool,
  pub created_at: DateTime<Utc>,
  pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineCommentsResponse {
  pub comments: Vec<InlineCommentResponse>,
  pub has_more: bool,
  pub next_offset: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineCommentReactionResponse {
  pub reaction_type: String,
  pub react_users: Vec<InlineCommentUserResponse>,
  pub comment_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetReactionsResponse {
  pub reactions: Vec<InlineCommentReactionResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetInlineCommentsQuery {
  pub limit: Option<i64>,
  pub offset: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInlineCommentRequest {
  pub content: String,
  pub block_id: Option<String>,
  pub reply_comment_id: Option<Uuid>,
  #[serde(default)]
  pub mentioned_user_uuids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInlineCommentResponse {
  pub comment_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteInlineCommentRequest {
  pub comment_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveInlineCommentRequest {
  pub comment_id: Uuid,
  pub is_resolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnchorUpdateRequest {
  pub comment_id: Uuid,
  pub doc_state: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetReactionsQuery {
  pub comment_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentReactionRequest {
  pub reaction_type: String,
  pub comment_id: Uuid,
}

pub async fn get_inline_comments(
  user_uuid: UserUuid,
  state: Data<AppState>,
  path: Path<(Uuid, Uuid)>,
  query: Query<GetInlineCommentsQuery>,
) -> Result<JsonAppResponse<InlineCommentsResponse>> {
  let (workspace_id, view_id) = path.into_inner();
  let uid = state.user_cache.get_user_uid(&user_uuid).await?;
  state
    .workspace_access_control
    .enforce_action(&uid, &workspace_id, Action::Read)
    .await?;

  let limit = query.limit.unwrap_or(50).clamp(1, 200);
  let offset = query.offset.unwrap_or(0).max(0);

  let rows = sqlx::query(
    r#"
    SELECT
      c.comment_id,
      c.view_id,
      c.block_id,
      c.content,
      c.reply_comment_id,
      c.is_resolved,
      c.is_deleted,
      c.created_at,
      c.updated_at,
      u.uuid as user_uuid,
      u.name as user_name,
      u.metadata ->> 'icon_url' as user_avatar_url,
      u.uid as user_uid
    FROM af_inline_comment c
    LEFT JOIN af_user u ON c.created_by = u.uid
    WHERE c.workspace_id = $1 AND c.view_id = $2
    ORDER BY c.created_at ASC
    LIMIT $3 OFFSET $4
    "#,
  )
  .bind(workspace_id)
  .bind(view_id)
  .bind(limit + 1)
  .bind(offset)
  .fetch_all(&state.pg_pool)
  .await
  .map_err(AppError::from)?;

  let has_more = rows.len() > limit as usize;
  let next_offset = if has_more { Some(offset + limit) } else { None };
  let fetch_rows = if has_more { &rows[..limit as usize] } else { &rows[..] };

  let comments = fetch_rows
    .iter()
    .map(|row| {
      let comment_id: Uuid = row.try_get("comment_id").unwrap_or_default();
      let view_id: Uuid = row.try_get("view_id").unwrap_or_default();
      let block_id: Option<String> = row.try_get("block_id").ok();
      let content: String = row.try_get("content").unwrap_or_default();
      let reply_comment_id: Option<Uuid> = row.try_get("reply_comment_id").ok();
      let is_resolved: bool = row.try_get("is_resolved").unwrap_or(false);
      let is_deleted: bool = row.try_get("is_deleted").unwrap_or(false);
      let created_at: DateTime<Utc> = row.try_get("created_at").unwrap_or_else(|_| Utc::now());
      let updated_at: DateTime<Utc> = row.try_get("updated_at").unwrap_or_else(|_| Utc::now());
      let user_uuid_val: Option<Uuid> = row.try_get("user_uuid").ok();
      let user_name_val: Option<String> = row.try_get("user_name").ok();
      let user_avatar_val: Option<String> = row.try_get("user_avatar_url").ok();
      let comment_user_uid: Option<i64> = row.try_get("user_uid").ok();

      let user = match (user_uuid_val, user_name_val) {
        (Some(u_uuid), Some(name)) => Some(InlineCommentUserResponse {
          uuid: u_uuid.to_string(),
          name,
          avatar_url: user_avatar_val,
        }),
        _ => None,
      };

      let can_be_deleted = !is_deleted && comment_user_uid == Some(uid);

      InlineCommentResponse {
        user,
        comment_id,
        view_id,
        block_id,
        content,
        reply_comment_id,
        is_resolved,
        is_deleted,
        can_be_deleted,
        created_at,
        updated_at,
      }
    })
    .collect();

  Ok(
    AppResponse::Ok()
      .with_data(InlineCommentsResponse {
        comments,
        has_more,
        next_offset,
      })
      .into(),
  )
}

pub async fn create_inline_comment_legacy(
  user_uuid: UserUuid,
  state: Data<AppState>,
  path: Path<(Uuid, Uuid)>,
  payload: Json<CreateInlineCommentRequest>,
) -> Result<JsonAppResponse<()>> {
  let (workspace_id, view_id) = path.into_inner();
  let uid = state.user_cache.get_user_uid(&user_uuid).await?;
  state
    .workspace_access_control
    .enforce_action(&uid, &workspace_id, Action::Write)
    .await?;

  let comment_id = Uuid::new_v4();
  sqlx::query(
    r#"
    INSERT INTO af_inline_comment (
      comment_id, workspace_id, view_id, block_id, content, reply_comment_id, created_by
    )
    VALUES ($1, $2, $3, $4, $5, $6, $7)
    "#,
  )
  .bind(comment_id)
  .bind(workspace_id)
  .bind(view_id)
  .bind(&payload.block_id)
  .bind(&payload.content)
  .bind(payload.reply_comment_id)
  .bind(uid)
  .execute(&state.pg_pool)
  .await
  .map_err(AppError::from)?;

  Ok(AppResponse::Ok().into())
}

pub async fn create_inline_comment_v2(
  user_uuid: UserUuid,
  state: Data<AppState>,
  path: Path<(Uuid, Uuid)>,
  payload: Json<CreateInlineCommentRequest>,
) -> Result<JsonAppResponse<CreateInlineCommentResponse>> {
  let (workspace_id, view_id) = path.into_inner();
  let uid = state.user_cache.get_user_uid(&user_uuid).await?;
  state
    .workspace_access_control
    .enforce_action(&uid, &workspace_id, Action::Write)
    .await?;

  let comment_id = Uuid::new_v4();
  sqlx::query(
    r#"
    INSERT INTO af_inline_comment (
      comment_id, workspace_id, view_id, block_id, content, reply_comment_id, created_by
    )
    VALUES ($1, $2, $3, $4, $5, $6, $7)
    "#,
  )
  .bind(comment_id)
  .bind(workspace_id)
  .bind(view_id)
  .bind(&payload.block_id)
  .bind(&payload.content)
  .bind(payload.reply_comment_id)
  .bind(uid)
  .execute(&state.pg_pool)
  .await
  .map_err(AppError::from)?;

  Ok(
    AppResponse::Ok()
      .with_data(CreateInlineCommentResponse { comment_id })
      .into(),
  )
}

pub async fn delete_inline_comment(
  user_uuid: UserUuid,
  state: Data<AppState>,
  path: Path<(Uuid, Uuid)>,
  payload: Json<DeleteInlineCommentRequest>,
) -> Result<JsonAppResponse<()>> {
  let (workspace_id, view_id) = path.into_inner();
  let uid = state.user_cache.get_user_uid(&user_uuid).await?;
  state
    .workspace_access_control
    .enforce_action(&uid, &workspace_id, Action::Write)
    .await?;

  sqlx::query(
    r#"
    UPDATE af_inline_comment
    SET is_deleted = TRUE, updated_at = CURRENT_TIMESTAMP
    WHERE comment_id = $1 AND workspace_id = $2 AND view_id = $3
    "#,
  )
  .bind(payload.comment_id)
  .bind(workspace_id)
  .bind(view_id)
  .execute(&state.pg_pool)
  .await
  .map_err(AppError::from)?;

  Ok(AppResponse::Ok().into())
}

pub async fn resolve_inline_comment(
  user_uuid: UserUuid,
  state: Data<AppState>,
  path: Path<(Uuid, Uuid)>,
  payload: Json<ResolveInlineCommentRequest>,
) -> Result<JsonAppResponse<()>> {
  let (workspace_id, view_id) = path.into_inner();
  let uid = state.user_cache.get_user_uid(&user_uuid).await?;
  state
    .workspace_access_control
    .enforce_action(&uid, &workspace_id, Action::Write)
    .await?;

  sqlx::query(
    r#"
    UPDATE af_inline_comment
    SET is_resolved = $4, updated_at = CURRENT_TIMESTAMP
    WHERE comment_id = $1 AND workspace_id = $2 AND view_id = $3
    "#,
  )
  .bind(payload.comment_id)
  .bind(workspace_id)
  .bind(view_id)
  .bind(payload.is_resolved)
  .execute(&state.pg_pool)
  .await
  .map_err(AppError::from)?;

  Ok(AppResponse::Ok().into())
}

pub async fn update_inline_comment_anchor(
  user_uuid: UserUuid,
  state: Data<AppState>,
  path: Path<(Uuid, Uuid)>,
  _payload: Json<AnchorUpdateRequest>,
) -> Result<JsonAppResponse<()>> {
  let (workspace_id, _view_id) = path.into_inner();
  let uid = state.user_cache.get_user_uid(&user_uuid).await?;
  state
    .workspace_access_control
    .enforce_action(&uid, &workspace_id, Action::Write)
    .await?;

  Ok(AppResponse::Ok().into())
}

pub async fn get_inline_comment_reactions(
  user_uuid: UserUuid,
  state: Data<AppState>,
  path: Path<(Uuid, Uuid)>,
  query: Query<GetReactionsQuery>,
) -> Result<JsonAppResponse<GetReactionsResponse>> {
  let (workspace_id, view_id) = path.into_inner();
  let uid = state.user_cache.get_user_uid(&user_uuid).await?;
  state
    .workspace_access_control
    .enforce_action(&uid, &workspace_id, Action::Read)
    .await?;

  let rows = if let Some(cid) = query.comment_id {
    sqlx::query(
      r#"
      SELECT
        r.comment_id,
        r.reaction_type,
        u.uuid as user_uuid,
        u.name as user_name,
        u.metadata ->> 'icon_url' as user_avatar_url
      FROM af_inline_comment_reaction r
      JOIN af_inline_comment c ON r.comment_id = c.comment_id
      LEFT JOIN af_user u ON r.created_by = u.uid
      WHERE c.workspace_id = $1 AND c.view_id = $2 AND r.comment_id = $3
      "#,
    )
    .bind(workspace_id)
    .bind(view_id)
    .bind(cid)
    .fetch_all(&state.pg_pool)
    .await
  } else {
    sqlx::query(
      r#"
      SELECT
        r.comment_id,
        r.reaction_type,
        u.uuid as user_uuid,
        u.name as user_name,
        u.metadata ->> 'icon_url' as user_avatar_url
      FROM af_inline_comment_reaction r
      JOIN af_inline_comment c ON r.comment_id = c.comment_id
      LEFT JOIN af_user u ON r.created_by = u.uid
      WHERE c.workspace_id = $1 AND c.view_id = $2
      "#,
    )
    .bind(workspace_id)
    .bind(view_id)
    .fetch_all(&state.pg_pool)
    .await
  }
  .map_err(AppError::from)?;

  let mut reaction_map: HashMap<(Uuid, String), Vec<InlineCommentUserResponse>> = HashMap::new();

  for row in rows {
    let comment_id: Uuid = row.try_get("comment_id").unwrap_or_default();
    let reaction_type: String = row.try_get("reaction_type").unwrap_or_default();
    let u_uuid: Option<Uuid> = row.try_get("user_uuid").ok();
    let u_name: Option<String> = row.try_get("user_name").ok();
    let u_avatar: Option<String> = row.try_get("user_avatar_url").ok();

    if let (Some(uuid_val), Some(name)) = (u_uuid, u_name) {
      reaction_map
        .entry((comment_id, reaction_type))
        .or_default()
        .push(InlineCommentUserResponse {
          uuid: uuid_val.to_string(),
          name,
          avatar_url: u_avatar,
        });
    }
  }

  let reactions = reaction_map
    .into_iter()
    .map(|((comment_id, reaction_type), react_users)| InlineCommentReactionResponse {
      reaction_type,
      react_users,
      comment_id,
    })
    .collect();

  Ok(AppResponse::Ok().with_data(GetReactionsResponse { reactions }).into())
}

pub async fn create_inline_comment_reaction(
  user_uuid: UserUuid,
  state: Data<AppState>,
  path: Path<(Uuid, Uuid)>,
  payload: Json<CommentReactionRequest>,
) -> Result<JsonAppResponse<()>> {
  let (workspace_id, _view_id) = path.into_inner();
  let uid = state.user_cache.get_user_uid(&user_uuid).await?;
  state
    .workspace_access_control
    .enforce_action(&uid, &workspace_id, Action::Write)
    .await?;

  sqlx::query(
    r#"
    INSERT INTO af_inline_comment_reaction (comment_id, reaction_type, created_by)
    VALUES ($1, $2, $3)
    ON CONFLICT (comment_id, reaction_type, created_by) DO NOTHING
    "#,
  )
  .bind(payload.comment_id)
  .bind(&payload.reaction_type)
  .bind(uid)
  .execute(&state.pg_pool)
  .await
  .map_err(AppError::from)?;

  Ok(AppResponse::Ok().into())
}

pub async fn delete_inline_comment_reaction(
  user_uuid: UserUuid,
  state: Data<AppState>,
  path: Path<(Uuid, Uuid)>,
  payload: Json<CommentReactionRequest>,
) -> Result<JsonAppResponse<()>> {
  let (workspace_id, _view_id) = path.into_inner();
  let uid = state.user_cache.get_user_uid(&user_uuid).await?;
  state
    .workspace_access_control
    .enforce_action(&uid, &workspace_id, Action::Write)
    .await?;

  sqlx::query(
    r#"
    DELETE FROM af_inline_comment_reaction
    WHERE comment_id = $1 AND reaction_type = $2 AND created_by = $3
    "#,
  )
  .bind(payload.comment_id)
  .bind(&payload.reaction_type)
  .bind(uid)
  .execute(&state.pg_pool)
  .await
  .map_err(AppError::from)?;

  Ok(AppResponse::Ok().into())
}
