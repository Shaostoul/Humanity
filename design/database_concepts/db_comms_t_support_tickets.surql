USE NS project_universe;
USE DB communications;

DEFINE TABLE support_tickets SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD ticket_id       ON TABLE support_tickets TYPE string;
DEFINE FIELD subject         ON TABLE support_tickets TYPE string;
DEFINE FIELD customer        ON TABLE support_tickets TYPE string;
DEFINE FIELD status          ON TABLE support_tickets TYPE string;
DEFINE FIELD created_at      ON TABLE support_tickets TYPE datetime;
DEFINE FIELD updated_at      ON TABLE support_tickets TYPE datetime;

DEFINE INDEX unique_support_ticket_id ON TABLE support_tickets COLUMNS ticket_id UNIQUE;

-- Example support tickets
CREATE support_tickets:ticket_001 SET
  ticket_id = "ticket_001",
  subject = "Login Issue",
  customer = "user_123",
  status = "Open",
  created_at = "2023-03-22T09:00:00Z",
  updated_at = "2023-03-22T09:30:00Z";

CREATE support_tickets:ticket_002 SET
  ticket_id = "ticket_002",
  subject = "Feature Request",
  customer = "user_456",
  status = "Closed",
  created_at = "2023-03-22T10:00:00Z",
  updated_at = "2023-03-22T11:00:00Z";
