USE NS project_universe;
USE DB entities;

DEFINE TABLE player_characters SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD character_id    ON TABLE player_characters TYPE string;
DEFINE FIELD name            ON TABLE player_characters TYPE string;
DEFINE FIELD class           ON TABLE player_characters TYPE string;
DEFINE FIELD level           ON TABLE player_characters TYPE integer;
DEFINE FIELD abilities       ON TABLE player_characters TYPE array;

DEFINE INDEX unique_player_character_id ON TABLE player_characters COLUMNS character_id UNIQUE;

-- Example player characters
CREATE player_characters:character_001 SET
  character_id = "character_001",
  name = "Aragorn",
  class = "Ranger",
  level = 20,
  abilities = ["Tracking", "Swordsmanship"];

CREATE player_characters:character_002 SET
  character_id = "character_002",
  name = "Gandalf",
  class = "Wizard",
  level = 30,
  abilities = ["Magic", "Wisdom"];
