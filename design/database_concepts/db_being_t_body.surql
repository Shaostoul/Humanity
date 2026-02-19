USE NS project_universe;
USE DB game;

DEFINE TABLE body SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD id            ON TABLE body TYPE string;
DEFINE FIELD being_id      ON TABLE body TYPE record(beings);
DEFINE FIELD height        ON TABLE body TYPE float;
DEFINE FIELD weight        ON TABLE body TYPE float;
DEFINE FIELD strength      ON TABLE body TYPE int;
DEFINE FIELD agility       ON TABLE body TYPE int;
DEFINE FIELD endurance     ON TABLE body TYPE int;
DEFINE FIELD health_status ON TABLE body TYPE string;
DEFINE FIELD created_at    ON TABLE body TYPE datetime;
DEFINE FIELD updated_at    ON TABLE body TYPE datetime;

DEFINE INDEX unique_id ON TABLE body COLUMNS id UNIQUE;

-- Example body
CREATE body:human_body_001 SET
  id = "human_body_001",
  being_id = beings:human_001,
  height = 1.8,
  weight = 75.0,
  strength = 7,
  agility = 6,
  endurance = 8,
  health_status = "Healthy",
  created_at = time::now(),
  updated_at = time::now(); 