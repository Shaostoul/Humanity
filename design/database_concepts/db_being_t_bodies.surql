USE NS project_universe;
USE DB game;

DEFINE TABLE bodies SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD id            ON TABLE bodies TYPE string;
DEFINE FIELD being_id      ON TABLE bodies TYPE record(beings);
DEFINE FIELD type          ON TABLE bodies TYPE string;
DEFINE FIELD mass          ON TABLE bodies TYPE float;
DEFINE FIELD volume        ON TABLE bodies TYPE float;
DEFINE FIELD density       ON TABLE bodies TYPE float;
DEFINE FIELD composition   ON TABLE bodies TYPE array;
DEFINE FIELD created_at    ON TABLE bodies TYPE datetime;
DEFINE FIELD updated_at    ON TABLE bodies TYPE datetime;

DEFINE INDEX unique_id ON TABLE bodies COLUMNS id UNIQUE;

-- Example bodies
CREATE bodies:human_body_001 SET
  id = "human_body_001",
  being_id = beings:human_001,
  type = "Humanoid",
  mass = 75.0,
  volume = 0.07,
  density = 1071.4,
  composition = ["Carbon", "Hydrogen", "Oxygen", "Nitrogen", "Calcium", "Phosphorus"],
  created_at = time::now(),
  updated_at = time::now();

CREATE bodies:alien_body_001 SET
  id = "alien_body_001",
  being_id = beings:alien_001,
  type = "Xylian",
  mass = 90.0,
  volume = 0.08,
  density = 1125.0,
  composition = ["Silicon", "Carbon", "Hydrogen", "Oxygen", "Xylithium"],
  created_at = time::now(),
  updated_at = time::now(); 