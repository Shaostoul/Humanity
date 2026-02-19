USE NS project_universe;

DEFINE DATABASE science;

USE DB science;

-- This is the main science database file that defines the overall structure
-- and relationships between different scientific concepts in the game.
-- The actual scientific content is defined in the following tables:
-- - physics: Physical laws and principles
-- - chemistry: Chemical reactions and properties
-- - biology: Biological systems and processes
-- - astronomy: Astronomical phenomena and objects
-- - geology: Geological processes and materials
-- - ecology: Ecological systems and relationships

-- Define relationships between different scientific concepts
DEFINE TABLE scientific_relationships SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE scientific_relationships TYPE string;
DEFINE FIELD description     ON TABLE scientific_relationships TYPE string;
DEFINE FIELD source_concept  ON TABLE scientific_relationships TYPE string;
DEFINE FIELD target_concept  ON TABLE scientific_relationships TYPE string;
DEFINE FIELD relationship_type ON TABLE scientific_relationships TYPE string;
DEFINE FIELD applications    ON TABLE scientific_relationships TYPE array;

DEFINE INDEX unique_relationship ON TABLE scientific_relationships COLUMNS name UNIQUE;

-- Example relationships
CREATE scientific_relationships:gravity_orbital SET
  name = "Gravity and Orbital Mechanics",
  description = "Relationship between gravitational forces and orbital motion",
  source_concept = "physics:gravitational_force",
  target_concept = "astronomy:orbital_motion",
  relationship_type = "causation",
  applications = ["Spacecraft Navigation", "Planetary Formation"];

CREATE scientific_relationships:photosynthesis_respiration SET
  name = "Photosynthesis and Respiration",
  description = "Complementary biological processes in energy flow",
  source_concept = "biology:photosynthesis",
  target_concept = "biology:cellular_respiration",
  relationship_type = "cycle",
  applications = ["Life Support Systems", "Biosphere Management"];

CREATE scientific_relationships:geology_ecology SET
  name = "Geology and Ecology",
  description = "Interaction between geological processes and ecological systems",
  source_concept = "geology:soil_formation",
  target_concept = "ecology:terrestrial_ecosystems",
  relationship_type = "influence",
  applications = ["Habitat Design", "Resource Management"];
