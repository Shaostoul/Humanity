USE NS project_universe;
USE DB demographics;

DEFINE TABLE population_data SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE population_data TYPE string;
DEFINE FIELD description     ON TABLE population_data TYPE string;
DEFINE FIELD category        ON TABLE population_data TYPE string;
DEFINE FIELD population_size ON TABLE population_data TYPE int;
DEFINE FIELD growth_rate     ON TABLE population_data TYPE float;
DEFINE FIELD related_fields  ON TABLE population_data TYPE array;

DEFINE INDEX unique_population_data_name ON TABLE population_data COLUMNS name UNIQUE;

-- Example population data
CREATE population_data:urban SET
  name = "Urban",
  description = "Population living in urban areas",
  category = "Area Type",
  population_size = 5000000,
  growth_rate = 1.5,
  related_fields = ["demographics:urbanization", "demographics:infrastructure"];

CREATE population_data:rural SET
  name = "Rural",
  description = "Population living in rural areas",
  category = "Area Type",
  population_size = 2000000,
  growth_rate = 0.5,
  related_fields = ["demographics:agriculture", "demographics:development"];

CREATE population_data:suburban SET
  name = "Suburban",
  description = "Population living in suburban areas",
  category = "Area Type",
  population_size = 3000000,
  growth_rate = 1.0,
  related_fields = ["demographics:urbanization", "demographics:living"]; 