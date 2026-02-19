USE NS project_universe;
USE DB technology;

DEFINE TABLE propulsion SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE propulsion TYPE string;
DEFINE FIELD description     ON TABLE propulsion TYPE string;
DEFINE FIELD type            ON TABLE propulsion TYPE string;
DEFINE FIELD principle       ON TABLE propulsion TYPE string;
DEFINE FIELD components      ON TABLE propulsion TYPE array;
DEFINE FIELD performance     ON TABLE propulsion TYPE object;
DEFINE FIELD limitations     ON TABLE propulsion TYPE array;
DEFINE FIELD applications    ON TABLE propulsion TYPE array;
DEFINE FIELD related_systems ON TABLE propulsion TYPE array;

DEFINE INDEX unique_propulsion_name ON TABLE propulsion COLUMNS name UNIQUE;

-- Example propulsion systems
CREATE propulsion:chemical_rocket SET
  name = "Chemical Rocket",
  description = "Rocket propulsion using chemical reactions to produce thrust",
  type = "Chemical",
  principle = "Combustion of propellant",
  components = ["Combustion Chamber", "Nozzle", "Fuel Tank"],
  performance = {specific_impulse: "300-450 s", thrust: "500-10000 kN"},
  limitations = ["High Fuel Consumption", "Environmental Impact"],
  applications = ["Space Launch", "Missile Propulsion"],
  related_systems = ["technology:power", "technology:materials"];

CREATE propulsion:ion_drive SET
  name = "Ion Drive",
  description = "Electric propulsion system using ionized propellant",
  type = "Electric",
  principle = "Ionization and acceleration of propellant",
  components = ["Ionization Chamber", "Accelerator Grid", "Power Supply"],
  performance = {specific_impulse: "2000-5000 s", thrust: "0.1-1 N"},
  limitations = ["Low Thrust", "High Power Requirement"],
  applications = ["Deep Space Missions", "Satellite Stationkeeping"],
  related_systems = ["technology:power", "technology:control"];

CREATE propulsion:nuclear_thermal_rocket SET
  name = "Nuclear Thermal Rocket",
  description = "Nuclear-powered thermal propulsion system",
  type = "Nuclear",
  principle = "Heating of propellant by nuclear reactor",
  components = ["Nuclear Reactor", "Heat Exchanger", "Nozzle"],
  performance = {specific_impulse: "800-1000 s", thrust: "100-1000 kN"},
  limitations = ["Radiation Hazard", "Complex Engineering"],
  applications = ["Interplanetary Travel", "Heavy Cargo Transport"],
  related_systems = ["technology:power", "technology:shielding"];

CREATE propulsion:fusion_drive SET
  name = "Fusion Drive",
  description = "Advanced propulsion system using controlled fusion reactions",
  type = "Fusion",
  principle = "Fusion of light atomic nuclei",
  components = ["Fusion Reactor", "Magnetic Confinement", "Exhaust Nozzle"],
  performance = {specific_impulse: "10000-50000 s", thrust: "10-100 kN"},
  limitations = ["Technological Challenges", "High Energy Requirement"],
  applications = ["Long-Duration Space Missions", "High-Speed Travel"],
  related_systems = ["technology:power", "technology:materials"];
