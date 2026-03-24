USE NS project_universe;
USE DB economy;

DEFINE TABLE economic_indicators SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE economic_indicators TYPE string;
DEFINE FIELD description     ON TABLE economic_indicators TYPE string;
DEFINE FIELD category        ON TABLE economic_indicators TYPE string;
DEFINE FIELD value           ON TABLE economic_indicators TYPE float;
DEFINE FIELD change_rate     ON TABLE economic_indicators TYPE float;
DEFINE FIELD related_fields  ON TABLE economic_indicators TYPE array;

DEFINE INDEX unique_economic_indicators_name ON TABLE economic_indicators COLUMNS name UNIQUE;

-- Example economic indicators
CREATE economic_indicators:gdp SET
  name = "GDP",
  description = "Gross Domestic Product, a measure of a country's economic performance",
  category = "Macroeconomic",
  value = 21000.0,
  change_rate = 2.5,
  related_fields = ["economy:macroeconomics", "economy:finance"];

CREATE economic_indicators:inflation_rate SET
  name = "Inflation Rate",
  description = "Rate at which the general level of prices for goods and services is rising",
  category = "Macroeconomic",
  value = 1.8,
  change_rate = 0.2,
  related_fields = ["economy:macroeconomics", "economy:finance"];

CREATE economic_indicators:unemployment_rate SET
  name = "Unemployment Rate",
  description = "Percentage of the labor force that is jobless and actively looking for employment",
  category = "Labor Market",
  value = 4.0,
  change_rate = -0.1,
  related_fields = ["economy:labor", "economy:employment"]; 