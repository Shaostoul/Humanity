USE NS project_universe;
USE DB game;

DEFINE TABLE abilities SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD id            ON TABLE abilities TYPE string;
DEFINE FIELD being_id      ON TABLE abilities TYPE record(beings);
DEFINE FIELD name          ON TABLE abilities TYPE string;
DEFINE FIELD type          ON TABLE abilities TYPE string;
DEFINE FIELD level         ON TABLE abilities TYPE int;
DEFINE FIELD description   ON TABLE abilities TYPE string;
DEFINE FIELD requirements  ON TABLE abilities TYPE array;
DEFINE FIELD cooldown      ON TABLE abilities TYPE int;
DEFINE FIELD created_at    ON TABLE abilities TYPE datetime;
DEFINE FIELD updated_at    ON TABLE abilities TYPE datetime;

DEFINE INDEX unique_id ON TABLE abilities COLUMNS id UNIQUE;

-- Example abilities
CREATE abilities:human_ability_001 SET
  id = "human_ability_001",
  being_id = beings:human_001,
  name = "Enhanced Strength",
  type = "Physical",
  level = 3,
  description = "Temporary increase in physical strength",
  requirements = ["Energy", "Focus"],
  cooldown = 300,
  created_at = time::now(),
  updated_at = time::now();

CREATE abilities:alien_ability_001 SET
  id = "alien_ability_001",
  being_id = beings:alien_001,
  name = "Quantum Manipulation",
  type = "Energy",
  level = 5,
  description = "Ability to manipulate quantum states",
  requirements = ["Quantum Energy", "Focus"],
  cooldown = 600,
  created_at = time::now(),
  updated_at = time::now(); 