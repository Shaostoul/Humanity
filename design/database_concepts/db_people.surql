USE NS project_universe;
USE DB game;

DEFINE TABLE people SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD username        ON TABLE people TYPE string;
DEFINE FIELD email           ON TABLE people TYPE string;
DEFINE FIELD password_hash   ON TABLE people TYPE string;
DEFINE FIELD role            ON TABLE people TYPE string;
DEFINE FIELD created_at      ON TABLE people TYPE datetime;
DEFINE FIELD last_login      ON TABLE people TYPE datetime;

DEFINE INDEX unique_username ON TABLE people COLUMNS username UNIQUE;
DEFINE INDEX unique_email ON TABLE people COLUMNS email UNIQUE;

-- Example people
CREATE people:john_doe SET
  username = "john_doe",
  email = "john.doe@example.com",
  password_hash = "hashed_password_123",
  role = "player",
  created_at = time::now(),
  last_login = time::now();

CREATE people:jane_smith SET
  username = "jane_smith",
  email = "jane.smith@example.com",
  password_hash = "hashed_password_456",
  role = "admin",
  created_at = time::now(),
  last_login = time::now(); 