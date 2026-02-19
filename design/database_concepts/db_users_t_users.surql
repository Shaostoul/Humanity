USE NS project_universe;
USE DB game;

DEFINE TABLE users SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD username        ON TABLE users TYPE string;
DEFINE FIELD email           ON TABLE users TYPE string;
DEFINE FIELD password_hash   ON TABLE users TYPE string;
DEFINE FIELD role            ON TABLE users TYPE string;
DEFINE FIELD created_at      ON TABLE users TYPE datetime;
DEFINE FIELD last_login      ON TABLE users TYPE datetime;

DEFINE INDEX unique_username ON TABLE users COLUMNS username UNIQUE;
DEFINE INDEX unique_email ON TABLE users COLUMNS email UNIQUE;

-- Example users
CREATE users:john_doe SET
  username = "john_doe",
  email = "john.doe@example.com",
  password_hash = "hashed_password_123",
  role = "player",
  created_at = time::now(),
  last_login = time::now();

CREATE users:jane_smith SET
  username = "jane_smith",
  email = "jane.smith@example.com",
  password_hash = "hashed_password_456",
  role = "admin",
  created_at = time::now(),
  last_login = time::now();
