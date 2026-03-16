USE NS project_universe;
USE DB game;

DEFINE TABLE minds SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD id            ON TABLE minds TYPE string;
DEFINE FIELD being_id      ON TABLE minds TYPE record(beings);
DEFINE FIELD type          ON TABLE minds TYPE string;
DEFINE FIELD capacity      ON TABLE minds TYPE int;
DEFINE FIELD processing_speed ON TABLE minds TYPE float;
DEFINE FIELD memory_storage ON TABLE minds TYPE float;
DEFINE FIELD consciousness_level ON TABLE minds TYPE string;
DEFINE FIELD created_at    ON TABLE minds TYPE datetime;
DEFINE FIELD updated_at    ON TABLE minds TYPE datetime;

DEFINE INDEX unique_id ON TABLE minds COLUMNS id UNIQUE;

-- Example minds
CREATE minds:human_mind_001 SET
  id = "human_mind_001",
  being_id = beings:human_001,
  type = "Human",
  capacity = 100,
  processing_speed = 1.0,
  memory_storage = 2.5,
  consciousness_level = "Self-aware",
  created_at = time::now(),
  updated_at = time::now();

CREATE minds:alien_mind_001 SET
  id = "alien_mind_001",
  being_id = beings:alien_001,
  type = "Xylian",
  capacity = 150,
  processing_speed = 1.5,
  memory_storage = 5.0,
  consciousness_level = "Collective-aware",
  created_at = time::now(),
  updated_at = time::now(); 