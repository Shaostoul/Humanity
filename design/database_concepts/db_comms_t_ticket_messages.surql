USE NS project_universe;
USE DB communications;

DEFINE TABLE ticket_messages SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD ticket_id       ON TABLE ticket_messages TYPE string;
DEFINE FIELD message_id      ON TABLE ticket_messages TYPE string;
DEFINE FIELD sender          ON TABLE ticket_messages TYPE string;
DEFINE FIELD timestamp       ON TABLE ticket_messages TYPE datetime;
DEFINE FIELD content         ON TABLE ticket_messages TYPE string;
DEFINE FIELD status          ON TABLE ticket_messages TYPE string;

DEFINE INDEX unique_ticket_message_id ON TABLE ticket_messages COLUMNS message_id UNIQUE;

-- Example ticket messages
CREATE ticket_messages:message_001 SET
  ticket_id = "ticket_001",
  message_id = "message_001",
  sender = "support_agent_123",
  timestamp = "2023-03-22T09:00:00Z",
  content = "Initial response to the support ticket.",
  status = "Open";

CREATE ticket_messages:message_002 SET
  ticket_id = "ticket_001",
  message_id = "message_002",
  sender = "customer_456",
  timestamp = "2023-03-22T09:30:00Z",
  content = "Follow-up question regarding the issue.",
  status = "Pending";
