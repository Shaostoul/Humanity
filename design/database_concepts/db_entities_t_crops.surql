USE NS project_universe;
USE DB entities;

DEFINE TABLE crops SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD crop_id         ON TABLE crops TYPE string;
DEFINE FIELD name            ON TABLE crops TYPE string;
DEFINE FIELD type            ON TABLE crops TYPE string;
DEFINE FIELD growth_cycle    ON TABLE crops TYPE string;
DEFINE FIELD yield           ON TABLE crops TYPE string;

DEFINE INDEX unique_crop_id ON TABLE crops COLUMNS crop_id UNIQUE;

-- Example crops
CREATE crops:crop_001 SET
  crop_id = "crop_001",
  name = "Wheat",
  type = "Cereal",
  growth_cycle = "Annual",
  yield = "High";

CREATE crops:crop_002 SET
  crop_id = "crop_002",
  name = "Tomato",
  type = "Fruit",
  growth_cycle = "Perennial",
  yield = "Moderate";
