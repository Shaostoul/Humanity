USE NS project_universe;
USE DB game;

DEFINE TABLE souls SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD id            ON TABLE souls TYPE string;
DEFINE FIELD being_id      ON TABLE souls TYPE record(beings);
DEFINE FIELD type          ON TABLE souls TYPE string;
DEFINE FIELD energy_level  ON TABLE souls TYPE float;
DEFINE FIELD purity        ON TABLE souls TYPE int;
DEFINE FIELD resonance     ON TABLE souls TYPE string;
DEFINE FIELD connections   ON TABLE souls TYPE array;
DEFINE FIELD created_at    ON TABLE souls TYPE datetime;
DEFINE FIELD updated_at    ON TABLE souls TYPE datetime;

DEFINE INDEX unique_id ON TABLE souls COLUMNS id UNIQUE;

-- Example souls
CREATE souls:human_soul_001 SET
  id = "human_soul_001",
  being_id = beings:human_001,
  type = "Human",
  energy_level = 100.0,
  purity = 85,
  resonance = "Harmonic",
  connections = ["family", "friends", "community"],
  created_at = time::now(),
  updated_at = time::now();

CREATE souls:alien_soul_001 SET
  id = "alien_soul_001",
  being_id = beings:alien_001,
  type = "Xylian",
  energy_level = 150.0,
  purity = 90,
  resonance = "Quantum",
  connections = ["hive_mind", "collective"],
  created_at = time::now(),
  updated_at = time::now(); 