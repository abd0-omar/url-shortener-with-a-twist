CREATE TABLE links_tokens(
   link_token TEXT NOT NULL,
   PRIMARY KEY (link_token),
   recepient_id uuid NOT NULL REFERENCES link_recipients (id),
   link_id TEXT NOT NULL REFERENCES links(id),
   -- pending, allowed
   status TEXT NOT NULL,
   -- allow null for non expiring links
   expiration_date timestamptz NULL
);