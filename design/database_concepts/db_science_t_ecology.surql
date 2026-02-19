USE NS project_universe;
USE DB science;
  
DEFINE TABLE ecology SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';
  
DEFINE FIELD name            ON TABLE ecology TYPE string;
DEFINE FIELD description     ON TABLE ecology TYPE string;
DEFINE FIELD type           ON TABLE ecology TYPE string;
DEFINE FIELD components     ON TABLE ecology TYPE array;
DEFINE FIELD interactions   ON TABLE ecology TYPE array;
DEFINE FIELD requirements   ON TABLE ecology TYPE array;
DEFINE FIELD outputs        ON TABLE ecology TYPE array;
DEFINE FIELD stability      ON TABLE ecology TYPE string;
DEFINE FIELD related_systems ON TABLE ecology TYPE array;
DEFINE FIELD environmental_conditions ON TABLE ecology TYPE object;
DEFINE FIELD system_metrics ON TABLE ecology TYPE object;
  
DEFINE INDEX unique_ecological_system ON TABLE ecology COLUMNS name UNIQUE;
DEFINE INDEX system_type ON TABLE ecology COLUMNS type;
DEFINE INDEX environmental_conditions ON TABLE ecology COLUMNS environmental_conditions;
DEFINE INDEX system_metrics ON TABLE ecology COLUMNS system_metrics;
  
-- Life Support Systems
CREATE ecology:closed_ecosystem SET
  name = "Closed Ecosystem",
  description = "Self-contained ecological system with minimal external inputs",
  type = "Life Support",
  components = ["Plants", "Microorganisms", "Water cycle", "Nutrient cycle"],
  interactions = ["Photosynthesis", "Respiration", "Decomposition", "Nutrient recycling"],
  requirements = ["Light", "CO₂", "Water", "Nutrients", "Temperature control"],
  outputs = ["Oxygen", "Food", "Biomass", "Clean water"],
  stability = "Requires careful monitoring and balance",
  related_systems = ["ecology:aquaponics", "ecology:terrestrial_ecosystem"],
  environmental_conditions = {
    temperature: "20-30°C",
    humidity: "40-60%",
    light_hours: "12-16",
    co2_level: "400-1000 ppm",
    ph: "6.0-7.0"
  },
  system_metrics = {
    biomass_production: "kg/m²/day",
    oxygen_generation: "kg/day",
    water_recycling: "L/day",
    nutrient_efficiency: "percentage"
  };
  
CREATE ecology:aquaponics SET
  name = "Aquaponics System",
  description = "Combined system of aquaculture and hydroponics",
  type = "Food Production",
  components = ["Fish", "Plants", "Bacteria", "Water"],
  interactions = ["Nutrient cycling", "Water filtration", "Symbiotic relationships"],
  requirements = ["Water", "Fish feed", "Light", "Temperature control"],
  outputs = ["Fish protein", "Vegetables", "Clean water"],
  stability = "Requires balanced fish-plant ratio",
  related_systems = ["ecology:closed_ecosystem", "ecology:hydroponics"],
  environmental_conditions = {
    water_temp: "20-25°C",
    ph: "6.5-7.0",
    ammonia: "<0.5 mg/L",
    dissolved_oxygen: "6-8 mg/L"
  },
  system_metrics = {
    fish_production: "kg/m³/year",
    plant_production: "kg/m²/year",
    water_recycling: "L/day",
    feed_conversion: "kg feed/kg fish"
  };
  
CREATE ecology:terrestrial_ecosystem SET
  name = "Terrestrial Ecosystem",
  description = "Land-based ecological system with soil and vegetation",
  type = "Natural",
  components = ["Soil", "Plants", "Animals", "Microorganisms"],
  interactions = ["Nutrient cycling", "Water cycle", "Food webs", "Decomposition"],
  requirements = ["Sunlight", "Water", "Nutrients", "Temperature range"],
  outputs = ["Biomass", "Oxygen", "Soil fertility", "Biodiversity"],
  stability = "Resilient but requires maintenance",
  related_systems = ["ecology:closed_ecosystem", "ecology:soil_ecosystem"],
  environmental_conditions = {
    temperature: "15-30°C",
    rainfall: "500-2000 mm/year",
    soil_ph: "5.5-7.5",
    sunlight: "Full to partial"
  },
  system_metrics = {
    biomass_density: "kg/m²",
    species_diversity: "number of species",
    soil_fertility: "nutrient levels",
    water_retention: "percentage"
  };
  
CREATE ecology:soil_ecosystem SET
  name = "Soil Ecosystem",
  description = "Complex community of organisms in soil",
  type = "Microbial",
  components = ["Bacteria", "Fungi", "Nematodes", "Organic matter"],
  interactions = ["Nutrient cycling", "Decomposition", "Symbiosis"],
  requirements = ["Organic matter", "Water", "Oxygen", "Temperature"],
  outputs = ["Nutrients", "Soil structure", "Carbon sequestration"],
  stability = "Sensitive to disturbance",
  related_systems = ["ecology:terrestrial_ecosystem", "ecology:microbial_community"],
  environmental_conditions = {
    moisture: "20-60%",
    temperature: "15-30°C",
    ph: "6.0-7.5",
    organic_matter: "2-5%"
  },
  system_metrics = {
    microbial_biomass: "mg/g soil",
    nutrient_cycling: "rate/day",
    soil_structure: "aggregate stability",
    decomposition_rate: "g/day"
  };
  
CREATE ecology:microbial_community SET
  name = "Microbial Community",
  description = "Complex network of microorganisms in a specific environment",
  type = "Microbial",
  components = ["Bacteria", "Archaea", "Fungi", "Protists"],
  interactions = ["Competition", "Cooperation", "Nutrient cycling", "Biofilm formation"],
  requirements = ["Nutrients", "Water", "Temperature", "pH"],
  outputs = ["Biomass", "Metabolites", "Nutrient cycling"],
  stability = "Highly adaptable",
  related_systems = ["ecology:soil_ecosystem", "ecology:closed_ecosystem"],
  environmental_conditions = {
    temperature: "20-37°C",
    ph: "6.0-8.0",
    oxygen: "Aerobic to anaerobic",
    nutrient_availability: "High to low"
  },
  system_metrics = {
    species_richness: "number of species",
    biomass_production: "mg/L/day",
    metabolic_activity: "rate of substrate use",
    community_stability: "resistance to change"
  }; 