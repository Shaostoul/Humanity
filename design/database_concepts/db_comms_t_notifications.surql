USE NS project_universe;
USE DB communications;

DEFINE TABLE notifications SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD notification_id ON TABLE notifications TYPE string;
DEFINE FIELD recipient       ON TABLE notifications TYPE string;
DEFINE FIELD timestamp       ON TABLE notifications TYPE datetime;
DEFINE FIELD message         ON TABLE notifications TYPE string;
DEFINE FIELD status          ON TABLE notifications TYPE string;

DEFINE INDEX unique_notification_id ON TABLE notifications COLUMNS notification_id UNIQUE;

-- Example notifications
CREATE notifications:notification_001 SET
  notification_id = "notification_001",
  recipient = "user_123",
  timestamp = "2023-03-22T10:00:00Z",
  message = "You have a new message in your inbox.",
  status = "Unread";

CREATE notifications:notification_002 SET
  notification_id = "notification_002",
  recipient = "user_456",
  timestamp = "2023-03-22T10:05:00Z",
  message = "Your password will expire in 5 days.",
  status = "Read";
