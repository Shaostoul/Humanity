USE NS project_universe;

DEFINE DATABASE humanities;

USE DB humanities;

-- This is the main humanities database file that defines the overall structure
-- and relationships between different humanities concepts in the game.
-- The actual humanities content is defined in the following tables:
-- - culture: Cultural systems and practices
-- - language: Languages and communication systems
-- - philosophy: Philosophical concepts and schools of thought
-- - religion: Religious systems and beliefs
-- - art: Artistic expressions and forms
-- - history: Historical events and periods
-- - society: Social structures and organizations
-- - education: Educational systems and methods

-- Define relationships between different humanities concepts
DEFINE TABLE humanities_relationships SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE humanities_relationships TYPE string;
DEFINE FIELD description     ON TABLE humanities_relationships TYPE string;
DEFINE FIELD source_concept  ON TABLE humanities_relationships TYPE string;
DEFINE FIELD target_concept  ON TABLE humanities_relationships TYPE string;
DEFINE FIELD relationship_type ON TABLE humanities_relationships TYPE string;
DEFINE FIELD applications    ON TABLE humanities_relationships TYPE array;

DEFINE INDEX unique_humanities_relationship ON TABLE humanities_relationships COLUMNS name UNIQUE;

-- Example relationships
CREATE humanities_relationships:language_culture SET
  name = "Language-Culture Integration",
  description = "Relationship between language and cultural expression",
  source_concept = "language:spoken_language",
  target_concept = "culture:traditional_culture",
  relationship_type = "influence",
  applications = ["Cultural Communication", "Social Integration"];

CREATE humanities_relationships:religion_philosophy SET
  name = "Religion-Philosophy Integration",
  description = "Relationship between religious beliefs and philosophical thought",
  source_concept = "religion:traditional_religion",
  target_concept = "philosophy:ethics",
  relationship_type = "influence",
  applications = ["Belief Systems", "Ethical Frameworks"];

CREATE humanities_relationships:art_society SET
  name = "Art-Society Integration",
  description = "Relationship between artistic expression and social structures",
  source_concept = "art:artistic_form",
  target_concept = "society:social_structure",
  relationship_type = "reflection",
  applications = ["Cultural Expression", "Social Commentary"];
