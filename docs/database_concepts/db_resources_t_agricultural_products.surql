USE NS project_universe;
USE DB economy;

DEFINE TABLE agricultural_products SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE agricultural_products TYPE string;
DEFINE FIELD description     ON TABLE agricultural_products TYPE string;
DEFINE FIELD category        ON TABLE agricultural_products TYPE string;
DEFINE FIELD production_cost ON TABLE agricultural_products TYPE float;
DEFINE FIELD market_value    ON TABLE agricultural_products TYPE float;
DEFINE FIELD related_fields  ON TABLE agricultural_products TYPE array;

DEFINE INDEX unique_agricultural_products_name ON TABLE agricultural_products COLUMNS name UNIQUE;

-- Example agricultural products
CREATE agricultural_products:wheat SET
  name = "Wheat",
  description = "Cereal grain used for food and feed",
  category = "Grain",
  production_cost = 100.0,
  market_value = 200.0,
  related_fields = ["economy:agriculture", "economy:food"];

CREATE agricultural_products:corn SET
  name = "Corn",
  description = "Staple food crop used for food and industrial products",
  category = "Grain",
  production_cost = 80.0,
  market_value = 150.0,
  related_fields = ["economy:agriculture", "economy:food"];

CREATE agricultural_products:soybeans SET
  name = "Soybeans",
  description = "Legume used for food, feed, and industrial products",
  category = "Legume",
  production_cost = 90.0,
  market_value = 180.0,
  related_fields = ["economy:agriculture", "economy:food"]; 