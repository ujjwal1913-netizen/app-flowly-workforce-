use actix_web::web::{Data, Json, Path};
use actix_web::{web, HttpRequest, HttpResponse, Result, Scope};
use app_error::AppError;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::biz::authentication::jwt::OptionalUserUuid;
use crate::state::AppState;

pub fn public_form_scope() -> Scope {
  web::scope("/api/workspace/public-form")
    .service(web::resource("/{token}").route(web::get().to(get_public_form_handler)))
    .service(web::resource("/{token}/submit").route(web::post().to(submit_public_form_handler)))
    .service(
      web::resource("/{token}/submissions/{submission_id}")
        .route(web::get().to(get_form_submission_handler)),
    )
    .service(web::resource("/{token}/upload-url").route(web::post().to(get_form_upload_url_handler)))
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PublicFormResponse {
  Active(serde_json::Value),
  Closed { message: String },
  AuthRequired { login_url: String },
}

async fn get_public_form_handler(
  optional_user: OptionalUserUuid,
  state: Data<AppState>,
  path: Path<String>,
) -> Result<HttpResponse> {
  let token = path.into_inner();

  let row = sqlx::query(
    r#"
    SELECT form_id, is_active, auth_required, schema
    FROM af_public_form
    WHERE token = $1
    "#,
  )
  .bind(&token)
  .fetch_optional(&state.pg_pool)
  .await
  .map_err(AppError::from)?;

  if let Some(r) = row {
    let is_active: bool = r.try_get("is_active").unwrap_or(true);
    let auth_required: bool = r.try_get("auth_required").unwrap_or(false);
    let schema: serde_json::Value = r.try_get("schema").unwrap_or_else(|_| serde_json::json!({}));

    if !is_active {
      return Ok(HttpResponse::Ok().json(PublicFormResponse::Closed {
        message: "This form is no longer accepting responses.".to_string(),
      }));
    }

    if auth_required && optional_user.as_uuid().is_none() {
      return Ok(HttpResponse::Ok().json(PublicFormResponse::AuthRequired {
        login_url: "/login".to_string(),
      }));
    }

    let mut obj = match schema {
      serde_json::Value::Object(map) => map,
      _ => serde_json::Map::new(),
    };
    obj.insert("kind".to_string(), serde_json::Value::String("active".to_string()));
    obj.insert("form_id".to_string(), serde_json::Value::String(token));
    if !obj.contains_key("title") {
      obj.insert("title".to_string(), serde_json::Value::String("Form".to_string()));
    }
    if !obj.contains_key("questions") {
      obj.insert("questions".to_string(), serde_json::Value::Array(vec![]));
    }
    if !obj.contains_key("submit_label") {
      obj.insert("submit_label".to_string(), serde_json::Value::String("Submit".to_string()));
    }
    if !obj.contains_key("submit_color") {
      obj.insert("submit_color".to_string(), serde_json::Value::String("#00BCF0".to_string()));
    }
    if !obj.contains_key("confirmation_title") {
      obj.insert(
        "confirmation_title".to_string(),
        serde_json::Value::String("Thank you!".to_string()),
      );
    }
    if !obj.contains_key("confirmation_body") {
      obj.insert(
        "confirmation_body".to_string(),
        serde_json::Value::String("Your response has been recorded.".to_string()),
      );
    }
    if !obj.contains_key("allow_another_response") {
      obj.insert("allow_another_response".to_string(), serde_json::Value::Bool(true));
    }
    if !obj.contains_key("hide_branding") {
      obj.insert("hide_branding".to_string(), serde_json::Value::Bool(false));
    }
    if !obj.contains_key("tier") {
      obj.insert("tier".to_string(), serde_json::Value::String("public".to_string()));
    }
    if !obj.contains_key("anonymous") {
      obj.insert("anonymous".to_string(), serde_json::Value::Bool(true));
    }

    return Ok(HttpResponse::Ok().json(obj));
  }

  // Synthesize default active form if token is a valid view uuid
  if let Ok(view_id) = Uuid::parse_str(&token) {
    let view_exists = sqlx::query_scalar!(
      r#"SELECT EXISTS(SELECT 1 FROM af_view WHERE view_id = $1) AS "exists!""#,
      view_id
    )
    .fetch_one(&state.pg_pool)
    .await
    .unwrap_or(true);

    if view_exists {
      let default_schema = serde_json::json!({
        "kind": "active",
        "form_id": token,
        "tier": "public",
        "anonymous": true,
        "title": "Form",
        "questions": [],
        "submit_label": "Submit",
        "submit_color": "#00BCF0",
        "confirmation_title": "Thank you!",
        "confirmation_body": "Your response has been recorded.",
        "allow_another_response": true,
        "hide_branding": false
      });
      return Ok(HttpResponse::Ok().json(default_schema));
    }
  }

  Ok(HttpResponse::NotFound().json(serde_json::json!({
    "code": 404,
    "message": "Form not found"
  })))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FormSubmissionPayload {
  pub answers: serde_json::Value,
}

async fn submit_public_form_handler(
  optional_user: OptionalUserUuid,
  state: Data<AppState>,
  req: HttpRequest,
  path: Path<String>,
  payload: Json<FormSubmissionPayload>,
) -> Result<HttpResponse> {
  let token = path.into_inner();
  let idempotency_key = req
    .headers()
    .get("Idempotency-Key")
    .and_then(|h| h.to_str().ok())
    .map(|s| s.to_string());

  let submission_id = Uuid::new_v4();
  let submitted_by = if let Some(uuid) = optional_user.as_uuid() {
    state.user_cache.get_user_uid(&uuid).await.ok()
  } else {
    None
  };

  let client_ip = req
    .connection_info()
    .realip_remote_addr()
    .map(|ip| ip.to_string());

  let form_row = sqlx::query("SELECT form_id FROM af_public_form WHERE token = $1")
    .bind(&token)
    .fetch_optional(&state.pg_pool)
    .await
    .map_err(AppError::from)?;

  let form_id = if let Some(r) = form_row {
    r.try_get("form_id").unwrap_or_default()
  } else {
    let dummy_id = Uuid::new_v4();
    let view_uuid = Uuid::parse_str(&token).unwrap_or(dummy_id);
    let ws_uuid = sqlx::query_scalar!(
      "SELECT workspace_id FROM af_view WHERE view_id = $1",
      view_uuid
    )
    .fetch_optional(&state.pg_pool)
    .await
    .ok()
    .flatten()
    .unwrap_or(dummy_id);

    sqlx::query(
      r#"
      INSERT INTO af_public_form (form_id, workspace_id, view_id, token)
      VALUES ($1, $2, $3, $4)
      ON CONFLICT (token) DO UPDATE SET updated_at = CURRENT_TIMESTAMP
      RETURNING form_id
      "#,
    )
    .bind(dummy_id)
    .bind(ws_uuid)
    .bind(view_uuid)
    .bind(&token)
    .fetch_one(&state.pg_pool)
    .await
    .map(|r| r.try_get("form_id").unwrap_or(dummy_id))
    .unwrap_or(dummy_id)
  };

  sqlx::query(
    r#"
    INSERT INTO af_form_submission (
      submission_id, form_id, token, idempotency_key, status, answers, submitted_by, client_ip
    )
    VALUES ($1, $2, $3, $4, 'accepted', $5, $6, $7)
    "#,
  )
  .bind(submission_id)
  .bind(form_id)
  .bind(&token)
  .bind(idempotency_key)
  .bind(&payload.answers)
  .bind(submitted_by)
  .bind(client_ip)
  .execute(&state.pg_pool)
  .await
  .map_err(AppError::from)?;

  Ok(HttpResponse::Ok().json(serde_json::json!({
    "kind": "submitted",
    "submission_id": submission_id.to_string(),
    "status": "accepted"
  })))
}

async fn get_form_submission_handler(
  state: Data<AppState>,
  path: Path<(String, Uuid)>,
) -> Result<HttpResponse> {
  let (token, submission_id) = path.into_inner();

  let row = sqlx::query(
    "SELECT status FROM af_form_submission WHERE token = $1 AND submission_id = $2",
  )
  .bind(&token)
  .bind(submission_id)
  .fetch_optional(&state.pg_pool)
  .await
  .map_err(AppError::from)?;

  let status: String = row
    .and_then(|r| r.try_get("status").ok())
    .unwrap_or_else(|| "accepted".to_string());

  Ok(HttpResponse::Ok().json(serde_json::json!({
    "submission_id": submission_id.to_string(),
    "status": status
  })))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PublicFormUploadUrlRequest {
  pub file_name: String,
  pub content_length: usize,
  pub content_type: Option<String>,
  pub upload_protocol: Option<String>,
}

async fn get_form_upload_url_handler(
  path: Path<String>,
  payload: Json<PublicFormUploadUrlRequest>,
) -> Result<HttpResponse> {
  let _token = path.into_inner();
  let file_id = Uuid::new_v4().to_string();
  let content_type = payload
    .content_type
    .clone()
    .unwrap_or_else(|| "application/octet-stream".to_string());

  let upload_url = format!("/api/file_storage/public/{}", file_id);
  let download_url = format!("/api/file_storage/public/{}", file_id);

  Ok(HttpResponse::Ok().json(serde_json::json!({
    "file_id": file_id,
    "upload_url": upload_url,
    "upload_content_type": content_type,
    "download_url": download_url,
    "expires_in_secs": 3600,
    "upload_protocol": "create_only_v2",
    "upload_if_none_match": "*"
  })))
}
