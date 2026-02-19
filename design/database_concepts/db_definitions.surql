USE NS project_universe;

-- Define the main databases
DEFINE DATABASE lore;
DEFINE DATABASE science;
DEFINE DATABASE technology;
DEFINE DATABASE humanities;
DEFINE DATABASE lifeforms;

-- Define the main relationship types
DEFINE TABLE relationships SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE relationships TYPE string;
DEFINE FIELD description     ON TABLE relationships TYPE string;
DEFINE FIELD source_database ON TABLE relationships TYPE string;
DEFINE FIELD target_database ON TABLE relationships TYPE string;
DEFINE FIELD relationship_type ON TABLE relationships TYPE string;
DEFINE FIELD applications    ON TABLE relationships TYPE array;

DEFINE INDEX unique_relationship ON TABLE relationships COLUMNS name UNIQUE;

-- Define cross-database relationships
CREATE relationships:science_technology SET
  name = "Science-Technology Integration",
  description = "Relationship between scientific principles and technological applications",
  source_database = "science",
  target_database = "technology",
  relationship_type = "application",
  applications = ["Technology Development", "Scientific Advancement"];

CREATE relationships:technology_humanities SET
  name = "Technology-Humanities Integration",
  description = "Relationship between technological systems and human cultural aspects",
  source_database = "technology",
  target_database = "humanities",
  relationship_type = "influence",
  applications = ["Cultural Evolution", "Social Impact"];

CREATE relationships:lifeforms_science SET
  name = "Lifeforms-Science Integration",
  description = "Relationship between lifeforms and scientific understanding",
  source_database = "lifeforms",
  target_database = "science",
  relationship_type = "study",
  applications = ["Biological Research", "Scientific Discovery"];

CREATE relationships:humanities_lifeforms SET
  name = "Humanities-Lifeforms Integration",
  description = "Relationship between human cultural aspects and lifeform interactions",
  source_database = "humanities",
  target_database = "lifeforms",
  relationship_type = "interaction",
  applications = ["Cultural Adaptation", "Social Evolution"];

-- Define common field types
DEFINE FIELD common_fields {
  name: string,
  description: string,
  type: string,
  requirements: array,
  applications: array,
  related_systems: array
};

-- Define common permissions
DEFINE PERMISSION common_permissions {
  select: true,
  create: "$auth.role == 'admin'",
  update: "$auth.role == 'admin'",
  delete: "$auth.role == 'admin'"
};

-- Define common indexes
DEFINE INDEX common_indexes {
  unique_name: "COLUMNS name UNIQUE",
  type_index: "COLUMNS type"
};

-- Define database-specific schemas
DEFINE SCHEMA lore_schema {
  stellar_objects: {
    name: string,
    plural: string,
    purpose: string,
    type: string,
    min_size: float
  }
};

DEFINE SCHEMA science_schema {
  physics: {
    name: string,
    description: string,
    law_formula: string,
    variables: array,
    units: array,
    applications: array,
    limitations: array,
    related_laws: array
  },
  chemistry: {
    name: string,
    description: string,
    reaction: string,
    reactants: array,
    products: array,
    conditions: array,
    applications: array,
    hazards: array,
    related_reactions: array
  },
  biology: {
    name: string,
    description: string,
    process: string,
    requirements: array,
    products: array,
    conditions: array,
    applications: array,
    hazards: array,
    related_processes: array,
    optimal_conditions: object,
    survival_limits: object
  }
};

DEFINE SCHEMA technology_schema {
  propulsion: {
    name: string,
    description: string,
    type: string,
    principle: string,
    components: array,
    requirements: array,
    performance: object,
    limitations: array,
    applications: array,
    related_systems: array
  },
  power: {
    name: string,
    description: string,
    type: string,
    principle: string,
    components: array,
    requirements: array,
    performance: object,
    limitations: array,
    applications: array,
    related_systems: array
  },
  life_support: {
    name: string,
    description: string,
    type: string,
    principle: string,
    components: array,
    requirements: array,
    performance: object,
    limitations: array,
    applications: array,
    related_systems: array
  }
};

DEFINE SCHEMA humanities_schema {
  culture: {
    name: string,
    description: string,
    type: string,
    principle: string,
    components: array,
    requirements: array,
    characteristics: object,
    variations: array,
    applications: array,
    related_systems: array
  },
  language: {
    name: string,
    description: string,
    type: string,
    principle: string,
    components: array,
    requirements: array,
    characteristics: object,
    variations: array,
    applications: array,
    related_systems: array
  },
  philosophy: {
    name: string,
    description: string,
    type: string,
    principle: string,
    components: array,
    requirements: array,
    characteristics: object,
    variations: array,
    applications: array,
    related_systems: array
  }
};

DEFINE SCHEMA lifeforms_schema {
  species: {
    name: string,
    description: string,
    type: string,
    classification: object,
    characteristics: object,
    requirements: array,
    adaptations: array,
    related_systems: array
  },
  physiology: {
    name: string,
    description: string,
    type: string,
    body_structure: object,
    systems: array,
    requirements: array,
    characteristics: object,
    related_systems: array
  },
  behavior: {
    name: string,
    description: string,
    type: string,
    social_structure: object,
    communication: array,
    patterns: array,
    requirements: array,
    related_systems: array
  },
  ecology: {
    name: string,
    description: string,
    type: string,
    habitat: object,
    niche: string,
    interactions: array,
    food_web: object,
    environmental_impact: object,
    adaptations: array,
    threats: array,
    related_systems: array
  }
};

USE DB lore;

DEFINE TABLE stellar_objects SCHEMAFULL
  PERMISSIONS
    FOR create WHERE $auth.role == "admin"
    FOR select WHERE true
    FOR update WHERE $auth.role == "admin"
    FOR delete WHERE $auth.role == "admin";

DEFINE FIELD name      ON TABLE stellar_objects TYPE string;
DEFINE FIELD plural    ON TABLE stellar_objects TYPE string;
DEFINE FIELD purpose   ON TABLE stellar_objects TYPE string;
DEFINE FIELD type      ON TABLE stellar_objects TYPE string;
DEFINE FIELD min_size  ON TABLE stellar_objects TYPE float;

DEFINE INDEX unique_stellar_object_name   ON TABLE stellar_objects COLUMNS name UNIQUE;
DEFINE INDEX unique_stellar_object_plural ON TABLE stellar_objects COLUMNS plural UNIQUE;

-- Populate stellar_objects table
CREATE stellar_objects:blackhole SET
  name = "Blackhole",
  plural = "Blackholes",
  purpose = "Warps spacetime and consumes matter",
  type = "Stellar",
  min_size = 10;

CREATE stellar_objects:pulsar SET
  name = "Pulsar",
  plural = "Pulsars",
  purpose = "Emits periodic electromagnetic pulses",
  type = "Stellar",
  min_size = 10;

CREATE stellar_objects:planet SET
  name = "Planet",
  plural = "Planets",
  purpose = "Orbits a star, potentially habitable",
  type = "Substellar",
  min_size = 2000;

CREATE stellar_objects:dwarf_planet SET
  name = "Dwarf_Planet",
  plural = "Dwarf_Planets",
  purpose = "Orbits a star but doesn't clear its orbit",
  type = "Substellar",
  min_size = 500;

CREATE stellar_objects:star SET
  name = "Star",
  plural = "Stars",
  purpose = "Fuses hydrogen, emits energy",
  type = "Stellar",
  min_size = 700000;

CREATE stellar_objects:moon SET
  name = "Moon",
  plural = "Moons",
  purpose = "Orbits a planet, influences tides",
  type = "Substellar",
  min_size = 10;

CREATE stellar_objects:asteroid SET
  name = "Asteroid",
  plural = "Asteroids",
  purpose = "Rocky body orbiting a star",
  type = "Substellar",
  min_size = 0.025;

CREATE stellar_objects:comet SET
  name = "Comet",
  plural = "Comets",
  purpose = "Icy body with tail, orbits a star",
  type = "Substellar",
  min_size = 0.1;

CREATE stellar_objects:nebula SET
  name = "Nebula",
  plural = "Nebulae",
  purpose = "Cloud of gas and dust, site of star formation",
  type = "Stellar",
  min_size = 1000000000;

CREATE stellar_objects:meteoroid SET
  name = "Meteoroid",
  plural = "Meteoroids",
  purpose = "Small rocky body in space",
  type = "Substellar",
  min_size = 0.000001;

CREATE stellar_objects:meteorite SET
  name = "Meteorite",
  plural = "Meteorites",
  purpose = "Small rocky body entering atmosphere",
  type = "Substellar",
  min_size = 0.000001;

CREATE stellar_objects:protostar SET
  name = "Protostar",
  plural = "Protostars",
  purpose = "Early stage of star formation, collapsing gas/dust",
  type = "Stellar",
  min_size = 100000;

CREATE stellar_objects:white_dwarf SET
  name = "White_Dwarf",
  plural = "White_Dwarfs",
  purpose = "Dense remnant of a low-mass star, cooling",
  type = "Stellar",
  min_size = 5000;

CREATE stellar_objects:neutron_star SET
  name = "Neutron_Star",
  plural = "Neutron_Stars",
  purpose = "Dense remnant of a massive star, not pulsing",
  type = "Stellar",
  min_size = 10;

CREATE stellar_objects:supernova_remnant SET
  name = "Supernova_Remnant",
  plural = "Supernova_Remnants",
  purpose = "Expanding shell from a supernova, forms elements",
  type = "Stellar",
  min_size = 1000000;
