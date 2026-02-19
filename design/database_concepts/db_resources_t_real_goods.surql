USE NS project_universe;
USE DB economy;

DEFINE TABLE real_goods SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE real_goods TYPE string;
DEFINE FIELD description     ON TABLE real_goods TYPE string;
DEFINE FIELD category        ON TABLE real_goods TYPE string;
DEFINE FIELD production_cost ON TABLE real_goods TYPE float;
DEFINE FIELD market_value    ON TABLE real_goods TYPE float;
DEFINE FIELD related_fields  ON TABLE real_goods TYPE array;

DEFINE INDEX unique_real_goods_name ON TABLE real_goods COLUMNS name UNIQUE;

-- Example real goods
CREATE real_goods:automobile SET
  name = "Automobile",
  description = "A wheeled motor vehicle used for transportation",
  category = "Transportation",
  production_cost = 20000.0,
  market_value = 30000.0,
  related_fields = ["economy:manufacturing", "economy:transportation"];

CREATE real_goods:smartphone SET
  name = "Smartphone",
  description = "A portable device that combines mobile telephone and computing functions",
  category = "Electronics",
  production_cost = 200.0,
  market_value = 800.0,
  related_fields = ["economy:technology", "economy:communication"];

CREATE real_goods:furniture SET
  name = "Furniture",
  description = "Large movable equipment used to make a house, office, or other space suitable for living or working",
  category = "Home Goods",
  production_cost = 500.0,
  market_value = 1500.0,
  related_fields = ["economy:manufacturing", "economy:retail"];
