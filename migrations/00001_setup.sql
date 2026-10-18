CREATE TABLE users
(
    id         uuid        NOT NULL DEFAULT uuidv7() UNIQUE,
    login      varchar(32) NOT NULL UNIQUE,
    password   text        NOT NULL,
    username   text        NOT NULL,
    admin      boolean     NOT NULL DEFAULT FALSE,
    private    boolean     NOT NULL DEFAULT FALSE,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (id)
);

CREATE TABLE sessions
(
    id         uuid        NOT NULL DEFAULT uuidv7() UNIQUE,
    user_id    uuid        NOT NULL,
    token      text        NOT NULL UNIQUE,
    expires_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (id),
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE
);

CREATE TABLE groups
(
    id         uuid        NOT NULL DEFAULT uuidv7() UNIQUE,
    name       text        NOT NULL,
    public     boolean     NOT NULL DEFAULT FALSE,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (id)
);

CREATE TABLE user_group
(
    id          uuid        NOT NULL DEFAULT uuidv7() UNIQUE,
    user_id     uuid        NOT NULL,
    group_id    uuid        NOT NULL,
    permissions text        NOT NULL CHECK (permissions IN ('owner', 'editor', 'viewer')),
    created_at  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (id),
    FOREIGN KEY (user_id)
        REFERENCES users (id) MATCH SIMPLE
        ON UPDATE CASCADE
        ON DELETE CASCADE,
    FOREIGN KEY (group_id)
        REFERENCES groups (id) MATCH SIMPLE
        ON UPDATE CASCADE
        ON DELETE CASCADE
);

CREATE TABLE task
(
    id                uuid        NOT NULL DEFAULT uuidv7() UNIQUE,
    owner_id          uuid        NOT NULL,
    group_id          uuid,
    title             text        NOT NULL,
    description       text,
    deadline          timestamptz,
    deadline_notified boolean     NOT NULL DEFAULT false,
    auto_delete       boolean     NOT NULL DEFAULT false,
    created_at        timestamptz NOT NULL DEFAULT now(),
    deleted_at        timestamptz,
    PRIMARY KEY (id),
    FOREIGN KEY (owner_id)
        REFERENCES users (id) MATCH SIMPLE
        ON UPDATE CASCADE
        ON DELETE CASCADE,
    FOREIGN KEY (group_id)
        REFERENCES groups (id) MATCH SIMPLE
        ON UPDATE CASCADE
        ON DELETE CASCADE
);

CREATE TABLE task_audit_log
(
    id         uuid        NOT NULL DEFAULT uuidv7() UNIQUE,
    task_id    uuid        NOT NULL,
    user_id    uuid,
    action     text        NOT NULL,
    old_value  text,
    new_value  text,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (id),
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE SET NULL,
    FOREIGN KEY (task_id) REFERENCES task (id) ON DELETE CASCADE
);

CREATE INDEX idx_task_deadline_pending
    ON task (deadline)
    WHERE deleted_at IS NULL AND deadline_notified = false;

CREATE INDEX idx_sessions_expires_at
    ON sessions (expires_at);

CREATE INDEX idx_task_owner_id ON task (owner_id);
CREATE INDEX idx_task_group_id ON task (group_id);

CREATE INDEX idx_user_group_user_id ON user_group (user_id);
CREATE INDEX idx_user_group_group_id ON user_group (group_id);

CREATE INDEX idx_sessions_user_id ON sessions (user_id);

CREATE INDEX idx_task_audit_log_task_id ON task_audit_log (task_id);
CREATE INDEX idx_task_audit_log_user_id ON task_audit_log (user_id);

CREATE INDEX idx_task_active
    ON task (created_at)
    WHERE deleted_at IS NULL;

CREATE TABLE invite_codes
(
    id         uuid        NOT NULL DEFAULT uuidv7() UNIQUE,
    code       varchar(32) NOT NULL UNIQUE,
    created_by uuid        NOT NULL,
    used_by    uuid,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (id),
    FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE CASCADE,
    FOREIGN KEY (used_by) REFERENCES users (id) ON DELETE SET NULL
);