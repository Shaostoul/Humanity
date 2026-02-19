USE NS project_universe;
USE DB environment;

DEFINE TABLE climate_data SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE climate_data TYPE string;
DEFINE FIELD description     ON TABLE climate_data TYPE string;
DEFINE FIELD category        ON TABLE climate_data TYPE string;
DEFINE FIELD average_temp    ON TABLE climate_data TYPE float;
DEFINE FIELD precipitation   ON TABLE climate_data TYPE float;
DEFINE FIELD related_fields  ON TABLE climate_data TYPE array;

DEFINE INDEX unique_climate_data_name ON TABLE climate_data COLUMNS name UNIQUE;

-- Example climate data
CREATE climate_data:tropical SET
  name = "Tropical",
  description = "Climate characterized by high temperatures and heavy rainfall",
  category = "Climate Zone",
  average_temp = 30.0,
  precipitation = 200.0,
  related_fields = ["environment:weather", "environment:ecosystems"];

CREATE climate_data:arid SET
  name = "Arid",
  description = "Climate characterized by low rainfall and high evaporation",
  category = "Climate Zone",
  average_temp = 35.0,
  precipitation = 50.0,
  related_fields = ["environment:weather", "environment:deserts"];

CREATE climate_data:temperate SET
  name = "Temperate",
  description = "Climate characterized by moderate temperatures and rainfall",
  category = "Climate Zone",
  average_temp = 15.0,
  precipitation = 100.0,
  related_fields = ["environment:weather", "environment:forests"]; 