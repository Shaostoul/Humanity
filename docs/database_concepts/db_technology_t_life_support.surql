USE NS project_universe;
USE DB technology;
  
DEFINE TABLE life_support SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';
  
DEFINE FIELD name            ON TABLE life_support TYPE string;
DEFINE FIELD description     ON TABLE life_support TYPE string;
DEFINE FIELD type           ON TABLE life_support TYPE string;
DEFINE FIELD principle      ON TABLE life_support TYPE string;
DEFINE FIELD components     ON TABLE life_support TYPE array;
DEFINE FIELD requirements   ON TABLE life_support TYPE array;
DEFINE FIELD performance    ON TABLE life_support TYPE object;
DEFINE FIELD limitations    ON TABLE life_support TYPE array;
DEFINE FIELD applications   ON TABLE life_support TYPE array;
DEFINE FIELD related_systems ON TABLE life_support TYPE array;
  
DEFINE INDEX unique_life_support_system ON TABLE life_support COLUMNS name UNIQUE;
DEFINE INDEX life_support_type ON TABLE life_support COLUMNS type;
  
-- Air Recycling
CREATE life_support:air_recycling SET
  name = "Air Recycling System",
  description = "System for maintaining breathable atmosphere through recycling",
  type = "Atmospheric",
  principle = "Chemical and physical processes for air purification",
  components = ["CO₂ scrubber", "Oxygen generator", "Particulate filter", "Humidity control"],
  requirements = ["Power supply", "Chemical consumables", "Temperature control", "Monitoring systems"],
  performance = {
    co2_removal_rate: "1-2 kg/day",
    oxygen_generation: "2-4 kg/day",
    air_quality: "99.9% clean",
    reliability: "99.99%"
  },
  limitations = ["Consumable materials", "Power intensive", "Complex maintenance"],
  applications = ["Spacecraft life support", "Space station atmosphere", "Surface habitats"],
  related_systems = ["power:power_distribution", "life_support:thermal_control"];
  
-- Water Recycling
CREATE life_support:water_recycling SET
  name = "Water Recycling System",
  description = "System for recycling and purifying water",
  type = "Water",
  principle = "Physical and chemical water treatment processes",
  components = ["Filtration unit", "Chemical processor", "UV sterilizer", "Storage tanks"],
  requirements = ["Power supply", "Chemical consumables", "Temperature control", "Monitoring systems"],
  performance = {
    recycling_rate: "95-98%",
    water_quality: "Potable standards",
    processing_capacity: "100-1000 L/day",
    reliability: "99.99%"
  },
  limitations = ["Consumable materials", "Power intensive", "Complex maintenance"],
  applications = ["Spacecraft water supply", "Space station water", "Surface base water"],
  related_systems = ["power:power_distribution", "life_support:thermal_control"];
  
-- Thermal Control
CREATE life_support:thermal_control SET
  name = "Thermal Control System",
  description = "System for maintaining optimal temperature conditions",
  type = "Thermal",
  principle = "Heat transfer and temperature regulation",
  components = ["Heat exchanger", "Radiator panels", "Coolant loops", "Temperature sensors"],
  requirements = ["Power supply", "Coolant", "Temperature sensors", "Control systems"],
  performance = {
    cooling_capacity: "10-100 kW",
    temperature_range: "18-25°C",
    stability: "±1°C",
    reliability: "99.99%"
  },
  limitations = ["Power intensive", "External radiator exposure", "Complex maintenance"],
  applications = ["Spacecraft temperature control", "Space station climate", "Surface habitat climate"],
  related_systems = ["power:power_distribution", "life_support:air_recycling"];
  
-- Radiation Protection
CREATE life_support:radiation_protection SET
  name = "Radiation Protection System",
  description = "System for protecting against harmful radiation",
  type = "Radiation",
  principle = "Radiation shielding and monitoring",
  components = ["Shielding material", "Radiation detectors", "Warning system", "Shelter areas"],
  requirements = ["Shielding materials", "Power supply", "Monitoring systems", "Emergency protocols"],
  performance = {
    shielding_factor: "100-1000x",
    detection_sensitivity: "0.1-1 mSv/h",
    response_time: "<1 second",
    reliability: "99.99%"
  },
  limitations = ["Mass intensive", "Limited shielding effectiveness", "Complex maintenance"],
  applications = ["Spacecraft radiation protection", "Space station shielding", "Surface habitat protection"],
  related_systems = ["sensors:radiation_detector", "life_support:thermal_control"]; 