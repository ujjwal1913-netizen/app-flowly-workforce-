-- stores public form definitions and configuration
CREATE TABLE IF NOT EXISTS af_public_form (
  form_id             UUID NOT NULL DEFAULT gen_random_uuid(),
  workspace_id        UUID NOT NULL REFERENCES af_workspace(workspace_id) ON DELETE CASCADE,
  view_id             UUID NOT NULL,
  token               TEXT NOT NULL UNIQUE,
  schema              JSONB NOT NULL DEFAULT '{}'::jsonb,
  is_active           BOOLEAN NOT NULL DEFAULT TRUE,
  auth_required       BOOLEAN NOT NULL DEFAULT FALSE,
  created_by          BIGINT REFERENCES af_user(uid) ON DELETE SET NULL,
  created_at          TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at          TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,

  PRIMARY KEY         (form_id)
);

CREATE INDEX IF NOT EXISTS idx_token_on_af_public_form ON af_public_form(token);
CREATE INDEX IF NOT EXISTS idx_workspace_view_on_af_public_form ON af_public_form(workspace_id, view_id);

-- stores form submissions and status
CREATE TABLE IF NOT EXISTS af_form_submission (
  submission_id       UUID NOT NULL DEFAULT gen_random_uuid(),
  form_id             UUID NOT NULL REFERENCES af_public_form(form_id) ON DELETE CASCADE,
  token               TEXT NOT NULL,
  idempotency_key     TEXT,
  status              TEXT NOT NULL DEFAULT 'accepted',
  answers             JSONB NOT NULL DEFAULT '{}'::jsonb,
  submitted_by        BIGINT REFERENCES af_user(uid) ON DELETE SET NULL,
  client_ip           TEXT,
  created_at          TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,

  PRIMARY KEY         (submission_id)
);

CREATE INDEX IF NOT EXISTS idx_form_token_on_af_form_submission ON af_form_submission(token);
CREATE INDEX IF NOT EXISTS idx_idempotency_on_af_form_submission ON af_form_submission(form_id, idempotency_key);
