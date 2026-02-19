USE NS project_universe;
USE DB game;

DEFINE TABLE mind SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD id            ON TABLE mind TYPE string;
DEFINE FIELD being_id      ON TABLE mind TYPE record(beings);
DEFINE FIELD intelligence  ON TABLE mind TYPE int;
DEFINE FIELD wisdom        ON TABLE mind TYPE int;
DEFINE FIELD memory        ON TABLE mind TYPE int;
DEFINE FIELD perception    ON TABLE mind TYPE int;
DEFINE FIELD consciousness ON TABLE mind TYPE string;
DEFINE FIELD created_at    ON TABLE mind TYPE datetime;
DEFINE FIELD updated_at    ON TABLE mind TYPE datetime;

DEFINE INDEX unique_id ON TABLE mind COLUMNS id UNIQUE;

-- Example mind
CREATE mind:human_mind_001 SET
  id = "human_mind_001",
  being_id = beings:human_001,
  intelligence = 8,
  wisdom = 7,
  memory = 6,
  perception = 7,
  consciousness = "Awake",
  created_at = time::now(),
  updated_at = time::now(); 