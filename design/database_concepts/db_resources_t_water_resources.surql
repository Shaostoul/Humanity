USE NS project_universe;
USE DB economy;

DEFINE TABLE water_resources SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE water_resources TYPE string;
DEFINE FIELD description     ON TABLE water_resources TYPE string;
DEFINE FIELD category        ON TABLE water_resources TYPE string;
DEFINE FIELD extraction_cost ON TABLE water_resources TYPE float;
DEFINE FIELD market_value    ON TABLE water_resources TYPE float;
DEFINE FIELD related_fields  ON TABLE water_resources TYPE array;

DEFINE INDEX unique_water_resources_name ON TABLE water_resources COLUMNS name UNIQUE;

-- Example water resources
CREATE water_resources:groundwater SET
  name = "Groundwater",
  description = "Water located beneath the earth's surface in soil pore spaces and in the fractures of rock formations",
  category = "Freshwater",
  extraction_cost = 10.0,
  market_value = 30.0,
  related_fields = ["economy:water_management", "economy:agriculture"];

CREATE water_resources:surface_water SET
  name = "Surface Water",
  description = "Water that collects on the surface of the ground, including rivers, lakes, and reservoirs",
  category = "Freshwater",
  extraction_cost = 15.0,
  market_value = 40.0,
  related_fields = ["economy:water_management", "economy:recreation"];

CREATE water_resources:desalinated_water SET
  name = "Desalinated Water",
  description = "Water that has been purified by removing salts and other impurities",
  category = "Processed",
  extraction_cost = 50.0,
  market_value = 100.0,
  related_fields = ["economy:water_management", "economy:technology"]; 