USE NS project_universe;
USE DB entities;

DEFINE TABLE fleets SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD fleet_id        ON TABLE fleets TYPE string;
DEFINE FIELD name            ON TABLE fleets TYPE string;
DEFINE FIELD type            ON TABLE fleets TYPE string;
DEFINE FIELD size            ON TABLE fleets TYPE integer;
DEFINE FIELD purpose         ON TABLE fleets TYPE string;

DEFINE INDEX unique_fleet_id ON TABLE fleets COLUMNS fleet_id UNIQUE;

-- Example fleets
CREATE fleets:fleet_001 SET
  fleet_id = "fleet_001",
  name = "Atlantic Fleet",
  type = "Naval",
  size = 50,
  purpose = "Defense and Security";

CREATE fleets:fleet_002 SET
  fleet_id = "fleet_002",
  name = "Pacific Fleet",
  type = "Naval",
  size = 60,
  purpose = "Patrol and Surveillance";
