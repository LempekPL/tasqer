CREATE TABLE users
(
    id       serial  NOT NULL UNIQUE,
    login    text    NOT NULL UNIQUE,
    password text    NOT NULL,
    admin    boolean NOT NULL DEFAULT FALSE,
    PRIMARY KEY (id)
);

CREATE TABLE groups
(
    id   serial NOT NULL UNIQUE,
    name text   NOT NULL,
    PRIMARY KEY (id)
);

CREATE TABLE user_group
(
    id          serial  NOT NULL UNIQUE,
    user_id     integer NOT NULL,
    group_id    integer NOT NULL,
    permissions text    NOT NULL,
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
    id          uuid    NOT NULL DEFAULT uuidv7() UNIQUE,
    owner       integer NOT NULL,
    title       text    NOT NULL,
    description text,
    PRIMARY KEY (id),
    FOREIGN KEY (user_id)
        REFERENCES users (id) MATCH SIMPLE
        ON UPDATE CASCADE
        ON DELETE CASCADE,
);