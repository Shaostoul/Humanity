USE NS project_universe;
USE DB game;

DEFINE TABLE soul SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD id            ON TABLE soul TYPE string;
DEFINE FIELD being_id      ON TABLE soul TYPE record(beings);
DEFINE FIELD essence       ON TABLE soul TYPE string;
DEFINE FIELD alignment     ON TABLE soul TYPE string;
DEFINE FIELD karma         ON TABLE soul TYPE int;
DEFINE FIELD aura          ON TABLE soul TYPE string;
DEFINE FIELD created_at    ON TABLE soul TYPE datetime;
DEFINE FIELD updated_at    ON TABLE soul TYPE datetime;

DEFINE INDEX unique_id ON TABLE soul COLUMNS id UNIQUE;

-- Example soul
CREATE soul:human_soul_001 SET
  id = "human_soul_001",
  being_id = beings:human_001,
  essence = "Pure",
  alignment = "Neutral Good",
  karma = 50,
  aura = "Blue",
  created_at = time::now(),
  updated_at = time::now(); 