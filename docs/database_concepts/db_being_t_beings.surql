USE NS project_universe;
USE DB game;

DEFINE TABLE beings SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD id            ON TABLE beings TYPE string;
DEFINE FIELD name          ON TABLE beings TYPE string;
DEFINE FIELD species       ON TABLE beings TYPE string;
DEFINE FIELD gender        ON TABLE beings TYPE string;
DEFINE FIELD age           ON TABLE beings TYPE int;
DEFINE FIELD body_id       ON TABLE beings TYPE record(bodies);
DEFINE FIELD mind_id       ON TABLE beings TYPE record(minds);
DEFINE FIELD soul_id       ON TABLE beings TYPE record(souls);
DEFINE FIELD created_at    ON TABLE beings TYPE datetime;
DEFINE FIELD updated_at    ON TABLE beings TYPE datetime;

DEFINE INDEX unique_id ON TABLE beings COLUMNS id UNIQUE;
DEFINE INDEX unique_name ON TABLE beings COLUMNS name UNIQUE;

-- Example beings
CREATE beings:human_001 SET
  id = "human_001",
  name = "John Smith",
  species = "Human",
  gender = "Male",
  age = 30,
  body_id = bodies:human_body_001,
  mind_id = minds:human_mind_001,
  soul_id = souls:human_soul_001,
  created_at = time::now(),
  updated_at = time::now();

CREATE beings:alien_001 SET
  id = "alien_001",
  name = "Xyl'vath",
  species = "Xylian",
  gender = "Non-binary",
  age = 150,
  body_id = bodies:alien_body_001,
  mind_id = minds:alien_mind_001,
  soul_id = souls:alien_soul_001,
  created_at = time::now(),
  updated_at = time::now(); 