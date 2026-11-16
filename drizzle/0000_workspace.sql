CREATE TABLE IF NOT EXISTS studio_workspace (
  user_id TEXT PRIMARY KEY NOT NULL,
  revision INTEGER NOT NULL,
  payload TEXT NOT NULL
);
