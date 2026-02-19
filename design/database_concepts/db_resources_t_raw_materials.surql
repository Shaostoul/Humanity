USE NS project_universe;
USE DB economy;

DEFINE TABLE raw_materials SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE raw_materials TYPE string;
DEFINE FIELD description     ON TABLE raw_materials TYPE string;
DEFINE FIELD category        ON TABLE raw_materials TYPE string;
DEFINE FIELD extraction_cost ON TABLE raw_materials TYPE float;
DEFINE FIELD market_value    ON TABLE raw_materials TYPE float;
DEFINE FIELD related_fields  ON TABLE raw_materials TYPE array;

DEFINE INDEX unique_raw_materials_name ON TABLE raw_materials COLUMNS name UNIQUE;

-- Example raw materials
CREATE raw_materials:iron_ore SET
  name = "Iron Ore",
  description = "Naturally occurring rock containing iron minerals",
  category = "Metal",
  extraction_cost = 50.0,
  market_value = 100.0,
  related_fields = ["economy:mining", "economy:manufacturing"];

CREATE raw_materials:crude_oil SET
  name = "Crude Oil",
  description = "Unrefined petroleum extracted from the ground",
  category = "Energy",
  extraction_cost = 30.0,
  market_value = 70.0,
  related_fields = ["economy:energy", "economy:refining"];

CREATE raw_materials:timber SET
  name = "Timber",
  description = "Wood prepared for use in building and carpentry",
  category = "Wood",
  extraction_cost = 20.0,
  market_value = 50.0,
  related_fields = ["economy:forestry", "economy:construction"];
