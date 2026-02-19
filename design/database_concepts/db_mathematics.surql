USE NS project_universe;

DEFINE DATABASE mathematics;

USE DB mathematics;

-- This is the main mathematics database file that defines the overall structure
-- and relationships between different mathematical concepts in the game.
-- The actual mathematical content is defined in the following tables:
-- - constants: Physical and mathematical constants
-- - formulas: Physics and engineering formulas
-- - geometry: Geometric formulas and relationships
-- - numerical_methods: Computational methods for solving mathematical problems
-- - statistical_methods: Statistical analysis methods

-- Define relationships between different mathematical concepts
DEFINE TABLE mathematical_relationships SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE mathematical_relationships TYPE string;
DEFINE FIELD description     ON TABLE mathematical_relationships TYPE string;
DEFINE FIELD source_concept  ON TABLE mathematical_relationships TYPE string;
DEFINE FIELD target_concept  ON TABLE mathematical_relationships TYPE string;
DEFINE FIELD relationship_type ON TABLE mathematical_relationships TYPE string;
DEFINE FIELD applications    ON TABLE mathematical_relationships TYPE array;

DEFINE INDEX unique_relationship ON TABLE mathematical_relationships COLUMNS name UNIQUE;

-- Example relationships
CREATE mathematical_relationships:kepler_orbital SET
  name = "Kepler's Laws and Orbital Mechanics",
  description = "Relationship between gravitational constants and orbital formulas",
  source_concept = "constants:gravitational",
  target_concept = "formulas:orbital_velocity",
  relationship_type = "dependency",
  applications = ["Spacecraft Navigation", "Orbital Planning"];

CREATE mathematical_relationships:energy_mass SET
  name = "Energy-Mass Equivalence",
  description = "Relationship between mass, energy, and the speed of light",
  source_concept = "constants:speed_of_light",
  target_concept = "formulas:kinetic_energy",
  relationship_type = "equivalence",
  applications = ["Propulsion Systems", "Energy Calculations"];

CREATE mathematical_relationships:statistical_physics SET
  name = "Statistical Physics Relationships",
  description = "Connection between Boltzmann constant and statistical methods",
  source_concept = "constants:boltzmann",
  target_concept = "statistical_methods:normal_distribution",
  relationship_type = "application",
  applications = ["Thermal Systems", "Particle Behavior"];
