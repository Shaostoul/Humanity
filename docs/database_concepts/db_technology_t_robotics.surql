USE NS project_universe;
USE DB technology;

DEFINE TABLE robotics SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE robotics TYPE string;
DEFINE FIELD description     ON TABLE robotics TYPE string;
DEFINE FIELD type            ON TABLE robotics TYPE string;
DEFINE FIELD components      ON TABLE robotics TYPE array;
DEFINE FIELD capabilities    ON TABLE robotics TYPE array;
DEFINE FIELD applications    ON TABLE robotics TYPE array;
DEFINE FIELD limitations     ON TABLE robotics TYPE array;
DEFINE FIELD related_systems ON TABLE robotics TYPE array;

DEFINE INDEX unique_robotics_name ON TABLE robotics COLUMNS name UNIQUE;

-- Example robotic systems
CREATE robotics:autonomous_drone SET
  name = "Autonomous Drone",
  description = "Aerial vehicle capable of autonomous flight and navigation",
  type = "Aerial",
  components = ["Propellers", "GPS", "Camera", "Sensors"],
  capabilities = ["Autonomous Navigation", "Surveillance", "Mapping"],
  applications = ["Reconnaissance", "Delivery", "Surveying"],
  limitations = ["Limited Battery Life", "Weather Sensitivity"],
  related_systems = ["technology:communication", "technology:power"];

CREATE robotics:industrial_robot SET
  name = "Industrial Robot",
  description = "Programmable robot used in manufacturing and assembly lines",
  type = "Industrial",
  components = ["Robotic Arm", "Sensors", "Control System"],
  capabilities = ["Precision Assembly", "Welding", "Material Handling"],
  applications = ["Manufacturing", "Assembly", "Quality Control"],
  limitations = ["High Initial Cost", "Limited Flexibility"],
  related_systems = ["technology:manufacturing", "technology:power"];

CREATE robotics:service_robot SET
  name = "Service Robot",
  description = "Robot designed to assist humans in service industries",
  type = "Service",
  components = ["Wheels", "Sensors", "AI System"],
  capabilities = ["Customer Interaction", "Navigation", "Task Automation"],
  applications = ["Hospitality", "Healthcare", "Retail"],
  limitations = ["Limited Understanding", "Dependence on AI"],
  related_systems = ["technology:AI", "technology:communication"];

CREATE robotics:exploration_rover SET
  name = "Exploration Rover",
  description = "Robotic vehicle designed for exploration of remote or hazardous environments",
  type = "Exploration",
  components = ["Wheels", "Cameras", "Sensors", "Communication System"],
  capabilities = ["Terrain Navigation", "Data Collection", "Remote Operation"],
  applications = ["Space Exploration", "Arctic Research", "Disaster Response"],
  limitations = ["Limited Power Supply", "Communication Delays"],
  related_systems = ["technology:communication", "technology:power"];
