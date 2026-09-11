-- 1. Space Permissions Table
CREATE TABLE IF NOT EXISTS af_space_permission (
    space_id UUID NOT NULL PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES af_workspace(workspace_id) ON DELETE CASCADE,
    visibility VARCHAR(32) NOT NULL DEFAULT 'public',
    owner_access_level INT NOT NULL DEFAULT 50,
    member_default_access_level INT,
    everyone_else_access_level INT,
    invite_policy VARCHAR(32) NOT NULL DEFAULT 'members_and_owners',
    sidebar_edit_policy VARCHAR(32) NOT NULL DEFAULT 'members_and_owners',
    invite_link_enabled BOOLEAN NOT NULL DEFAULT false,
    security JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_af_space_permission_workspace ON af_space_permission (workspace_id);

-- 2. Space Members Table
CREATE TABLE IF NOT EXISTS af_space_member (
    space_id UUID NOT NULL REFERENCES af_space_permission(space_id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES af_workspace(workspace_id) ON DELETE CASCADE,
    uid BIGINT NOT NULL REFERENCES af_user(uid) ON DELETE CASCADE,
    role VARCHAR(32) NOT NULL DEFAULT 'member',
    access_level INT NOT NULL DEFAULT 30,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (space_id, uid)
);

CREATE INDEX IF NOT EXISTS idx_af_space_member_workspace_uid ON af_space_member (workspace_id, uid);
