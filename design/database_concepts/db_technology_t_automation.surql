USE NS project_universe;
USE DB technology;

DEFINE TABLE automation SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE automation TYPE string;
DEFINE FIELD description     ON TABLE automation TYPE string;
DEFINE FIELD type            ON TABLE automation TYPE string;
DEFINE FIELD components      ON TABLE automation TYPE array;
DEFINE FIELD capabilities    ON TABLE automation TYPE array;
DEFINE FIELD applications    ON TABLE automation TYPE array;
DEFINE FIELD limitations     ON TABLE automation TYPE array;
DEFINE FIELD related_systems ON TABLE automation TYPE array;

DEFINE INDEX unique_automation_name ON TABLE automation COLUMNS name UNIQUE;

-- Example automation systems
CREATE automation:industrial_automation SET
  name = "Industrial Automation",
  description = "Automation of industrial processes using control systems",
  type = "Industrial",
  components = ["PLC", "Sensors", "Actuators"],
  capabilities = ["Process Control", "Monitoring", "Data Collection"],
  applications = ["Manufacturing", "Assembly Lines", "Quality Control"],
  limitations = ["High Initial Cost", "Complex Integration"],
  related_systems = ["technology:robotics", "technology:control"];

CREATE automation:home_automation SET
  name = "Home Automation",
  description = "Automation of home activities using smart devices",
  type = "Residential",
  components = ["Smart Thermostat", "Smart Lights", "Security Cameras"],
  capabilities = ["Remote Control", "Energy Management", "Security Monitoring"],
  applications = ["Smart Homes", "Energy Efficiency", "Home Security"],
  limitations = ["Privacy Concerns", "Interoperability Issues"],
  related_systems = ["technology:IoT", "technology:networking"];

CREATE automation:business_process_automation SET
  name = "Business Process Automation",
  description = "Automation of business processes using software applications",
  type = "Enterprise",
  components = ["Workflow Software", "RPA Tools", "APIs"],
  capabilities = ["Task Automation", "Data Processing", "Workflow Management"],
  applications = ["Finance", "Human Resources", "Customer Service"],
  limitations = ["Implementation Cost", "Change Management"],
  related_systems = ["technology:AI", "technology:cloud_computing"];

CREATE automation:agricultural_automation SET
  name = "Agricultural Automation",
  description = "Automation of agricultural tasks using machinery and technology",
  type = "Agricultural",
  components = ["Drones", "Automated Tractors", "Irrigation Systems"],
  capabilities = ["Precision Farming", "Crop Monitoring", "Resource Management"],
  applications = ["Crop Production", "Livestock Management", "Soil Analysis"],
  limitations = ["High Equipment Cost", "Technical Expertise Required"],
  related_systems = ["technology:IoT", "technology:robotics"];
