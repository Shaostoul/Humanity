USE NS project_universe;
USE DB communications;

DEFINE TABLE private_messages SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD message_id      ON TABLE private_messages TYPE string;
DEFINE FIELD conversation_id ON TABLE private_messages TYPE string;
DEFINE FIELD sender          ON TABLE private_messages TYPE string;
DEFINE FIELD timestamp       ON TABLE private_messages TYPE datetime;
DEFINE FIELD content         ON TABLE private_messages TYPE string;

DEFINE INDEX unique_private_message_id ON TABLE private_messages COLUMNS message_id UNIQUE;

-- Example private messages
CREATE private_messages:message_001 SET
  message_id = "message_001",
  conversation_id = "conversation_001",
  sender = "user_123",
  timestamp = "2023-03-22T10:05:00Z",
  content = "Can we discuss the project details later?";

CREATE private_messages:message_002 SET
  message_id = "message_002",
  conversation_id = "conversation_001",
  sender = "user_456",
  timestamp = "2023-03-22T10:10:00Z",
  content = "Sure, let's schedule a time for that.";
