USE NS project_universe;
USE DB economy;

DEFINE TABLE virtual_goods SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE virtual_goods TYPE string;
DEFINE FIELD description     ON TABLE virtual_goods TYPE string;
DEFINE FIELD category        ON TABLE virtual_goods TYPE string;
DEFINE FIELD production_cost ON TABLE virtual_goods TYPE float;
DEFINE FIELD market_value    ON TABLE virtual_goods TYPE float;
DEFINE FIELD related_fields  ON TABLE virtual_goods TYPE array;

DEFINE INDEX unique_virtual_goods_name ON TABLE virtual_goods COLUMNS name UNIQUE;

-- Example virtual goods
CREATE virtual_goods:software_license SET
  name = "Software License",
  description = "A legal instrument governing the use or redistribution of software",
  category = "Software",
  production_cost = 0.0,
  market_value = 100.0,
  related_fields = ["economy:technology", "economy:law"];

CREATE virtual_goods:ebook SET
  name = "Ebook",
  description = "A book publication made available in digital form, consisting of text, images, or both",
  category = "Digital Media",
  production_cost = 0.0,
  market_value = 10.0,
  related_fields = ["economy:publishing", "economy:media"];

CREATE virtual_goods:virtual_currency SET
  name = "Virtual Currency",
  description = "A type of unregulated digital currency that is only available in electronic form",
  category = "Finance",
  production_cost = 0.0,
  market_value = 1.0,
  related_fields = ["economy:finance", "economy:cryptocurrency"];
