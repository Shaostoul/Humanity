USE NS project_universe;
USE DB economy;

DEFINE TABLE crafted_items SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE crafted_items TYPE string;
DEFINE FIELD description     ON TABLE crafted_items TYPE string;
DEFINE FIELD category        ON TABLE crafted_items TYPE string;
DEFINE FIELD crafting_cost   ON TABLE crafted_items TYPE float;
DEFINE FIELD market_value    ON TABLE crafted_items TYPE float;
DEFINE FIELD related_fields  ON TABLE crafted_items TYPE array;

DEFINE INDEX unique_crafted_items_name ON TABLE crafted_items COLUMNS name UNIQUE;

-- Example crafted items
CREATE crafted_items:wooden_chair SET
  name = "Wooden Chair",
  description = "A handcrafted chair made from oak wood",
  category = "Furniture",
  crafting_cost = 50.0,
  market_value = 150.0,
  related_fields = ["economy:woodworking", "economy:retail"];

CREATE crafted_items:leather_wallet SET
  name = "Leather Wallet",
  description = "A wallet crafted from genuine leather",
  category = "Accessories",
  crafting_cost = 20.0,
  market_value = 60.0,
  related_fields = ["economy:leatherworking", "economy:retail"];

CREATE crafted_items:ceramic_vase SET
  name = "Ceramic Vase",
  description = "A decorative vase made from ceramic clay",
  category = "Decor",
  crafting_cost = 30.0,
  market_value = 90.0,
  related_fields = ["economy:pottery", "economy:art"];
