USE NS project_universe;
USE DB communications;

DEFINE TABLE private_conversations SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD conversation_id ON TABLE private_conversations TYPE string;
DEFINE FIELD participants    ON TABLE private_conversations TYPE array;
DEFINE FIELD start_time      ON TABLE private_conversations TYPE datetime;
DEFINE FIELD end_time        ON TABLE private_conversations TYPE datetime;
DEFINE FIELD topic           ON TABLE private_conversations TYPE string;

DEFINE INDEX unique_private_conversation_id ON TABLE private_conversations COLUMNS conversation_id UNIQUE;

-- Example private conversations
CREATE private_conversations:conversation_001 SET
  conversation_id = "conversation_001",
  participants = ["user_123", "user_456"],
  start_time = "2023-03-22T10:00:00Z",
  end_time = "2023-03-22T11:00:00Z",
  topic = "Project Update";

CREATE private_conversations:conversation_002 SET
  conversation_id = "conversation_002",
  participants = ["user_789", "user_321"],
  start_time = "2023-03-22T12:00:00Z",
  end_time = "2023-03-22T13:00:00Z",
  topic = "Confidential Discussion";
