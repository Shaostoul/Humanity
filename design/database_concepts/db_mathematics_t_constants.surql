USE NS project_universe;
USE DB mathematics;

DEFINE TABLE constants SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE constants TYPE string;
DEFINE FIELD symbol          ON TABLE constants TYPE string;
DEFINE FIELD value           ON TABLE constants TYPE float;
DEFINE FIELD units           ON TABLE constants TYPE string;
DEFINE FIELD description     ON TABLE constants TYPE string;
DEFINE FIELD applications    ON TABLE constants TYPE array;

DEFINE INDEX unique_constant ON TABLE constants COLUMNS name UNIQUE;

CREATE constants:pi SET
  name = "Pi",
  symbol = "π",
  value = 3.141592653589793,
  units = "Unitless",
  description = "Ratio of circle circumference to diameter.",
  applications = ["Circle Geometry", "Orbital Mechanics"];

CREATE constants:e SET
  name = "Euler's Number",
  symbol = "e",
  value = 2.718281828459045,
  units = "Unitless",
  description = "Base of natural logarithms.",
  applications = ["Exponential Growth", "Decay", "Natural Processes"];

CREATE constants:golden_ratio SET
  name = "Golden Ratio",
  symbol = "φ",
  value = 1.618033988749895,
  units = "Unitless",
  description = "Ratio where the whole is to the larger part as the larger part is to the smaller part.",
  applications = ["Design", "Habitat Construction Aesthetics"];

CREATE constants:speed_of_light SET
  name = "Speed of Light",
  symbol = "c",
  value = 299792.458,
  units = "km/s",
  description = "Universal constant speed of electromagnetic radiation in vacuum.",
  applications = ["Interstellar Communication", "Navigation Calculations"];

CREATE constants:gravitational SET
  name = "Gravitational Constant",
  symbol = "G",
  value = 6.67430e-11,
  units = "m³⋅kg⁻¹⋅s⁻²",
  description = "Universal constant of gravitation.",
  applications = ["Orbital Simulations", "Gravity Calculations"];

CREATE constants:planck SET
  name = "Planck Constant",
  symbol = "h",
  value = 6.62607015e-34,
  units = "J⋅s",
  description = "Fundamental constant of quantum mechanics.",
  applications = ["Quantum Mechanics", "Advanced Technology"];

CREATE constants:boltzmann SET
  name = "Boltzmann Constant",
  symbol = "k",
  value = 1.380649e-23,
  units = "J⋅K⁻¹",
  description = "Constant relating temperature to energy.",
  applications = ["Temperature-Energy Relationships", "Thermal Management"];

CREATE constants:avogadro SET
  name = "Avogadro's Number",
  symbol = "Nₐ",
  value = 6.02214076e23,
  units = "mol⁻¹",
  description = "Number of constituent particles in one mole of a substance.",
  applications = ["Chemistry", "Resource Refinement", "Material Processing"];
