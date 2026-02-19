USE NS project_universe;
USE DB game;

DEFINE TABLE sessions SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD session_id   ON TABLE sessions TYPE string;
DEFINE FIELD person_id    ON TABLE sessions TYPE record(people);
DEFINE FIELD start_time   ON TABLE sessions TYPE datetime;
DEFINE FIELD end_time     ON TABLE sessions TYPE datetime;
DEFINE FIELD active       ON TABLE sessions TYPE bool;

DEFINE INDEX unique_session_id ON TABLE sessions COLUMNS session_id UNIQUE;

-- Example sessions
CREATE sessions:session_001 SET
  session_id = "session_001",
  person_id = people:john_doe,
  start_time = time::now(),
  end_time = time::now() + time::duration("2h"),
  active = true;

CREATE sessions:session_002 SET
  session_id = "session_002",
  person_id = people:jane_smith,
  start_time = time::now(),
  end_time = time::now() + time::duration("1h"),
  active = true; 