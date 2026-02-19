USE NS project_universe;
USE DB technology;

DEFINE TABLE energy_systems SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE energy_systems TYPE string;
DEFINE FIELD description     ON TABLE energy_systems TYPE string;
DEFINE FIELD type            ON TABLE energy_systems TYPE string;
DEFINE FIELD principle       ON TABLE energy_systems TYPE string;
DEFINE FIELD components      ON TABLE energy_systems TYPE array;
DEFINE FIELD performance     ON TABLE energy_systems TYPE object;
DEFINE FIELD limitations     ON TABLE energy_systems TYPE array;
DEFINE FIELD applications    ON TABLE energy_systems TYPE array;
DEFINE FIELD related_systems ON TABLE energy_systems TYPE array;

DEFINE INDEX unique_energy_system_name ON TABLE energy_systems COLUMNS name UNIQUE;

-- Example energy systems
CREATE energy_systems:solar_array SET
  name = "Solar Array",
  description = "Photovoltaic system for converting solar radiation to electricity",
  type = "Photovoltaic",
  principle = "Conversion of sunlight to electricity",
  components = ["Solar Panels", "Inverter", "Battery Storage"],
  performance = {efficiency: "20-40%", power_density: "100-300 W/m²", lifetime: "15-25 years"},
  limitations = ["Weather Dependent", "High Initial Cost"],
  applications = ["Residential Power", "Spacecraft Power"],
  related_systems = ["technology:power_distribution", "technology:storage"];

CREATE energy_systems:nuclear_reactor SET
  name = "Nuclear Reactor",
  description = "Fission reactor for generating electrical power",
  type = "Fission",
  principle = "Nuclear fission of fuel to produce heat",
  components = ["Reactor Core", "Control Rods", "Cooling System"],
  performance = {power_output: "100-1000 MW", efficiency: "30-35%", lifetime: "20-40 years"},
  limitations = ["Radioactive Waste", "High Safety Requirements"],
  applications = ["Base Load Power", "Naval Propulsion"],
  related_systems = ["technology:cooling", "technology:shielding"];

CREATE energy_systems:fusion_reactor SET
  name = "Fusion Reactor",
  description = "Advanced power generation system using controlled fusion reactions",
  type = "Fusion",
  principle = "Fusion of light atomic nuclei to produce energy",
  components = ["Tokamak", "Magnetic Confinement", "Heat Exchanger"],
  performance = {power_output: "1000-10000 MW", efficiency: "40-50%", lifetime: "30-50 years"},
  limitations = ["Technological Challenges", "High Energy Input"],
  applications = ["Future Power Plants", "Space Colonies"],
  related_systems = ["technology:cooling", "technology:materials"];

CREATE energy_systems:wind_turbine SET
  name = "Wind Turbine",
  description = "System for converting wind energy into electrical power",
  type = "Wind",
  principle = "Conversion of kinetic energy from wind to electricity",
  components = ["Rotor Blades", "Generator", "Tower"],
  performance = {capacity_factor: "20-40%", power_output: "1-10 MW", lifetime: "20-30 years"},
  limitations = ["Intermittent Power", "Noise Pollution"],
  applications = ["Onshore Wind Farms", "Offshore Wind Farms"],
  related_systems = ["technology:power_distribution", "technology:storage"];
