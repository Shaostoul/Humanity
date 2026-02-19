USE NS project_universe;
USE DB entities;

DEFINE TABLE livestock SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD livestock_id    ON TABLE livestock TYPE string;
DEFINE FIELD name            ON TABLE livestock TYPE string;
DEFINE FIELD species         ON TABLE livestock TYPE string;
DEFINE FIELD age             ON TABLE livestock TYPE integer;
DEFINE FIELD purpose         ON TABLE livestock TYPE string;

DEFINE INDEX unique_livestock_id ON TABLE livestock COLUMNS livestock_id UNIQUE;

-- Example livestock
CREATE livestock:livestock_001 SET
  livestock_id = "livestock_001",
  name = "Bessie",
  species = "Cow",
  age = 5,
  purpose = "Dairy Production";

CREATE livestock:livestock_002 SET
  livestock_id = "livestock_002",
  name = "Clucky",
  species = "Chicken",
  age = 2,
  purpose = "Egg Production";
