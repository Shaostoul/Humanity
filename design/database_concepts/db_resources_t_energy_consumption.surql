USE NS project_universe;
USE DB economy;

DEFINE TABLE energy_consumption SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE energy_consumption TYPE string;
DEFINE FIELD description     ON TABLE energy_consumption TYPE string;
DEFINE FIELD category        ON TABLE energy_consumption TYPE string;
DEFINE FIELD consumption_cost ON TABLE energy_consumption TYPE float;
DEFINE FIELD market_value    ON TABLE energy_consumption TYPE float;
DEFINE FIELD related_fields  ON TABLE energy_consumption TYPE array;

DEFINE INDEX unique_energy_consumption_name ON TABLE energy_consumption COLUMNS name UNIQUE;

-- Example energy consumption
CREATE energy_consumption:residential SET
  name = "Residential",
  description = "Energy consumed by households for daily activities",
  category = "Sector",
  consumption_cost = 100.0,
  market_value = 200.0,
  related_fields = ["economy:utilities", "economy:living"];

CREATE energy_consumption:industrial SET
  name = "Industrial",
  description = "Energy used by industries for manufacturing and production",
  category = "Sector",
  consumption_cost = 300.0,
  market_value = 600.0,
  related_fields = ["economy:manufacturing", "economy:production"];

CREATE energy_consumption:transportation SET
  name = "Transportation",
  description = "Energy used for transportation of goods and people",
  category = "Sector",
  consumption_cost = 200.0,
  market_value = 400.0,
  related_fields = ["economy:transportation", "economy:logistics"]; 