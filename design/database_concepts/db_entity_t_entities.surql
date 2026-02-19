USE NS project_universe;
USE DB game;

DEFINE TABLE entities SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD id            ON TABLE entities TYPE string;
DEFINE FIELD name          ON TABLE entities TYPE string;
DEFINE FIELD type          ON TABLE entities TYPE string;
DEFINE FIELD category      ON TABLE entities TYPE string;
DEFINE FIELD description   ON TABLE entities TYPE string;
DEFINE FIELD attributes    ON TABLE entities TYPE object;
DEFINE FIELD status        ON TABLE entities TYPE string;
DEFINE FIELD created_at    ON TABLE entities TYPE datetime;
DEFINE FIELD updated_at    ON TABLE entities TYPE datetime;

DEFINE INDEX unique_id ON TABLE entities COLUMNS id UNIQUE;
DEFINE INDEX entity_type ON TABLE entities COLUMNS type;

-- Example entities
CREATE entities:entity_001 SET
  id = "entity_001",
  name = "Quantum Core",
  type = "Energy",
  category = "Power Source",
  description = "Advanced energy source utilizing quantum principles",
  attributes = {
    power_output: 1000,
    efficiency: 0.95,
    stability: "High",
    maintenance_required: false
  },
  status = "Active",
  created_at = time::now(),
  updated_at = time::now();

CREATE entities:entity_002 SET
  id = "entity_002",
  name = "Gravity Stabilizer",
  type = "Technology",
  category = "Environmental Control",
  description = "Device for maintaining stable gravitational fields",
  attributes = {
    range: 100,
    power_consumption: 500,
    effectiveness: 0.98,
    durability: "Excellent"
  },
  status = "Operational",
  created_at = time::now(),
  updated_at = time::now(); 