USE NS project_universe;

DEFINE DATABASE technology;

USE DB technology;

-- This is the main technology database file that defines the overall structure
-- and relationships between different technological concepts in the game.
-- The actual technological content is defined in the following tables:
-- - propulsion: Propulsion systems and engines
-- - power: Power generation and distribution
-- - life_support: Life support and environmental systems
-- - communication: Communication and data systems
-- - sensors: Sensors and detection systems

-- Define relationships between different technological concepts
DEFINE TABLE technological_relationships SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE technological_relationships TYPE string;
DEFINE FIELD description     ON TABLE technological_relationships TYPE string;
DEFINE FIELD source_system   ON TABLE technological_relationships TYPE string;
DEFINE FIELD target_system   ON TABLE technological_relationships TYPE string;
DEFINE FIELD relationship_type ON TABLE technological_relationships TYPE string;
DEFINE FIELD applications    ON TABLE technological_relationships TYPE array;

DEFINE INDEX unique_technological_relationship ON TABLE technological_relationships COLUMNS name UNIQUE;

-- Example relationships
CREATE technological_relationships:propulsion_power SET
  name = "Propulsion-Power Integration",
  description = "Relationship between propulsion systems and power generation",
  source_system = "propulsion:ion_drive",
  target_system = "power:nuclear_reactor",
  relationship_type = "dependency",
  applications = ["Spacecraft Propulsion", "Energy Management"];

CREATE technological_relationships:life_support_power SET
  name = "Life Support-Power Integration",
  description = "Relationship between life support systems and power requirements",
  source_system = "life_support:air_recycling",
  target_system = "power:power_distribution",
  relationship_type = "dependency",
  applications = ["Habitat Management", "Resource Optimization"];

CREATE technological_relationships:communication_sensors SET
  name = "Communication-Sensors Integration",
  description = "Relationship between communication systems and sensor networks",
  source_system = "communication:quantum_network",
  target_system = "sensors:deep_space_array",
  relationship_type = "integration",
  applications = ["Data Collection", "Remote Sensing"];
