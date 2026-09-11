use std::collections::HashMap;
use uuid::Uuid;

use app_error::AppError;
use appflowy_collaborate::ws2::WorkspaceCollabInstanceCache;
use collab_rt_entity::user::RealtimeUser;
use database_entity::dto::AFAccessLevel;
use shared_entity::dto::workspace_dto::{
  AddSpaceMemberParams, CreateStructuredSpaceParams, Space, SpaceListItemDto, SpaceMemberDto,
  SpaceMembersResponseDto, SpacePermissionResponseDto, SpacePermissionSettingsDto, SpaceSecuritySettingsDto,
  SpacesResponseDto, UpdateSpaceMemberParams, UpdateStructuredSpaceParams,
};
use sqlx::Row;

use crate::biz::collab::folder_view::{
  check_if_space_is_private, check_if_view_is_space, parse_extra_field_as_json,
};
use crate::biz::workspace::page_view::{create_space, update_space};
use crate::state::AppState;

/// Creates a structured space with granular permissions and auto-registers the creator as owner.
pub async fn create_structured_space(
  state: &AppState,
  user: RealtimeUser,
  workspace_id: Uuid,
  payload: CreateStructuredSpaceParams,
) -> Result<Space, AppError> {
  let is_private = match &payload.permission {
    Some(p) if p.visibility == "private" => true,
    Some(p) if p.visibility == "custom" && p.everyone_else_access_level.is_none() => true,
    _ => false,
  };
  let legacy_perm = if is_private {
    shared_entity::dto::workspace_dto::SpacePermission::Private
  } else {
    shared_entity::dto::workspace_dto::SpacePermission::PublicToAll
  };

  let space = create_space(
    state,
    user.clone(),
    workspace_id,
    &legacy_perm,
    &payload.name,
    &payload.space_icon,
    &payload.space_icon_color,
    payload.view_id,
  )
  .await?;

  let space_id = space.view_id;

  let permission_settings = payload.permission.unwrap_or_else(|| SpacePermissionSettingsDto {
    visibility: if is_private { "private".to_string() } else { "public".to_string() },
    owner_access_level: AFAccessLevel::FullAccess,
    member_default_access_level: Some(AFAccessLevel::ReadAndWrite),
    everyone_else_access_level: None,
    invite_policy: "members_and_owners".to_string(),
    sidebar_edit_policy: "members_and_owners".to_string(),
    invite_link_enabled: false,
    security: Some(SpaceSecuritySettingsDto::default()),
  });

  let security_json = serde_json::to_value(&permission_settings.security).unwrap_or_default();

  sqlx::query(
    r#"
    INSERT INTO af_space_permission (
      space_id, workspace_id, visibility, owner_access_level,
      member_default_access_level, everyone_else_access_level,
      invite_policy, sidebar_edit_policy, invite_link_enabled, security
    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
    ON CONFLICT (space_id) DO UPDATE SET
      visibility = EXCLUDED.visibility,
      owner_access_level = EXCLUDED.owner_access_level,
      member_default_access_level = EXCLUDED.member_default_access_level,
      everyone_else_access_level = EXCLUDED.everyone_else_access_level,
      invite_policy = EXCLUDED.invite_policy,
      sidebar_edit_policy = EXCLUDED.sidebar_edit_policy,
      invite_link_enabled = EXCLUDED.invite_link_enabled,
      security = EXCLUDED.security,
      updated_at = CURRENT_TIMESTAMP
    "#,
  )
  .bind(space_id)
  .bind(workspace_id)
  .bind(&permission_settings.visibility)
  .bind(permission_settings.owner_access_level as i32)
  .bind(permission_settings.member_default_access_level.map(|l| l as i32))
  .bind(permission_settings.everyone_else_access_level.map(|l| l as i32))
  .bind(&permission_settings.invite_policy)
  .bind(&permission_settings.sidebar_edit_policy)
  .bind(permission_settings.invite_link_enabled)
  .bind(security_json)
  .execute(&state.pg_pool)
  .await?;

  // Automatically record creator as the space owner
  sqlx::query(
    r#"
    INSERT INTO af_space_member (space_id, workspace_id, uid, role, access_level)
    VALUES ($1, $2, $3, 'owner', 50)
    ON CONFLICT (space_id, uid) DO UPDATE SET
      role = 'owner',
      access_level = 50,
      updated_at = CURRENT_TIMESTAMP
    "#,
  )
  .bind(space_id)
  .bind(workspace_id)
  .bind(user.uid)
  .execute(&state.pg_pool)
  .await?;

  let _ = state
    .collab_access_control
    .update_access_level_policy(&user.uid, &space_id, AFAccessLevel::FullAccess)
    .await;

  Ok(space)
}

/// Lists all spaces in the workspace with membership stats and permissions.
pub async fn list_workspace_spaces(
  state: &AppState,
  uid: i64,
  workspace_id: Uuid,
) -> Result<SpacesResponseDto, AppError> {
  let folder = state.ws_server.get_folder(workspace_id).await?;
  let mut space_views = folder.get_views_belong_to(&workspace_id.to_string(), uid);
  for private_section in folder.get_my_private_sections(uid) {
    if let Some(pv) = folder.get_view(&private_section.id, uid) {
      if check_if_view_is_space(&pv) && !space_views.iter().any(|v| v.id == pv.id) {
        space_views.push(pv);
      }
    }
  }

  // Filter views that represent spaces
  let space_views: Vec<_> = space_views
    .into_iter()
    .filter(|v| check_if_view_is_space(v))
    .collect();

  // Load existing permissions
  let perm_rows = sqlx::query(
    r#"
    SELECT space_id, visibility, owner_access_level, member_default_access_level,
           everyone_else_access_level, invite_policy, sidebar_edit_policy,
           invite_link_enabled, security
    FROM af_space_permission
    WHERE workspace_id = $1
    "#,
  )
  .bind(workspace_id)
  .fetch_all(&state.pg_pool)
  .await?;

  let mut perm_map = HashMap::new();
  for r in perm_rows {
    let s_id: Uuid = r.try_get("space_id")?;
    let visibility: String = r.try_get("visibility")?;
    let owner_access: i32 = r.try_get("owner_access_level")?;
    let member_default: Option<i32> = r.try_get("member_default_access_level")?;
    let everyone_else: Option<i32> = r.try_get("everyone_else_access_level")?;
    let invite_policy: String = r.try_get("invite_policy")?;
    let sidebar_policy: String = r.try_get("sidebar_edit_policy")?;
    let invite_link: bool = r.try_get("invite_link_enabled")?;
    let security_val: serde_json::Value = r.try_get("security")?;
    let security: Option<SpaceSecuritySettingsDto> = serde_json::from_value(security_val).ok();

    perm_map.insert(
      s_id,
      SpacePermissionSettingsDto {
        visibility,
        owner_access_level: access_level_from_i32(owner_access),
        member_default_access_level: member_default.map(access_level_from_i32),
        everyone_else_access_level: everyone_else.map(access_level_from_i32),
        invite_policy,
        sidebar_edit_policy: sidebar_policy,
        invite_link_enabled: invite_link,
        security,
      },
    );
  }

  // Load member counts and caller membership
  let member_rows = sqlx::query(
    r#"
    SELECT space_id, uid, role, access_level
    FROM af_space_member
    WHERE workspace_id = $1
    "#,
  )
  .bind(workspace_id)
  .fetch_all(&state.pg_pool)
  .await?;

  let mut member_count_map: HashMap<Uuid, i64> = HashMap::new();
  let mut user_membership_map: HashMap<Uuid, (String, i32)> = HashMap::new();

  for r in member_rows {
    let s_id: Uuid = r.try_get("space_id")?;
    let m_uid: i64 = r.try_get("uid")?;
    let role: String = r.try_get("role")?;
    let access_level: i32 = r.try_get("access_level")?;

    *member_count_map.entry(s_id).or_insert(0) += 1;
    if m_uid == uid {
      user_membership_map.insert(s_id, (role, access_level));
    }
  }

  let mut spaces = Vec::new();
  for view in space_views {
    let s_id = match Uuid::parse_str(&view.id) {
      Ok(id) => id,
      Err(_) => continue,
    };

    let permission = perm_map.remove(&s_id).unwrap_or_else(|| {
      let is_private = check_if_space_is_private(&folder, &view.id);
      SpacePermissionSettingsDto {
        visibility: if is_private { "private".to_string() } else { "public".to_string() },
        owner_access_level: AFAccessLevel::FullAccess,
        member_default_access_level: Some(AFAccessLevel::ReadAndWrite),
        everyone_else_access_level: None,
        invite_policy: "members_and_owners".to_string(),
        sidebar_edit_policy: "members_and_owners".to_string(),
        invite_link_enabled: false,
        security: Some(SpaceSecuritySettingsDto::default()),
      }
    });

    let explicit_member_count = member_count_map.get(&s_id).copied().unwrap_or(0);
    let membership = user_membership_map.get(&s_id);
    let is_explicit_member = membership.is_some();
    let can_leave = match membership {
      Some((role, _)) => role != "owner",
      None => false,
    };

    let current_user_access_level = match membership {
      Some((_, lvl)) => Some(access_level_from_i32(*lvl)),
      None => match permission.visibility.as_str() {
        "public" => permission.member_default_access_level,
        "private" => None,
        "custom" => permission.everyone_else_access_level,
        _ => permission.member_default_access_level,
      },
    };

    spaces.push(SpaceListItemDto {
      space_id: s_id,
      name: view.name.clone(),
      permission,
      current_user_access_level,
      explicit_member_count,
      is_explicit_member,
      can_leave,
    });
  }

  Ok(SpacesResponseDto { spaces })
}

/// Retrieves permissions and caller capability flags for a specific space.
pub async fn get_space_permission(
  state: &AppState,
  uid: i64,
  workspace_id: Uuid,
  space_id: Uuid,
) -> Result<SpacePermissionResponseDto, AppError> {
  let row = sqlx::query(
    r#"
    SELECT visibility, owner_access_level, member_default_access_level,
           everyone_else_access_level, invite_policy, sidebar_edit_policy,
           invite_link_enabled, security
    FROM af_space_permission
    WHERE space_id = $1 AND workspace_id = $2
    "#,
  )
  .bind(space_id)
  .bind(workspace_id)
  .fetch_optional(&state.pg_pool)
  .await?;

  let permission = match row {
    Some(r) => {
      let visibility: String = r.try_get("visibility")?;
      let owner_access: i32 = r.try_get("owner_access_level")?;
      let member_default: Option<i32> = r.try_get("member_default_access_level")?;
      let everyone_else: Option<i32> = r.try_get("everyone_else_access_level")?;
      let invite_policy: String = r.try_get("invite_policy")?;
      let sidebar_policy: String = r.try_get("sidebar_edit_policy")?;
      let invite_link: bool = r.try_get("invite_link_enabled")?;
      let security_val: serde_json::Value = r.try_get("security")?;
      let security: Option<SpaceSecuritySettingsDto> = serde_json::from_value(security_val).ok();

      SpacePermissionSettingsDto {
        visibility,
        owner_access_level: access_level_from_i32(owner_access),
        member_default_access_level: member_default.map(access_level_from_i32),
        everyone_else_access_level: everyone_else.map(access_level_from_i32),
        invite_policy,
        sidebar_edit_policy: sidebar_policy,
        invite_link_enabled: invite_link,
        security,
      }
    },
    None => {
      // Check if space view exists in folder collab
      let folder = state.ws_server.get_folder(workspace_id).await?;
      let view = folder.get_view(&space_id.to_string(), uid);
      if view.is_none() {
        return Err(AppError::RecordNotFound("Space not found".to_string()));
      }
      let is_private = check_if_space_is_private(&folder, &space_id.to_string());
      SpacePermissionSettingsDto {
        visibility: if is_private { "private".to_string() } else { "public".to_string() },
        owner_access_level: AFAccessLevel::FullAccess,
        member_default_access_level: Some(AFAccessLevel::ReadAndWrite),
        everyone_else_access_level: None,
        invite_policy: "members_and_owners".to_string(),
        sidebar_edit_policy: "members_and_owners".to_string(),
        invite_link_enabled: false,
        security: Some(SpaceSecuritySettingsDto::default()),
      }
    },
  };

  // Caller membership
  let member_row = sqlx::query(
    r#"
    SELECT role, access_level
    FROM af_space_member
    WHERE space_id = $1 AND uid = $2
    "#,
  )
  .bind(space_id)
  .bind(uid)
  .fetch_optional(&state.pg_pool)
  .await?;

  let (is_owner, current_user_access_level) = match member_row {
    Some(r) => {
      let role: String = r.try_get("role")?;
      let lvl: i32 = r.try_get("access_level")?;
      (role == "owner", Some(access_level_from_i32(lvl)))
    },
    None => {
      let lvl = match permission.visibility.as_str() {
        "public" => permission.member_default_access_level,
        "private" => None,
        "custom" => permission.everyone_else_access_level,
        _ => permission.member_default_access_level,
      };
      (false, lvl)
    },
  };

  // Check workspace role for workspace owner override
  let is_workspace_owner = sqlx::query_scalar::<_, i64>(
    r#"
    SELECT COUNT(*) FROM af_workspace WHERE workspace_id = $1 AND owner_uid = $2
    "#,
  )
  .bind(workspace_id)
  .bind(uid)
  .fetch_one(&state.pg_pool)
  .await
  .unwrap_or(0)
    > 0;

  let can_manage = is_owner || is_workspace_owner;

  let explicit_count = sqlx::query_scalar::<_, i64>(
    r#"
    SELECT COUNT(*) FROM af_space_member WHERE space_id = $1
    "#,
  )
  .bind(space_id)
  .fetch_one(&state.pg_pool)
  .await
  .unwrap_or(0);

  Ok(SpacePermissionResponseDto {
    space_id,
    permission,
    current_user_access_level,
    can_manage_space: can_manage,
    can_manage_members: can_manage,
    can_invite_members: can_manage,
    can_edit_sidebar: can_manage,
    explicit_member_count: explicit_count,
  })
}

/// Updates structured space permissions and synchronizes collaborative view privacy.
pub async fn update_space_permission(
  state: &AppState,
  user: RealtimeUser,
  workspace_id: Uuid,
  space_id: Uuid,
  permission: SpacePermissionSettingsDto,
) -> Result<SpacePermissionResponseDto, AppError> {
  let security_json = serde_json::to_value(&permission.security).unwrap_or_default();

  sqlx::query(
    r#"
    INSERT INTO af_space_permission (
      space_id, workspace_id, visibility, owner_access_level,
      member_default_access_level, everyone_else_access_level,
      invite_policy, sidebar_edit_policy, invite_link_enabled, security
    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
    ON CONFLICT (space_id) DO UPDATE SET
      visibility = EXCLUDED.visibility,
      owner_access_level = EXCLUDED.owner_access_level,
      member_default_access_level = EXCLUDED.member_default_access_level,
      everyone_else_access_level = EXCLUDED.everyone_else_access_level,
      invite_policy = EXCLUDED.invite_policy,
      sidebar_edit_policy = EXCLUDED.sidebar_edit_policy,
      invite_link_enabled = EXCLUDED.invite_link_enabled,
      security = EXCLUDED.security,
      updated_at = CURRENT_TIMESTAMP
    "#,
  )
  .bind(space_id)
  .bind(workspace_id)
  .bind(&permission.visibility)
  .bind(permission.owner_access_level as i32)
  .bind(permission.member_default_access_level.map(|l| l as i32))
  .bind(permission.everyone_else_access_level.map(|l| l as i32))
  .bind(&permission.invite_policy)
  .bind(&permission.sidebar_edit_policy)
  .bind(permission.invite_link_enabled)
  .bind(security_json)
  .execute(&state.pg_pool)
  .await?;

  // Synchronize view privacy in collab folder if visibility changed
  let is_private = match permission.visibility.as_str() {
    "private" => true,
    "custom" if permission.everyone_else_access_level.is_none() => true,
    _ => false,
  };
  let legacy_perm = if is_private {
    shared_entity::dto::workspace_dto::SpacePermission::Private
  } else {
    shared_entity::dto::workspace_dto::SpacePermission::PublicToAll
  };

  let mut folder = state.ws_server.get_folder(workspace_id).await?;
  let (name, icon, color) = if let Some(view) = folder.get_view(&space_id.to_string(), user.uid) {
    let extra = view.extra.as_deref().map(parse_extra_field_as_json);
    let icon = extra
      .as_ref()
      .and_then(|e| e.get("space_icon"))
      .and_then(|v| v.as_str())
      .unwrap_or("")
      .to_string();
    let color = extra
      .as_ref()
      .and_then(|e| e.get("space_icon_color"))
      .and_then(|v| v.as_str())
      .unwrap_or("")
      .to_string();
    (view.name.clone(), icon, color)
  } else {
    ("".to_string(), "".to_string(), "".to_string())
  };

  let _ = update_space(
    state,
    user.clone(),
    workspace_id,
    &space_id.to_string(),
    &legacy_perm,
    &name,
    &icon,
    &color,
  )
  .await;

  get_space_permission(state, user.uid, workspace_id, space_id).await
}

/// Updates space metadata and/or permission.
pub async fn update_structured_space(
  state: &AppState,
  user: RealtimeUser,
  workspace_id: Uuid,
  space_id: Uuid,
  payload: UpdateStructuredSpaceParams,
) -> Result<Space, AppError> {
  if let Some(perm) = payload.permission {
    let _ = update_space_permission(state, user.clone(), workspace_id, space_id, perm).await?;
  }

  let mut folder = state.ws_server.get_folder(workspace_id).await?;
  if let Some(view) = folder.get_view(&space_id.to_string(), user.uid) {
    let name = payload.name.unwrap_or_else(|| view.name.clone());
    let icon = payload.space_icon.unwrap_or_default();
    let color = payload.space_icon_color.unwrap_or_default();
    let is_private = check_if_space_is_private(&folder, &space_id.to_string());
    let legacy_perm = if is_private {
      shared_entity::dto::workspace_dto::SpacePermission::Private
    } else {
      shared_entity::dto::workspace_dto::SpacePermission::PublicToAll
    };
    let _ = update_space(
      state,
      user,
      workspace_id,
      &space_id.to_string(),
      &legacy_perm,
      &name,
      &icon,
      &color,
    )
    .await?;
  }

  Ok(Space { view_id: space_id })
}

/// Lists all explicit members assigned to a space.
pub async fn get_space_members(
  state: &AppState,
  workspace_id: Uuid,
  space_id: Uuid,
) -> Result<SpaceMembersResponseDto, AppError> {
  let rows = sqlx::query(
    r#"
    SELECT
      m.uid,
      m.role,
      m.access_level,
      u.name,
      u.email,
      r.name AS workspace_role
    FROM af_space_member m
    JOIN af_user u ON m.uid = u.uid
    LEFT JOIN af_workspace_member wm ON wm.uid = m.uid AND wm.workspace_id = m.workspace_id
    LEFT JOIN af_roles r ON wm.role_id = r.id
    WHERE m.space_id = $1 AND m.workspace_id = $2
    ORDER BY m.created_at ASC
    "#,
  )
  .bind(space_id)
  .bind(workspace_id)
  .fetch_all(&state.pg_pool)
  .await?;

  let mut members = Vec::new();
  for r in rows {
    let uid: i64 = r.try_get("uid")?;
    let role: String = r.try_get("role")?;
    let access_level: i32 = r.try_get("access_level")?;
    let name: Option<String> = r.try_get("name")?;
    let email: Option<String> = r.try_get("email")?;
    let workspace_role: Option<String> = r.try_get("workspace_role")?;

    members.push(SpaceMemberDto {
      uid,
      email,
      name,
      role,
      access_level: access_level_from_i32(access_level),
      source: "manual".to_string(),
      workspace_role,
    });
  }

  Ok(SpaceMembersResponseDto {
    members,
    groups: vec![],
  })
}

/// Adds a member to a space and updates realtime access policy.
pub async fn add_space_member(
  state: &AppState,
  workspace_id: Uuid,
  space_id: Uuid,
  payload: AddSpaceMemberParams,
) -> Result<SpaceMemberDto, AppError> {
  // Validate target is a member of the workspace
  let in_workspace = sqlx::query_scalar::<_, i64>(
    r#"
    SELECT COUNT(*) FROM af_workspace_member WHERE workspace_id = $1 AND uid = $2
    "#,
  )
  .bind(workspace_id)
  .bind(payload.uid)
  .fetch_one(&state.pg_pool)
  .await
  .unwrap_or(0)
    > 0;

  if !in_workspace {
    return Err(AppError::RecordNotFound(
      "User is not a member of the workspace".to_string(),
    ));
  }

  sqlx::query(
    r#"
    INSERT INTO af_space_member (space_id, workspace_id, uid, role, access_level)
    VALUES ($1, $2, $3, $4, $5)
    ON CONFLICT (space_id, uid) DO UPDATE SET
      role = EXCLUDED.role,
      access_level = EXCLUDED.access_level,
      updated_at = CURRENT_TIMESTAMP
    "#,
  )
  .bind(space_id)
  .bind(workspace_id)
  .bind(payload.uid)
  .bind(&payload.role)
  .bind(payload.access_level as i32)
  .execute(&state.pg_pool)
  .await?;

  let _ = state
    .collab_access_control
    .update_access_level_policy(&payload.uid, &space_id, payload.access_level)
    .await;

  // Retrieve member details
  let row = sqlx::query(
    r#"
    SELECT u.name, u.email, r.name AS workspace_role
    FROM af_user u
    LEFT JOIN af_workspace_member wm ON wm.uid = u.uid AND wm.workspace_id = $1
    LEFT JOIN af_roles r ON wm.role_id = r.id
    WHERE u.uid = $2
    "#,
  )
  .bind(workspace_id)
  .bind(payload.uid)
  .fetch_one(&state.pg_pool)
  .await?;

  let name: Option<String> = row.try_get("name")?;
  let email: Option<String> = row.try_get("email")?;
  let workspace_role: Option<String> = row.try_get("workspace_role")?;

  Ok(SpaceMemberDto {
    uid: payload.uid,
    email,
    name,
    role: payload.role,
    access_level: payload.access_level,
    source: "manual".to_string(),
    workspace_role,
  })
}

/// Updates an existing space member's role or access level.
pub async fn update_space_member(
  state: &AppState,
  workspace_id: Uuid,
  space_id: Uuid,
  target_uid: i64,
  payload: UpdateSpaceMemberParams,
) -> Result<SpaceMemberDto, AppError> {
  if let Some(role) = &payload.role {
    sqlx::query(
      r#"
      UPDATE af_space_member SET role = $1, updated_at = CURRENT_TIMESTAMP
      WHERE space_id = $2 AND uid = $3 AND workspace_id = $4
      "#,
    )
    .bind(role)
    .bind(space_id)
    .bind(target_uid)
    .bind(workspace_id)
    .execute(&state.pg_pool)
    .await?;
  }

  if let Some(level) = payload.access_level {
    sqlx::query(
      r#"
      UPDATE af_space_member SET access_level = $1, updated_at = CURRENT_TIMESTAMP
      WHERE space_id = $2 AND uid = $3 AND workspace_id = $4
      "#,
    )
    .bind(level as i32)
    .bind(space_id)
    .bind(target_uid)
    .bind(workspace_id)
    .execute(&state.pg_pool)
    .await?;

    let _ = state
      .collab_access_control
      .update_access_level_policy(&target_uid, &space_id, level)
      .await;
  }

  let row = sqlx::query(
    r#"
    SELECT m.role, m.access_level, u.name, u.email, r.name AS workspace_role
    FROM af_space_member m
    JOIN af_user u ON m.uid = u.uid
    LEFT JOIN af_workspace_member wm ON wm.uid = m.uid AND wm.workspace_id = m.workspace_id
    LEFT JOIN af_roles r ON wm.role_id = r.id
    WHERE m.space_id = $1 AND m.uid = $2 AND m.workspace_id = $3
    "#,
  )
  .bind(space_id)
  .bind(target_uid)
  .bind(workspace_id)
  .fetch_one(&state.pg_pool)
  .await?;

  let role: String = row.try_get("role")?;
  let access_level: i32 = row.try_get("access_level")?;
  let name: Option<String> = row.try_get("name")?;
  let email: Option<String> = row.try_get("email")?;
  let workspace_role: Option<String> = row.try_get("workspace_role")?;

  Ok(SpaceMemberDto {
    uid: target_uid,
    email,
    name,
    role,
    access_level: access_level_from_i32(access_level),
    source: "manual".to_string(),
    workspace_role,
  })
}

/// Removes a member from a space and removes their Casbin policy.
pub async fn remove_space_member(
  state: &AppState,
  workspace_id: Uuid,
  space_id: Uuid,
  target_uid: i64,
) -> Result<(), AppError> {
  sqlx::query(
    r#"
    DELETE FROM af_space_member
    WHERE space_id = $1 AND workspace_id = $2 AND uid = $3
    "#,
  )
  .bind(space_id)
  .bind(workspace_id)
  .bind(target_uid)
  .execute(&state.pg_pool)
  .await?;

  let _ = state
    .collab_access_control
    .remove_access_level(&target_uid, &space_id)
    .await;

  Ok(())
}

fn access_level_from_i32(val: i32) -> AFAccessLevel {
  match val {
    10 => AFAccessLevel::ReadOnly,
    20 => AFAccessLevel::ReadAndComment,
    30 => AFAccessLevel::ReadAndWrite,
    50 => AFAccessLevel::FullAccess,
    _ => AFAccessLevel::ReadAndWrite,
  }
}
