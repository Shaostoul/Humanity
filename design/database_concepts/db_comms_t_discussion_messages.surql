USE NS project_universe;
USE DB communications;

DEFINE TABLE discussion_messages SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD message_id      ON TABLE discussion_messages TYPE string;
DEFINE FIELD discussion_id   ON TABLE discussion_messages TYPE string;
DEFINE FIELD sender          ON TABLE discussion_messages TYPE string;
DEFINE FIELD timestamp       ON TABLE discussion_messages TYPE datetime;
DEFINE FIELD content         ON TABLE discussion_messages TYPE string;

DEFINE INDEX unique_discussion_message_id ON TABLE discussion_messages COLUMNS message_id UNIQUE;

-- Example discussion messages
CREATE discussion_messages:message_001 SET
  message_id = "message_001",
  discussion_id = "discussion_001",
  sender = "user_123",
  timestamp = "2023-03-22T10:05:00Z",
  content = "I think we should consider alternative solutions.";

CREATE discussion_messages:message_002 SET
  message_id = "message_002",
  discussion_id = "discussion_001",
  sender = "user_456",
  timestamp = "2023-03-22T10:10:00Z",
  content = "That's a good point, let's explore those options.";
