USE NS project_universe;
USE DB economy;

DEFINE TABLE recycleable_materials SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE recycleable_materials TYPE string;
DEFINE FIELD description     ON TABLE recycleable_materials TYPE string;
DEFINE FIELD category        ON TABLE recycleable_materials TYPE string;
DEFINE FIELD recycling_value ON TABLE recycleable_materials TYPE float;
DEFINE FIELD market_demand   ON TABLE recycleable_materials TYPE float;
DEFINE FIELD related_fields  ON TABLE recycleable_materials TYPE array;

DEFINE INDEX unique_recycleable_materials_name ON TABLE recycleable_materials COLUMNS name UNIQUE;

-- Example recyclable materials
CREATE recycleable_materials:aluminum_cans SET
  name = "Aluminum Cans",
  description = "Used beverage cans made of aluminum, suitable for recycling",
  category = "Metal",
  recycling_value = 0.5,
  market_demand = 0.9,
  related_fields = ["economy:waste_management", "economy:manufacturing"];

CREATE recycleable_materials:glass_bottles SET
  name = "Glass Bottles",
  description = "Used glass bottles, suitable for recycling",
  category = "Glass",
  recycling_value = 0.3,
  market_demand = 0.85,
  related_fields = ["economy:waste_management", "economy:manufacturing"];

CREATE recycleable_materials:plastic_bottles SET
  name = "Plastic Bottles",
  description = "Used plastic bottles, suitable for recycling",
  category = "Plastic",
  recycling_value = 0.2,
  market_demand = 0.8,
  related_fields = ["economy:waste_management", "economy:manufacturing"];
