USE NS project_universe;
USE DB economy;

DEFINE TABLE ugc_sales SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE ugc_sales TYPE string;
DEFINE FIELD description     ON TABLE ugc_sales TYPE string;
DEFINE FIELD category        ON TABLE ugc_sales TYPE string;
DEFINE FIELD production_cost ON TABLE ugc_sales TYPE float;
DEFINE FIELD market_value    ON TABLE ugc_sales TYPE float;
DEFINE FIELD related_fields  ON TABLE ugc_sales TYPE array;

DEFINE INDEX unique_ugc_sales_name ON TABLE ugc_sales COLUMNS name UNIQUE;

-- Example UGC sales
CREATE ugc_sales:custom_tshirt SET
  name = "Custom T-Shirt",
  description = "A t-shirt designed by users and printed on demand",
  category = "Apparel",
  production_cost = 10.0,
  market_value = 25.0,
  related_fields = ["economy:retail", "economy:design"];

CREATE ugc_sales:art_print SET
  name = "Art Print",
  description = "A high-quality print of user-generated artwork",
  category = "Art",
  production_cost = 5.0,
  market_value = 20.0,
  related_fields = ["economy:art", "economy:printing"];

CREATE ugc_sales:music_track SET
  name = "Music Track",
  description = "A music track created and sold by independent artists",
  category = "Music",
  production_cost = 0.0,
  market_value = 1.0,
  related_fields = ["economy:music", "economy:entertainment"];
