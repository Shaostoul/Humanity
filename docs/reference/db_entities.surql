USE NS project_universe;
USE DB entities;

DEFINE TABLE entities SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD entity_id       ON TABLE entities TYPE string;
DEFINE FIELD name            ON TABLE entities TYPE string;
DEFINE FIELD type            ON TABLE entities TYPE string;
DEFINE FIELD description     ON TABLE entities TYPE string;

DEFINE INDEX unique_entity_id ON TABLE entities COLUMNS entity_id UNIQUE;

-- Example entities
CREATE entities:entity_001 SET
  entity_id = "entity_001",
  name = "Generic Entity",
  type = "Abstract",
  description = "A generic entity used for demonstration purposes.";

CREATE entities:entity_002 SET
  entity_id = "entity_002",
  name = "Sample Entity",
  type = "Concrete",
  description = "A sample entity used for testing.";
