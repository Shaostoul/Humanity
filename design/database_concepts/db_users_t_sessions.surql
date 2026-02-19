USE NS project_universe;
USE DB game;

DEFINE TABLE sessions SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD session_id      ON TABLE sessions TYPE string;
DEFINE FIELD user_id         ON TABLE sessions TYPE record(users);
DEFINE FIELD start_time      ON TABLE sessions TYPE datetime;
DEFINE FIELD end_time        ON TABLE sessions TYPE datetime;
DEFINE FIELD active          ON TABLE sessions TYPE bool;

DEFINE INDEX unique_session_id ON TABLE sessions COLUMNS session_id UNIQUE;

-- Example sessions
CREATE sessions:session_001 SET
  session_id = "session_001",
  user_id = users:john_doe,
  start_time = time::now() - 2h,
  end_time = time::now(),
  active = false;

CREATE sessions:session_002 SET
  session_id = "session_002",
  user_id = users:jane_smith,
  start_time = time::now() - 1h,
  end_time = time::now(),
  active = false;
