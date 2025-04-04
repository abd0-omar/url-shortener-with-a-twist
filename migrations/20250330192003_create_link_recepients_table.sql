CREATE TABLE link_recipients(
   id uuid NOT NULL,
   PRIMARY KEY (id),
   email TEXT NOT NULL UNIQUE,
   name TEXT NOT NULL,
   received_link_at timestamptz NOT NULL
);