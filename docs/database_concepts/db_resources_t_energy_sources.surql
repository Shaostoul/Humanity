USE NS project_universe;
USE DB economy;

DEFINE TABLE energy_sources SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE energy_sources TYPE string;
DEFINE FIELD description     ON TABLE energy_sources TYPE string;
DEFINE FIELD category        ON TABLE energy_sources TYPE string;
DEFINE FIELD production_cost ON TABLE energy_sources TYPE float;
DEFINE FIELD market_value    ON TABLE energy_sources TYPE float;
DEFINE FIELD related_fields  ON TABLE energy_sources TYPE array;

DEFINE INDEX unique_energy_sources_name ON TABLE energy_sources COLUMNS name UNIQUE;

-- Example energy sources
CREATE energy_sources:solar_power SET
  name = "Solar Power",
  description = "Energy harnessed from the sun's rays",
  category = "Renewable",
  production_cost = 20.0,
  market_value = 50.0,
  related_fields = ["economy:renewable_energy", "economy:technology"];

CREATE energy_sources:wind_power SET
  name = "Wind Power",
  description = "Energy generated from wind turbines",
  category = "Renewable",
  production_cost = 25.0,
  market_value = 60.0,
  related_fields = ["economy:renewable_energy", "economy:technology"];

CREATE energy_sources:natural_gas SET
  name = "Natural Gas",
  description = "Fossil fuel used for heating and electricity",
  category = "Non-renewable",
  production_cost = 15.0,
  market_value = 40.0,
  related_fields = ["economy:energy", "economy:utilities"]; 