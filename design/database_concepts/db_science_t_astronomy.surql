USE NS project_universe;
USE DB science;

DEFINE TABLE astronomy SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE astronomy TYPE string;
DEFINE FIELD description     ON TABLE astronomy TYPE string;
DEFINE FIELD type           ON TABLE astronomy TYPE string;
DEFINE FIELD characteristics ON TABLE astronomy TYPE array;
DEFINE FIELD formation      ON TABLE astronomy TYPE string;
DEFINE FIELD lifecycle      ON TABLE astronomy TYPE string;
DEFINE FIELD hazards        ON TABLE astronomy TYPE array;
DEFINE FIELD resources      ON TABLE astronomy TYPE array;
DEFINE FIELD related_objects ON TABLE astronomy TYPE array;
DEFINE FIELD physical_properties ON TABLE astronomy TYPE object;
DEFINE FIELD orbital_properties ON TABLE astronomy TYPE object;

DEFINE INDEX unique_astronomical_object ON TABLE astronomy COLUMNS name UNIQUE;
DEFINE INDEX object_type ON TABLE astronomy COLUMNS type;
DEFINE INDEX physical_properties ON TABLE astronomy COLUMNS physical_properties;
DEFINE INDEX orbital_properties ON TABLE astronomy COLUMNS orbital_properties;

-- Stellar Objects
CREATE astronomy:star SET
  name = "Star",
  description = "Massive, luminous sphere of plasma held together by gravity",
  type = "Stellar",
  characteristics = ["Nuclear fusion", "Light emission", "Heat generation"],
  formation = "Gravitational collapse of gas cloud",
  lifecycle = "Protostar → Main sequence → Red giant → White dwarf/Neutron star/Black hole",
  hazards = ["Radiation", "Solar flares", "Gravitational effects"],
  resources = ["Helium-3", "Heavy elements", "Energy"],
  related_objects = ["astronomy:planets", "astronomy:asteroids", "astronomy:comets"],
  physical_properties = {
    mass_range: "0.1-100 solar masses",
    temperature: "3000-50000 K",
    luminosity: "0.0001-1000000 solar luminosities",
    diameter: "100000-1000000 km"
  },
  orbital_properties = {
    rotation_period: "Hours to months",
    orbital_period: "Millions of years around galactic center",
    eccentricity: "0.0-0.2"
  };

CREATE astronomy:planet SET
  name = "Planet",
  description = "Celestial body orbiting a star, massive enough to be rounded by gravity",
  type = "Planetary",
  characteristics = ["Orbital motion", "Atmospheric retention", "Geological activity"],
  formation = "Accretion in protoplanetary disk",
  lifecycle = "Formation → Active → Stable → Aging",
  hazards = ["Meteor impacts", "Atmospheric conditions", "Radiation"],
  resources = ["Minerals", "Water", "Atmospheric gases"],
  related_objects = ["astronomy:star", "astronomy:moons", "astronomy:asteroids"],
  physical_properties = {
    mass_range: "0.0001-1000 Earth masses",
    temperature: "-230°C to 460°C",
    diameter: "1000-150000 km",
    atmosphere: "Present or absent"
  },
  orbital_properties = {
    rotation_period: "Hours to days",
    orbital_period: "Days to years",
    eccentricity: "0.0-0.5"
  };

CREATE astronomy:asteroid SET
  name = "Asteroid",
  description = "Small rocky body orbiting a star",
  type = "Small body",
  characteristics = ["Irregular shape", "Rocky composition", "Orbital motion"],
  formation = "Remnants of planet formation",
  lifecycle = "Formation → Orbital evolution → Collision/Impact",
  hazards = ["Impact potential", "Orbital instability", "Collision risk"],
  resources = ["Minerals", "Water ice", "Metals"],
  related_objects = ["astronomy:star", "astronomy:planets", "astronomy:comets"],
  physical_properties = {
    mass_range: "10^6-10^20 kg",
    temperature: "-150°C to 100°C",
    diameter: "1-1000 km",
    composition: "Rocky, metallic, carbonaceous"
  },
  orbital_properties = {
    rotation_period: "Hours to days",
    orbital_period: "Years to centuries",
    eccentricity: "0.0-0.8"
  };

CREATE astronomy:comet SET
  name = "Comet",
  description = "Icy body with a tail, orbiting a star",
  type = "Small body",
  characteristics = ["Icy nucleus", "Coma", "Tail formation"],
  formation = "Outer solar system formation",
  lifecycle = "Formation → Storage → Activation → Decay",
  hazards = ["Impact potential", "Dust tail effects", "Orbital instability"],
  resources = ["Water ice", "Organic compounds", "Volatiles"],
  related_objects = ["astronomy:star", "astronomy:asteroids", "astronomy:planets"],
  physical_properties = {
    mass_range: "10^12-10^18 kg",
    temperature: "-200°C to 100°C",
    diameter: "1-50 km",
    composition: "Ice, dust, organic compounds"
  },
  orbital_properties = {
    rotation_period: "Hours to days",
    orbital_period: "Years to millennia",
    eccentricity: "0.5-1.0"
  };

CREATE astronomy:black_hole SET
  name = "Black Hole",
  description = "Region of spacetime where gravity prevents everything from escaping",
  type = "Stellar remnant",
  characteristics = ["Event horizon", "Gravitational lensing", "Accretion disk"],
  formation = "Collapse of massive star or galactic center",
  lifecycle = "Formation → Growth → Evaporation",
  hazards = ["Tidal forces", "Radiation", "Gravitational effects"],
  resources = ["Energy extraction", "Gravitational potential"],
  related_objects = ["astronomy:star", "astronomy:galaxy", "astronomy:neutron_star"],
  physical_properties = {
    mass_range: "3-1000000000 solar masses",
    temperature: "Near absolute zero to millions K",
    diameter: "Event horizon radius",
    type: "Stellar, intermediate, supermassive"
  },
  orbital_properties = {
    rotation_period: "Hours to days",
    orbital_period: "Millions of years",
    eccentricity: "0.0-0.9"
  }; 