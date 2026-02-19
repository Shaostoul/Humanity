USE NS project_universe;
USE DB economy;

DEFINE TABLE ugc_items SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE ugc_items TYPE string;
DEFINE FIELD description     ON TABLE ugc_items TYPE string;
DEFINE FIELD category        ON TABLE ugc_items TYPE string;
DEFINE FIELD creation_cost   ON TABLE ugc_items TYPE float;
DEFINE FIELD market_value    ON TABLE ugc_items TYPE float;
DEFINE FIELD related_fields  ON TABLE ugc_items TYPE array;

DEFINE INDEX unique_ugc_items_name ON TABLE ugc_items COLUMNS name UNIQUE;

-- Example UGC items
CREATE ugc_items:custom_mug SET
  name = "Custom Mug",
  description = "A mug designed by users with personalized graphics",
  category = "Kitchenware",
  creation_cost = 3.0,
  market_value = 12.0,
  related_fields = ["economy:retail", "economy:design"];

CREATE ugc_items:graphic_tee SET
  name = "Graphic Tee",
  description = "A t-shirt featuring user-generated designs",
  category = "Apparel",
  creation_cost = 8.0,
  market_value = 20.0,
  related_fields = ["economy:apparel", "economy:design"];

CREATE ugc_items:photo_book SET
  name = "Photo Book",
  description = "A book compiled with user-uploaded photos",
  category = "Photography",
  creation_cost = 15.0,
  market_value = 40.0,
  related_fields = ["economy:photography", "economy:printing"];
