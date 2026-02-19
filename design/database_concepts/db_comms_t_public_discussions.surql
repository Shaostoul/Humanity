USE NS project_universe;
USE DB communications;

DEFINE TABLE public_discussions SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD discussion_id   ON TABLE public_discussions TYPE string;
DEFINE FIELD topic           ON TABLE public_discussions TYPE string;
DEFINE FIELD participants    ON TABLE public_discussions TYPE array;
DEFINE FIELD start_time      ON TABLE public_discussions TYPE datetime;
DEFINE FIELD end_time        ON TABLE public_discussions TYPE datetime;

DEFINE INDEX unique_public_discussion_id ON TABLE public_discussions COLUMNS discussion_id UNIQUE;

-- Example public discussions
CREATE public_discussions:discussion_001 SET
  discussion_id = "discussion_001",
  topic = "Community Guidelines Update",
  participants = ["user_123", "user_456", "user_789"],
  start_time = "2023-03-22T10:00:00Z",
  end_time = "2023-03-22T11:00:00Z";

CREATE public_discussions:discussion_002 SET
  discussion_id = "discussion_002",
  topic = "Feature Requests",
  participants = ["user_321", "user_654", "user_987"],
  start_time = "2023-03-22T12:00:00Z",
  end_time = "2023-03-22T13:00:00Z";
