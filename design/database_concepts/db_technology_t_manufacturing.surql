USE NS project_universe;
USE DB technology;
  
DEFINE TABLE manufacturing SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';
  
DEFINE FIELD name            ON TABLE manufacturing TYPE string;
DEFINE FIELD description     ON TABLE manufacturing TYPE string;
DEFINE FIELD type           ON TABLE manufacturing TYPE string;
DEFINE FIELD principle      ON TABLE manufacturing TYPE string;
DEFINE FIELD components     ON TABLE manufacturing TYPE array;
DEFINE FIELD requirements   ON TABLE manufacturing TYPE array;
DEFINE FIELD performance    ON TABLE manufacturing TYPE object;
DEFINE FIELD limitations    ON TABLE manufacturing TYPE array;
DEFINE FIELD applications   ON TABLE manufacturing TYPE array;
DEFINE FIELD related_systems ON TABLE manufacturing TYPE array;
  
DEFINE INDEX unique_manufacturing_system ON TABLE manufacturing COLUMNS name UNIQUE;
DEFINE INDEX manufacturing_type ON TABLE manufacturing COLUMNS type;
  
-- 3D Printer
CREATE manufacturing:additive_printer SET
  name = "Additive Manufacturing System",
  description = "Advanced 3D printing system for complex part fabrication",
  type = "Additive",
  principle = "Layer-by-layer material deposition",
  components = ["Print head", "Build platform", "Material feed", "Control system"],
  requirements = ["Power supply", "Raw materials", "Environmental control", "Post-processing"],
  performance = {
    resolution: "0.1-1 mm",
    build_volume: "100-1000 mm³",
    print_speed: "10-100 cm³/hour",
    material_types: "Multiple"
  },
  limitations = ["Build time", "Material properties", "Surface finish"],
  applications = ["Part fabrication", "Tool production", "Spare parts"],
  related_systems = ["power:power_distribution", "robotics:manipulator"];
  
-- CNC Machine
CREATE manufacturing:cnc_machine SET
  name = "CNC Manufacturing System",
  description = "Computer-controlled machining system for precision parts",
  type = "Subtractive",
  principle = "Material removal with controlled tooling",
  components = ["Tool changer", "Work holding", "Coolant system", "Control system"],
  requirements = ["Power supply", "Tooling", "Coolant", "Workpiece material"],
  performance = {
    positioning_accuracy: "0.01-0.1 mm",
    spindle_speed: "1000-10000 RPM",
    tool_capacity: "10-50 tools",
    work_volume: "100-1000 mm³"
  },
  limitations = ["Tool wear", "Material constraints", "Setup time"],
  applications = ["Precision parts", "Tool making", "Prototype production"],
  related_systems = ["power:power_distribution", "robotics:manipulator"];
  
-- Foundry System
CREATE manufacturing:foundry SET
  name = "Metal Casting System",
  description = "Advanced metal casting and molding system",
  type = "Casting",
  principle = "Molten metal casting and solidification",
  components = ["Melting furnace", "Mold system", "Pouring system", "Cooling system"],
  requirements = ["Power supply", "Raw materials", "Mold materials", "Temperature control"],
  performance = {
    melt_capacity: "10-1000 kg",
    temperature_range: "500-2000°C",
    casting_accuracy: "0.1-1 mm",
    production_rate: "1-100 parts/day"
  },
  limitations = ["Material constraints", "Cooling time", "Mold life"],
  applications = ["Metal parts", "Complex shapes", "Mass production"],
  related_systems = ["power:power_distribution", "life_support:thermal_control"];
  
-- Assembly Line
CREATE manufacturing:assembly_line SET
  name = "Automated Assembly System",
  description = "Robotic assembly line for component integration",
  type = "Assembly",
  principle = "Automated component assembly and testing",
  components = ["Conveyor system", "Robotic arms", "Quality control", "Testing stations"],
  requirements = ["Power supply", "Component inventory", "Assembly procedures", "Quality standards"],
  performance = {
    assembly_rate: "10-1000 parts/hour",
    quality_rate: "99.9%",
    flexibility: "Multiple product types",
    uptime: "95-99%"
  },
  limitations = ["Component availability", "Changeover time", "Complex assemblies"],
  applications = ["Product assembly", "System integration", "Mass production"],
  related_systems = ["power:power_distribution", "robotics:assembly_bot", "communication:data_processor"]; 