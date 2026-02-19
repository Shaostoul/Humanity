USE NS project_universe;
USE DB economy;

DEFINE TABLE mineral_resources SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE mineral_resources TYPE string;
DEFINE FIELD description     ON TABLE mineral_resources TYPE string;
DEFINE FIELD category        ON TABLE mineral_resources TYPE string;
DEFINE FIELD extraction_cost ON TABLE mineral_resources TYPE float;
DEFINE FIELD market_value    ON TABLE mineral_resources TYPE float;
DEFINE FIELD related_fields  ON TABLE mineral_resources TYPE array;

DEFINE INDEX unique_mineral_resources_name ON TABLE mineral_resources COLUMNS name UNIQUE;

-- Example mineral resources
CREATE mineral_resources:gold SET
  name = "Gold",
  description = "Precious metal used in jewelry and electronics",
  category = "Precious Metal",
  extraction_cost = 500.0,
  market_value = 1500.0,
  related_fields = ["economy:mining", "economy:manufacturing"];

CREATE mineral_resources:copper SET
  name = "Copper",
  description = "Base metal used in electrical wiring and plumbing",
  category = "Base Metal",
  extraction_cost = 200.0,
  market_value = 600.0,
  related_fields = ["economy:mining", "economy:construction"];

CREATE mineral_resources:coal SET
  name = "Coal",
  description = "Fossil fuel used for electricity generation and steel production",
  category = "Fossil Fuel",
  extraction_cost = 100.0,
  market_value = 300.0,
  related_fields = ["economy:energy", "economy:manufacturing"]; 