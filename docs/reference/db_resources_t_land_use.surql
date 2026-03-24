USE NS project_universe;
USE DB economy;

DEFINE TABLE land_use SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE land_use TYPE string;
DEFINE FIELD description     ON TABLE land_use TYPE string;
DEFINE FIELD category        ON TABLE land_use TYPE string;
DEFINE FIELD usage_cost      ON TABLE land_use TYPE float;
DEFINE FIELD market_value    ON TABLE land_use TYPE float;
DEFINE FIELD related_fields  ON TABLE land_use TYPE array;

DEFINE INDEX unique_land_use_name ON TABLE land_use COLUMNS name UNIQUE;

-- Example land use
CREATE land_use:agricultural SET
  name = "Agricultural",
  description = "Land used for farming and cultivation of crops",
  category = "Agriculture",
  usage_cost = 50.0,
  market_value = 150.0,
  related_fields = ["economy:agriculture", "economy:food"];

CREATE land_use:residential SET
  name = "Residential",
  description = "Land used for housing and residential buildings",
  category = "Housing",
  usage_cost = 100.0,
  market_value = 300.0,
  related_fields = ["economy:real_estate", "economy:living"];

CREATE land_use:commercial SET
  name = "Commercial",
  description = "Land used for business and commercial activities",
  category = "Business",
  usage_cost = 200.0,
  market_value = 500.0,
  related_fields = ["economy:business", "economy:commerce"]; 