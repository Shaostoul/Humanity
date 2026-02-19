USE NS project_universe;
USE DB entities;

DEFINE TABLE settlements SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD settlement_id   ON TABLE settlements TYPE string;
DEFINE FIELD name            ON TABLE settlements TYPE string;
DEFINE FIELD type            ON TABLE settlements TYPE string;
DEFINE FIELD population      ON TABLE settlements TYPE integer;
DEFINE FIELD location        ON TABLE settlements TYPE string;

DEFINE INDEX unique_settlement_id ON TABLE settlements COLUMNS settlement_id UNIQUE;

-- Example settlements
CREATE settlements:settlement_001 SET
  settlement_id = "settlement_001",
  name = "Rivendell",
  type = "Elven City",
  population = 500,
  location = "Valley of Imladris";

CREATE settlements:settlement_002 SET
  settlement_id = "settlement_002",
  name = "Hobbiton",
  type = "Hobbit Village",
  population = 300,
  location = "The Shire";
