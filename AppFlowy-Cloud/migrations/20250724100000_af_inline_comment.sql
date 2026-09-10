-- stores the inline comments for documents
CREATE TABLE IF NOT EXISTS af_inline_comment (
  comment_id          UUID NOT NULL DEFAULT gen_random_uuid(),
  reply_comment_id    UUID REFERENCES af_inline_comment(comment_id) ON DELETE CASCADE,
  workspace_id        UUID NOT NULL REFERENCES af_workspace(workspace_id) ON DELETE CASCADE,
  view_id             UUID NOT NULL,
  block_id            TEXT,
  content             TEXT NOT NULL,
  is_resolved         BOOLEAN NOT NULL DEFAULT FALSE,
  is_deleted          BOOLEAN NOT NULL DEFAULT FALSE,
  created_by          BIGINT REFERENCES af_user(uid) ON DELETE SET NULL,
  created_at          TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at          TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,

  PRIMARY KEY         (comment_id)
);

CREATE INDEX IF NOT EXISTS idx_workspace_view_on_af_inline_comment ON af_inline_comment(workspace_id, view_id);
CREATE INDEX IF NOT EXISTS idx_view_id_on_af_inline_comment ON af_inline_comment(view_id);

-- stores reactions on inline comments
CREATE TABLE IF NOT EXISTS af_inline_comment_reaction (
  comment_id          UUID NOT NULL REFERENCES af_inline_comment(comment_id) ON DELETE CASCADE,
  reaction_type       TEXT NOT NULL,
  created_by          BIGINT NOT NULL REFERENCES af_user(uid) ON DELETE CASCADE,
  created_at          TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,

  PRIMARY KEY         (comment_id, reaction_type, created_by)
);
