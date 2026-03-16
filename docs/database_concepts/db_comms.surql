USE NS project_universe;
USE DB communications;

DEFINE TABLE communications SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE communications TYPE string;
DEFINE FIELD description     ON TABLE communications TYPE string;
DEFINE FIELD type            ON TABLE communications TYPE string;
DEFINE FIELD components      ON TABLE communications TYPE array;
DEFINE FIELD applications    ON TABLE communications TYPE array;
DEFINE FIELD related_fields  ON TABLE communications TYPE array;

DEFINE INDEX unique_communications_name ON TABLE communications COLUMNS name UNIQUE;

-- Example communication systems
CREATE communications:email_system SET
  name = "Email System",
  description = "A system for sending and receiving electronic mail",
  type = "Digital",
  components = ["Mail Servers", "Email Clients"],
  applications = ["Business Communication", "Personal Communication"],
  related_fields = ["technology:networking", "technology:security"];

CREATE communications:instant_messaging SET
  name = "Instant Messaging",
  description = "A system for real-time text communication",
  type = "Digital",
  components = ["Messaging Servers", "User Clients"],
  applications = ["Team Collaboration", "Customer Support"],
  related_fields = ["technology:networking", "technology:security"];

CREATE communications:video_conferencing SET
  name = "Video Conferencing",
  description = "A system for conducting live video meetings",
  type = "Digital",
  components = ["Video Servers", "User Clients"],
  applications = ["Remote Work", "Online Education"],
  related_fields = ["technology:networking", "technology:security"];

CREATE communications:telephony SET
  name = "Telephony",
  description = "A system for voice communication over distances",
  type = "Analog/Digital",
  components = ["Switching Systems", "User Handsets"],
  applications = ["Business Communication", "Emergency Services"],
  related_fields = ["technology:networking", "technology:security"];
