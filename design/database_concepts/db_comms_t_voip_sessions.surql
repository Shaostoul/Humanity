USE NS project_universe;
USE DB communications;

DEFINE TABLE voip_sessions SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD session_id      ON TABLE voip_sessions TYPE string;
DEFINE FIELD caller          ON TABLE voip_sessions TYPE string;
DEFINE FIELD receiver        ON TABLE voip_sessions TYPE string;
DEFINE FIELD start_time      ON TABLE voip_sessions TYPE datetime;
DEFINE FIELD end_time        ON TABLE voip_sessions TYPE datetime;
DEFINE FIELD status          ON TABLE voip_sessions TYPE string;
DEFINE FIELD quality_metrics ON TABLE voip_sessions TYPE array;

DEFINE INDEX unique_voip_session_id ON TABLE voip_sessions COLUMNS session_id UNIQUE;

-- Example VoIP sessions
CREATE voip_sessions:session_001 SET
  session_id = "session_001",
  caller = "user_123",
  receiver = "user_456",
  start_time = "2023-03-22T10:00:00Z",
  end_time = "2023-03-22T10:30:00Z",
  status = "Completed",
  quality_metrics = ["Good", "No Lag"];

CREATE voip_sessions:session_002 SET
  session_id = "session_002",
  caller = "user_789",
  receiver = "user_321",
  start_time = "2023-03-22T11:00:00Z",
  end_time = "2023-03-22T11:15:00Z",
  status = "Dropped",
  quality_metrics = ["Poor", "High Latency"];
