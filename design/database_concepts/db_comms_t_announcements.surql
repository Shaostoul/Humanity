USE NS project_universe;
USE DB communications;

DEFINE TABLE announcements SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD announcement_id ON TABLE announcements TYPE string;
DEFINE FIELD title           ON TABLE announcements TYPE string;
DEFINE FIELD content         ON TABLE announcements TYPE string;
DEFINE FIELD timestamp       ON TABLE announcements TYPE datetime;
DEFINE FIELD author          ON TABLE announcements TYPE string;

DEFINE INDEX unique_announcement_id ON TABLE announcements COLUMNS announcement_id UNIQUE;

-- Example announcements
CREATE announcements:announcement_001 SET
  announcement_id = "announcement_001",
  title = "System Maintenance",
  content = "The system will be down for maintenance on March 25th from 2 AM to 4 AM UTC.",
  timestamp = "2023-03-22T08:00:00Z",
  author = "admin";

CREATE announcements:announcement_002 SET
  announcement_id = "announcement_002",
  title = "New Feature Release",
  content = "We are excited to announce the release of our new feature, available from March 26th.",
  timestamp = "2023-03-22T09:00:00Z",
  author = "product_team";
