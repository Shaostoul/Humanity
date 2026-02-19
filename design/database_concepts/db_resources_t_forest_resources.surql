USE NS project_universe;
USE DB economy;

DEFINE TABLE forest_resources SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE forest_resources TYPE string;
DEFINE FIELD description     ON TABLE forest_resources TYPE string;
DEFINE FIELD category        ON TABLE forest_resources TYPE string;
DEFINE FIELD extraction_cost ON TABLE forest_resources TYPE float;
DEFINE FIELD market_value    ON TABLE forest_resources TYPE float;
DEFINE FIELD related_fields  ON TABLE forest_resources TYPE array;

DEFINE INDEX unique_forest_resources_name ON TABLE forest_resources COLUMNS name UNIQUE;

-- Example forest resources
CREATE forest_resources:timber SET
  name = "Timber",
  description = "Wood prepared for use in building and carpentry",
  category = "Wood",
  extraction_cost = 50.0,
  market_value = 100.0,
  related_fields = ["economy:forestry", "economy:construction"];

CREATE forest_resources:rubber SET
  name = "Rubber",
  description = "Natural polymer harvested from rubber trees",
  category = "Latex",
  extraction_cost = 30.0,
  market_value = 70.0,
  related_fields = ["economy:forestry", "economy:manufacturing"];

CREATE forest_resources:bamboo SET
  name = "Bamboo",
  description = "Fast-growing grass used for construction and crafts",
  category = "Grass",
  extraction_cost = 20.0,
  market_value = 50.0,
  related_fields = ["economy:forestry", "economy:crafts"]; 