USE NS project_universe;
USE DB economy;

DEFINE TABLE communication_technologies SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE communication_technologies TYPE string;
DEFINE FIELD description     ON TABLE communication_technologies TYPE string;
DEFINE FIELD category        ON TABLE communication_technologies TYPE string;
DEFINE FIELD development_cost ON TABLE communication_technologies TYPE float;
DEFINE FIELD market_value    ON TABLE communication_technologies TYPE float;
DEFINE FIELD related_fields  ON TABLE communication_technologies TYPE array;

DEFINE INDEX unique_communication_technologies_name ON TABLE communication_technologies COLUMNS name UNIQUE;

-- Example communication technologies
CREATE communication_technologies:5g_network SET
  name = "5G Network",
  description = "Fifth generation technology standard for cellular networks",
  category = "Wireless",
  development_cost = 100.0,
  market_value = 200.0,
  related_fields = ["economy:telecommunications", "economy:technology"];

CREATE communication_technologies:fiber_optic SET
  name = "Fiber Optic",
  description = "Technology that uses glass or plastic threads to transmit data",
  category = "Wired",
  development_cost = 80.0,
  market_value = 150.0,
  related_fields = ["economy:telecommunications", "economy:infrastructure"];

CREATE communication_technologies:satellite_communication SET
  name = "Satellite Communication",
  description = "Use of satellite technology for telecommunication",
  category = "Satellite",
  development_cost = 120.0,
  market_value = 180.0,
  related_fields = ["economy:telecommunications", "economy:space"]; 