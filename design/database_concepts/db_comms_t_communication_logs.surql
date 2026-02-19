USE NS project_universe;
USE DB communications;

DEFINE TABLE communication_logs SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD log_id          ON TABLE communication_logs TYPE string;
DEFINE FIELD session_id      ON TABLE communication_logs TYPE string;
DEFINE FIELD timestamp       ON TABLE communication_logs TYPE datetime;
DEFINE FIELD event           ON TABLE communication_logs TYPE string;
DEFINE FIELD details         ON TABLE communication_logs TYPE string;

DEFINE INDEX unique_communication_log_id ON TABLE communication_logs COLUMNS log_id UNIQUE;

-- Example communication logs
CREATE communication_logs:log_001 SET
  log_id = "log_001",
  session_id = "session_001",
  timestamp = "2023-03-22T10:05:00Z",
  event = "Call Started",
  details = "VoIP call initiated by user_123.";

CREATE communication_logs:log_002 SET
  log_id = "log_002",
  session_id = "session_001",
  timestamp = "2023-03-22T10:30:00Z",
  event = "Call Ended",
  details = "VoIP call ended by user_456.";
