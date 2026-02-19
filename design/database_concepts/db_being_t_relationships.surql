USE NS project_universe;
USE DB game;

DEFINE TABLE relationships SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD id            ON TABLE relationships TYPE string;
DEFINE FIELD being_id_1    ON TABLE relationships TYPE record(beings);
DEFINE FIELD being_id_2    ON TABLE relationships TYPE record(beings);
DEFINE FIELD type          ON TABLE relationships TYPE string;
DEFINE FIELD status        ON TABLE relationships TYPE string;
DEFINE FIELD strength      ON TABLE relationships TYPE int;
DEFINE FIELD history       ON TABLE relationships TYPE array;
DEFINE FIELD created_at    ON TABLE relationships TYPE datetime;
DEFINE FIELD updated_at    ON TABLE relationships TYPE datetime;

DEFINE INDEX unique_id ON TABLE relationships COLUMNS id UNIQUE;
DEFINE INDEX relationship_pair ON TABLE relationships COLUMNS being_id_1, being_id_2 UNIQUE;

-- Example relationships
CREATE relationships:relationship_001 SET
  id = "relationship_001",
  being_id_1 = beings:human_001,
  being_id_2 = beings:alien_001,
  type = "Friendship",
  status = "Active",
  strength = 75,
  history = ["First contact", "Cultural exchange", "Joint mission"],
  created_at = time::now(),
  updated_at = time::now();

CREATE relationships:relationship_002 SET
  id = "relationship_002",
  being_id_1 = beings:human_001,
  being_id_2 = beings:human_002,
  type = "Family",
  status = "Active",
  strength = 90,
  history = ["Birth", "Childhood", "Adulthood"],
  created_at = time::now(),
  updated_at = time::now(); 