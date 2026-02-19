USE NS project_universe;
USE DB entities;

DEFINE TABLE ships SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD ship_id         ON TABLE ships TYPE string;
DEFINE FIELD name            ON TABLE ships TYPE string;
DEFINE FIELD type            ON TABLE ships TYPE string;
DEFINE FIELD tonnage         ON TABLE ships TYPE string;
DEFINE FIELD capacity        ON TABLE ships TYPE string;

DEFINE INDEX unique_ship_id ON TABLE ships COLUMNS ship_id UNIQUE;

-- Example ships
CREATE ships:ship_001 SET
  ship_id = "ship_001",
  name = "USS Enterprise",
  type = "Aircraft Carrier",
  tonnage = "100,000 tons",
  capacity = "5,000 personnel";

CREATE ships:ship_002 SET
  ship_id = "ship_002",
  name = "HMS Victory",
  type = "Ship of the Line",
  tonnage = "3,500 tons",
  capacity = "850 personnel";
