USE NS project_universe;
USE DB science;

DEFINE TABLE biology SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE biology TYPE string;
DEFINE FIELD description     ON TABLE biology TYPE string;
DEFINE FIELD process         ON TABLE biology TYPE string;
DEFINE FIELD requirements    ON TABLE biology TYPE array;
DEFINE FIELD products        ON TABLE biology TYPE array;
DEFINE FIELD conditions      ON TABLE biology TYPE array;
DEFINE FIELD applications    ON TABLE biology TYPE array;
DEFINE FIELD hazards         ON TABLE biology TYPE array;
DEFINE FIELD related_processes ON TABLE biology TYPE array;
DEFINE FIELD optimal_conditions ON TABLE biology TYPE object;
DEFINE FIELD survival_limits ON TABLE biology TYPE object;

DEFINE INDEX unique_biological_process ON TABLE biology COLUMNS name UNIQUE;
DEFINE INDEX process_type ON TABLE biology COLUMNS process;
DEFINE INDEX optimal_conditions ON TABLE biology COLUMNS optimal_conditions;
DEFINE INDEX survival_limits ON TABLE biology COLUMNS survival_limits;

-- Life Support Systems
CREATE biology:plant_growth SET
  name = "Plant Growth",
  description = "Growth and development of plants in controlled environments",
  process = "Photosynthesis and cellular respiration cycle",
  requirements = ["Light", "CO₂", "Water", "Nutrients", "Oxygen"],
  products = ["Biomass", "Oxygen", "Food"],
  conditions = ["Controlled temperature", "Humidity", "Light cycle"],
  applications = ["Food Production", "Oxygen Generation", "CO₂ Removal"],
  hazards = ["Pathogens", "Pests", "Nutrient imbalances"],
  related_processes = ["biology:photosynthesis", "biology:cellular_respiration"],
  optimal_conditions = {
    temperature: "20-30°C",
    humidity: "40-60%",
    light_hours: "12-16",
    co2_level: "400-1000 ppm",
    ph: "6.0-7.0"
  },
  survival_limits = {
    temperature: "-5-40°C",
    humidity: "20-80%",
    light_hours: "8-24",
    co2_level: "200-2000 ppm",
    ph: "5.0-8.0"
  };
CREATE biology:microbial_bioreactor SET
  name = "Microbial Bioreactor",
  description = "Use of microorganisms for waste processing and resource production",
  process = "Aerobic and anaerobic microbial metabolism",
  requirements = ["Organic waste", "Water", "Nutrients", "Oxygen (aerobic)"],
  products = ["Biomass", "CO₂", "Methane (anaerobic)", "Nutrients"],
  conditions = ["Temperature control", "pH balance", "Oxygen levels"],
  applications = ["Waste Processing", "Biomass Production", "Nutrient Recycling"],
  hazards = ["Pathogenic microbes", "Toxic byproducts", "Oxygen depletion"],
  related_processes = ["biology:fermentation", "biology:decomposition"],
  optimal_conditions = {
    temperature: "25-35°C",
    ph: "6.5-7.5",
    oxygen: "2-6 mg/L",
    retention_time: "12-24 hours"
  },
  survival_limits = {
    temperature: "10-45°C",
    ph: "5.0-9.0",
    oxygen: "0-8 mg/L",
    retention_time: "4-48 hours"
  };
CREATE biology:algae_cultivation SET
  name = "Algae Cultivation",
  description = "Growth of algae for biomass and oxygen production",
  process = "Photosynthesis and nutrient uptake",
  requirements = ["Light", "CO₂", "Water", "Nutrients"],
  products = ["Biomass", "Oxygen", "Lipids", "Proteins"],
  conditions = ["Light exposure", "Nutrient levels", "Temperature"],
  applications = ["Biofuel Production", "Oxygen Generation", "Food Supplement"],
  hazards = ["Algal blooms", "Toxin production", "Oxygen depletion"],
  related_processes = ["biology:photosynthesis", "biology:biomass_production"],
  optimal_conditions = {
    temperature: "25-30°C",
    light_intensity: "1000-2000 μmol/m²/s",
    co2_level: "1000-2000 ppm",
    ph: "7.0-8.5"
  },
  survival_limits = {
    temperature: "15-35°C",
    light_intensity: "500-3000 μmol/m²/s",
    co2_level: "400-5000 ppm",
    ph: "6.0-9.0"
  };
CREATE biology:insect_farming SET
  name = "Insect Farming",
  description = "Raising insects for protein production and waste processing",
  process = "Growth and reproduction cycle",
  requirements = ["Organic waste", "Water", "Temperature control", "Ventilation"],
  products = ["Protein", "Fertilizer", "Chitin"],
  conditions = ["Humidity", "Temperature", "Food supply"],
  applications = ["Food Production", "Waste Processing", "Resource Recycling"],
  hazards = ["Disease", "Escape risk", "Allergens"],
  related_processes = ["biology:decomposition", "biology:biomass_production"],
  optimal_conditions = {
    temperature: "25-30°C",
    humidity: "60-70%",
    ventilation: "5-10 air changes/hour",
    light_hours: "12-16"
  },
  survival_limits = {
    temperature: "20-35°C",
    humidity: "40-80%",
    ventilation: "3-15 air changes/hour",
    light_hours: "8-24"
  };
CREATE biology:fish_aquaculture SET
  name = "Fish Aquaculture",
  description = "Raising fish in controlled aquatic environments",
  process = "Growth and reproduction in water",
  requirements = ["Water", "Oxygen", "Food", "Temperature control"],
  products = ["Protein", "Nutrients", "Biomass"],
  conditions = ["Water quality", "Oxygen levels", "Temperature"],
  applications = ["Food Production", "Nutrient Recycling", "Ecosystem Balance"],
  hazards = ["Disease", "Water quality issues", "Oxygen depletion"],
  related_processes = ["biology:nutrient_cycling", "biology:ecosystem_balance"],
  optimal_conditions = {
    temperature: "20-25°C",
    oxygen: "6-8 mg/L",
    ph: "6.5-7.5",
    ammonia: "<0.5 mg/L"
  },
  survival_limits = {
    temperature: "15-30°C",
    oxygen: "4-10 mg/L",
    ph: "6.0-8.0",
    ammonia: "<2.0 mg/L"
  };
