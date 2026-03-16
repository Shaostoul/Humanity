USE NS project_universe;
USE DB economy;

DEFINE TABLE infrastructure_projects SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE infrastructure_projects TYPE string;
DEFINE FIELD description     ON TABLE infrastructure_projects TYPE string;
DEFINE FIELD category        ON TABLE infrastructure_projects TYPE string;
DEFINE FIELD construction_cost ON TABLE infrastructure_projects TYPE float;
DEFINE FIELD market_value    ON TABLE infrastructure_projects TYPE float;
DEFINE FIELD related_fields  ON TABLE infrastructure_projects TYPE array;

DEFINE INDEX unique_infrastructure_projects_name ON TABLE infrastructure_projects COLUMNS name UNIQUE;

-- Example infrastructure projects
CREATE infrastructure_projects:highway_construction SET
  name = "Highway Construction",
  description = "Development of a major road for vehicular traffic",
  category = "Transportation",
  construction_cost = 500.0,
  market_value = 1000.0,
  related_fields = ["economy:transportation", "economy:construction"];

CREATE infrastructure_projects:bridge_building SET
  name = "Bridge Building",
  description = "Construction of a bridge to span physical obstacles",
  category = "Transportation",
  construction_cost = 300.0,
  market_value = 600.0,
  related_fields = ["economy:transportation", "economy:engineering"];

CREATE infrastructure_projects:power_plant SET
  name = "Power Plant",
  description = "Facility for generating electric power",
  category = "Energy",
  construction_cost = 800.0,
  market_value = 1500.0,
  related_fields = ["economy:energy", "economy:utilities"]; 