USE NS project_universe;
USE DB technology;
  
DEFINE TABLE materials SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';
  
DEFINE FIELD name            ON TABLE materials TYPE string;
DEFINE FIELD description     ON TABLE materials TYPE string;
DEFINE FIELD type           ON TABLE materials TYPE string;
DEFINE FIELD principle      ON TABLE materials TYPE string;
DEFINE FIELD components     ON TABLE materials TYPE array;
DEFINE FIELD requirements   ON TABLE materials TYPE array;
DEFINE FIELD performance    ON TABLE materials TYPE object;
DEFINE FIELD limitations    ON TABLE materials TYPE array;
DEFINE FIELD applications   ON TABLE materials TYPE array;
DEFINE FIELD related_systems ON TABLE materials TYPE array;
  
DEFINE INDEX unique_material_system ON TABLE materials COLUMNS name UNIQUE;
DEFINE INDEX material_type ON TABLE materials COLUMNS type;
  
-- Ore Processing
CREATE materials:ore_processor SET
  name = "Ore Processing System",
  description = "Advanced system for extracting and refining raw materials",
  type = "Extraction",
  principle = "Physical and chemical separation of valuable materials",
  components = ["Crusher", "Separator", "Refiner", "Waste handler"],
  requirements = ["Power supply", "Raw ore", "Processing chemicals", "Water supply"],
  performance = {
    processing_rate: "1-100 tons/hour",
    recovery_rate: "80-95%",
    purity: "90-99.9%",
    waste_reduction: "50-90%"
  },
  limitations = ["Ore quality", "Chemical consumption", "Environmental impact"],
  applications = ["Metal extraction", "Mineral processing", "Resource refinement"],
  related_systems = ["power:power_distribution", "manufacturing:foundry"];
  
-- Material Synthesis
CREATE materials:material_synthesizer SET
  name = "Material Synthesis System",
  description = "Advanced system for creating engineered materials",
  type = "Synthesis",
  principle = "Chemical and physical material synthesis",
  components = ["Reactor", "Catalyst system", "Purification unit", "Quality control"],
  requirements = ["Power supply", "Raw materials", "Catalysts", "Temperature control"],
  performance = {
    synthesis_rate: "1-100 kg/hour",
    purity: "99.9-99.999%",
    yield: "80-95%",
    material_types: "Multiple"
  },
  limitations = ["Reaction conditions", "Catalyst life", "Energy requirements"],
  applications = ["Advanced materials", "Composites", "Specialized alloys"],
  related_systems = ["power:power_distribution", "manufacturing:additive_printer"];
  
-- Recycling System
CREATE materials:recycling_system SET
  name = "Material Recycling System",
  description = "Comprehensive system for material recovery and reuse",
  type = "Recycling",
  principle = "Material separation and reprocessing",
  components = ["Sorting system", "Shredder", "Separator", "Refiner"],
  requirements = ["Power supply", "Waste materials", "Processing chemicals", "Water supply"],
  performance = {
    processing_rate: "1-100 tons/hour",
    recovery_rate: "70-90%",
    material_quality: "80-95%",
    energy_efficiency: "40-60%"
  },
  limitations = ["Material contamination", "Energy consumption", "Quality degradation"],
  applications = ["Waste processing", "Resource recovery", "Closed-loop manufacturing"],
  related_systems = ["power:power_distribution", "manufacturing:assembly_line"];
  
-- Material Testing
CREATE materials:material_tester SET
  name = "Material Testing System",
  description = "Advanced system for material characterization and testing",
  type = "Testing",
  principle = "Physical and chemical material analysis",
  components = ["Test chamber", "Sensors", "Data acquisition", "Analysis system"],
  requirements = ["Power supply", "Test specimens", "Environmental control", "Calibration"],
  performance = {
    test_types: "Multiple",
    accuracy: "0.1-1%",
    throughput: "10-100 tests/day",
    data_quality: "High"
  },
  limitations = ["Test duration", "Sample size", "Environmental constraints"],
  applications = ["Quality control", "Material development", "Failure analysis"],
  related_systems = ["power:power_distribution", "sensors:environmental_sensor", "communication:data_processor"]; 