USE NS project_universe;
USE DB technology;
  
DEFINE TABLE power SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';
  
DEFINE FIELD name            ON TABLE power TYPE string;
DEFINE FIELD description     ON TABLE power TYPE string;
DEFINE FIELD type           ON TABLE power TYPE string;
DEFINE FIELD principle      ON TABLE power TYPE string;
DEFINE FIELD components     ON TABLE power TYPE array;
DEFINE FIELD requirements   ON TABLE power TYPE array;
DEFINE FIELD performance    ON TABLE power TYPE object;
DEFINE FIELD limitations    ON TABLE power TYPE array;
DEFINE FIELD applications   ON TABLE power TYPE array;
DEFINE FIELD related_systems ON TABLE power TYPE array;
  
DEFINE INDEX unique_power_system ON TABLE power COLUMNS name UNIQUE;
DEFINE INDEX power_type ON TABLE power COLUMNS type;
  
-- Solar Power
CREATE power:solar_array SET
  name = "Solar Array",
  description = "Photovoltaic system for converting solar radiation to electricity",
  type = "Renewable",
  principle = "Photovoltaic effect - conversion of light to electrical energy",
  components = ["Solar panels", "Power conditioning unit", "Mounting structure", "Battery storage"],
  requirements = ["Sunlight", "Orientation control", "Temperature management", "Radiation protection"],
  performance = {
    efficiency: "20-40%",
    power_density: "100-300 W/m²",
    lifetime: "15-25 years",
    degradation_rate: "0.5-1% per year"
  },
  limitations = ["Dependent on sunlight", "Degradation over time", "Large surface area needed"],
  applications = ["Spacecraft power", "Space station power", "Surface base power"],
  related_systems = ["power:power_distribution", "life_support:thermal_control"];
  
-- Nuclear Power
CREATE power:nuclear_reactor SET
  name = "Nuclear Reactor",
  description = "Fission reactor for generating electrical power",
  type = "Nuclear",
  principle = "Controlled nuclear fission chain reaction",
  components = ["Reactor core", "Control rods", "Heat exchanger", "Turbine system"],
  requirements = ["Nuclear fuel", "Coolant", "Control systems", "Radiation shielding"],
  performance = {
    power_output: "100-1000 MW",
    efficiency: "30-35%",
    lifetime: "20-40 years",
    refueling_interval: "1-2 years"
  },
  limitations = ["Radiation hazards", "Complex maintenance", "High initial cost"],
  applications = ["Large spacecraft power", "Space station power", "Surface base power"],
  related_systems = ["life_support:radiation_protection", "power:power_distribution"];
  
-- Fusion Power
CREATE power:fusion_reactor SET
  name = "Fusion Reactor",
  description = "Advanced power generation using controlled fusion reactions",
  type = "Fusion",
  principle = "Magnetic confinement fusion with energy extraction",
  components = ["Fusion chamber", "Magnetic coils", "Heat exchanger", "Power conversion"],
  requirements = ["Fusion fuel", "High power supply", "Magnetic confinement", "Cooling system"],
  performance = {
    power_output: "1000-10000 MW",
    efficiency: "40-50%",
    lifetime: "30-50 years",
    fuel_consumption: "Very low"
  },
  limitations = ["Experimental technology", "Complex magnetic systems", "High development cost"],
  applications = ["Advanced spacecraft", "Large space stations", "Interstellar missions"],
  related_systems = ["sensors:plasma_diagnostics", "power:power_distribution"];
  
-- Power Distribution
CREATE power:power_distribution SET
  name = "Power Distribution System",
  description = "System for distributing and managing electrical power",
  type = "Distribution",
  principle = "Electrical power transmission and control",
  components = ["Power lines", "Transformers", "Switches", "Control systems"],
  requirements = ["Power source", "Insulation", "Cooling", "Protection systems"],
  performance = {
    efficiency: "95-98%",
    voltage_range: "100V-1000V",
    current_capacity: "100-1000A",
    reliability: "99.9%"
  },
  limitations = ["Power losses", "Heat generation", "Complex maintenance"],
  applications = ["Spacecraft power distribution", "Space station power management", "Surface base power"],
  related_systems = ["power:solar_array", "power:nuclear_reactor", "power:fusion_reactor"]; 