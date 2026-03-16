USE NS project_universe;
USE DB game;

DEFINE TABLE health_conditions SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD id            ON TABLE health_conditions TYPE string;
DEFINE FIELD being_id      ON TABLE health_conditions TYPE record(beings);
DEFINE FIELD condition     ON TABLE health_conditions TYPE string;
DEFINE FIELD severity      ON TABLE health_conditions TYPE int;
DEFINE FIELD symptoms      ON TABLE health_conditions TYPE array;
DEFINE FIELD treatment     ON TABLE health_conditions TYPE string;
DEFINE FIELD status        ON TABLE health_conditions TYPE string;
DEFINE FIELD created_at    ON TABLE health_conditions TYPE datetime;
DEFINE FIELD updated_at    ON TABLE health_conditions TYPE datetime;

DEFINE INDEX unique_id ON TABLE health_conditions COLUMNS id UNIQUE;

-- Example health conditions
CREATE health_conditions:condition_001 SET
  id = "condition_001",
  being_id = beings:human_001,
  condition = "Common Cold",
  severity = 2,
  symptoms = ["Cough", "Sneezing", "Fever"],
  treatment = "Rest and hydration",
  status = "Active",
  created_at = time::now(),
  updated_at = time::now();

CREATE health_conditions:condition_002 SET
  id = "condition_002",
  being_id = beings:alien_001,
  condition = "Quantum Instability",
  severity = 4,
  symptoms = ["Energy fluctuations", "Dimensional shifts"],
  treatment = "Stabilization therapy",
  status = "Chronic",
  created_at = time::now(),
  updated_at = time::now(); 