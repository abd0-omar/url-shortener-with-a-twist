CREATE TABLE links (
  id TEXT UNIQUE NOT NULL,
  PRIMARY KEY (id),
  target_url TEXT NOT NULL,
  created_at timestamptz NOT NULL
);
